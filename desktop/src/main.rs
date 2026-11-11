// No console window behind the app on Windows (release builds).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod assets;
mod host;
mod net;
mod parties;
mod playback;
mod together;
mod update;
mod theme;
mod ui;
mod views;

use api::Meta;
use futures::StreamExt;
use gpui::{
    Focusable, div, img, prelude::*, px, size, App, Application, Bounds, Context, Entity, FontWeight, Global, Image, ImageFormat, SharedString,
    TitlebarOptions, WeakEntity, Window, WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowOptions,
};
use serde_json::json;
use std::sync::Arc;
use ui::icon;

#[derive(Clone, PartialEq)]
pub enum Route {
    Home,
    Shows,
    Movies,
    Lists,
    Calendar,
    Couch,
    Search,
    Detail(Meta),
    Player,
    Settings,
    SignIn,
    Person(String, Option<u64>),
}

/// Getting around from anywhere: `Nav::detail(&meta, cx)`.
pub struct Nav(WeakEntity<Root>);
impl Global for Nav {}

impl Nav {
    fn root(cx: &App) -> Option<Entity<Root>> {
        cx.try_global::<Nav>().and_then(|n| n.0.upgrade())
    }
    pub fn go(route: Route, cx: &mut App) {
        if let Some(r) = Self::root(cx) {
            r.update(cx, |r, cx| {
                if r.route != route {
                    r.back.push(r.route.clone());
                    r.route = route;
                    // lists are fresh every visit
                    if r.route == Route::Lists {
                        if let Some(l) = &r.library {
                            l.update(cx, |_, cx| views::library::Library::load(cx));
                        }
                    }
                }
                cx.notify();
            });
        }
    }
    pub fn back(cx: &mut App) {
        if let Some(r) = Self::root(cx) {
            r.update(cx, |r, cx| {
                r.route = r.back.pop().unwrap_or(Route::Home);
                cx.notify();
            });
        }
    }
    pub fn detail(m: &Meta, cx: &mut App) {
        Self::go(Route::Detail(m.clone()), cx);
    }
    pub fn play(m: &Meta, cx: &mut App) {
        playback::quick_play(m.clone(), None, None, cx);
    }
    pub fn play_episode(m: &Meta, video_id: String, cx: &mut App) {
        playback::quick_play(m.clone(), Some(video_id), None, cx);
    }
    pub fn add(m: &Meta, cx: &mut App) {
        let meta = serde_json::to_value(m).unwrap_or_default();
        let name = m.name.clone();
        cx.spawn(async move |cx| {
            let r = api::call::<serde_json::Value>("library_add", json!({ "meta": meta })).await;
            let msg = match r {
                Ok(_) => format!("added {name} to My List"),
                Err(e) => e,
            };
            let _ = cx.update(|cx| Nav::toast(msg, cx));
        })
        .detach();
    }
    /// open "throw a watch party" for a title
    pub fn plan_party(m: &Meta, cx: &mut App) {
        if let Some(r) = Self::root(cx) {
            let p = r.read(cx).planner.clone();
            let m = m.clone();
            p.update(cx, |p, cx| p.open(m, cx));
        }
    }
    pub fn toast(msg: String, cx: &mut App) {
        if let Some(r) = Self::root(cx) {
            r.update(cx, |r, cx| {
                r.toast = Some(msg);
                cx.notify();
                cx.spawn(async move |this, cx| {
                    gpui::Timer::after(std::time::Duration::from_secs(3)).await;
                    let _ = this.update(cx, |r, cx| {
                        r.toast = None;
                        cx.notify();
                    });
                })
                .detach();
            });
        }
    }
}

pub struct Root {
    route: Route,
    back: Vec<Route>,
    home: Entity<views::home::Home>,
    shows: Option<Entity<views::home::Home>>,
    movies: Option<Entity<views::home::Home>>,
    toast: Option<String>,
    mark: Arc<Image>,
    player: Entity<views::player::PlayerView>,
    detail: Option<Entity<views::detail::Detail>>,
    search: Option<Entity<views::search::Search>>,
    library: Option<Entity<views::library::Library>>,
    calendar: Option<Entity<views::calendar::Calendar>>,
    couch: Option<Entity<views::couch::Couch>>,
    settings: Option<Entity<views::settings::Settings>>,
    signin: Option<Entity<views::signin::SignIn>>,
    person: Option<Entity<views::person::Person>>,
    invite: Entity<views::invite::Invite>,
    pub planner: Entity<views::planner::Planner>,
    me: serde_json::Value,
    focus: gpui::FocusHandle,
    last_size: gpui::Size<gpui::Pixels>,
}

const TABS: &[(&str, fn() -> Route)] = &[
    ("Home", || Route::Home),
    ("Shows", || Route::Shows),
    ("Movies", || Route::Movies),
    ("My Lists", || Route::Lists),
    ("Calendar", || Route::Calendar),
    ("Couch", || Route::Couch),
];

impl Root {
    fn nav_bar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let route = self.route.clone();
        div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h(px(76.))
            .bg(gpui::linear_gradient(180., gpui::linear_color_stop(theme::black(0.75), 0.), gpui::linear_color_stop(theme::black(0.0), 1.)))
            .child(div().absolute().top_0().left_0().size_full().window_control_area(WindowControlArea::Drag))
            .child(
                div()
                    .absolute()
                    .left(px(theme::GUTTER))
                    .top(px(18.))
                    .child(img(self.mark.clone()).w(px(52.)).h(px(40.))),
            )
            .child(
                div().absolute().top(px(18.)).left_0().w_full().flex().justify_center().child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            div()
                                .id("tab-search")
                                .size(px(38.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .cursor_pointer()
                                .when(route == Route::Search, |d| d.bg(theme::white(0.16)))
                                .hover(|s| s.bg(theme::white(0.1)))
                                .on_click(|_, _, cx| Nav::go(Route::Search, cx))
                                .child(icon("search", 20.).text_color(theme::label2())),
                        )
                        .children(TABS.iter().map(|(label, r)| {
                            let target = r();
                            let on = route == target || (matches!(route, Route::Detail(_)) && target == Route::Home && false);
                            div()
                                .id(SharedString::from(format!("tab-{label}")))
                                .h(px(38.))
                                .px(px(14.))
                                .flex()
                                .items_center()
                                .rounded_full()
                                .text_size(px(16.))
                                .font_weight(FontWeight::BOLD)
                                .cursor_pointer()
                                .text_color(if on { theme::text() } else { theme::label2() })
                                .when(on, |d| d.bg(theme::white(0.16)).border_1().border_color(theme::white(0.1)))
                                .when(!on, |d| d.hover(|s| s.text_color(theme::text())))
                                .on_click(move |_, _, cx| Nav::go(target.clone(), cx))
                                .child(*label)
                        })),
                ),
            )
            .child(
                div()
                    .id("nav-settings")
                    .absolute()
                    .right(px(theme::GUTTER + 108.))
                    .top(px(20.))
                    .cursor_pointer()
                    .on_click(|_, _, cx| Nav::go(Route::Settings, cx))
                    .size(px(36.))
                    .rounded(px(10.))
                    .bg(theme::neon())
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_weight(FontWeight::BLACK)
                    .text_color(theme::text())
                    .overflow_hidden()
                    .child(match self.me["avatar"].as_str() {
                        Some(u) => img(u.to_string()).size_full().into_any_element(),
                        None => div()
                            .child(self.me["name"].as_str().or(self.me["username"].as_str()).and_then(|n| n.chars().next()).unwrap_or('S').to_uppercase().to_string())
                            .into_any_element(),
                    }),
            )
            .child(window_controls())
    }
}

impl Root {
    /// keys: space play/pause, arrows seek/volume, f full screen, esc back
    fn key(&mut self, e: &gpui::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = e.keystroke.key.as_str();
        if self.route == Route::Player {
            let pb = playback::get(cx);
            let p = pb.read(cx);
            match k {
                "space" | "k" => p.cmd("pause", json!({ "value": !p.state.paused })),
                "left" | "j" => p.cmd("seek", json!({ "to": if k == "j" { -10 } else { -5 }, "relative": true })),
                "right" | "l" => p.cmd("seek", json!({ "to": if k == "l" { 10 } else { 5 }, "relative": true })),
                "up" => p.cmd("volume", json!({ "value": (p.state.volume + 5.).min(130.) })),
                "down" => p.cmd("volume", json!({ "value": (p.state.volume - 5.).max(0.) })),
                "m" => p.cmd("mute", json!({})),
                "f" => window.toggle_fullscreen(),
                "escape" => {
                    if window.is_fullscreen() {
                        window.toggle_fullscreen();
                    } else {
                        pb.update(cx, |p, cx| p.close(cx));
                        Nav::back(cx);
                    }
                }
                _ => {}
            }
            self.player.update(cx, |v, cx| v.poke(cx));
            return;
        }
        if k == "escape" || (k == "backspace" && matches!(self.route, Route::Detail(_))) {
            Nav::back(cx);
        }
    }
}

impl Root {
    /// people asking to join my couch; my own ask while it waits
    fn together_cards(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = together::get(cx);
        let tr = t.read(cx);
        let card = || {
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .p(px(12.))
                .pl(px(14.))
                .rounded(px(18.))
                .bg(gpui::rgba(0x0c090bf2))
                .border_1()
                .border_color(theme::white(0.1))
        };
        let mut col = div().absolute().top(px(70.)).right(px(theme::GUTTER)).flex().flex_col().gap(px(10.)).w(px(420.));
        for (i, a) in tr.inbox.iter().enumerate() {
            let name = a["fromName"].as_str().unwrap_or("someone").to_string();
            let (a1, a2) = (a.clone(), a.clone());
            let (t1, t2) = (t.clone(), t.clone());
            col = col.child(
                card()
                    .child(div().size(px(36.)).rounded(px(10.)).bg(theme::neon()).flex().items_center().justify_center().font_weight(FontWeight::BLACK).child(name.chars().next().unwrap_or('?').to_uppercase().to_string()))
                    .child(div().flex_1().flex().flex_col().child(div().font_weight(FontWeight::BOLD).child(name.clone())).child(div().text_size(px(13.)).text_color(theme::label2()).child("wants to join your couch")))
                    .child(
                        div()
                            .id(SharedString::from(format!("deny-{i}")))
                            .size(px(34.))
                            .rounded_full()
                            .bg(theme::white(0.1))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_click(move |_, _, cx| t1.update(cx, |t, cx| t.answer(a1.clone(), false, cx)))
                            .child(icon("close", 16.)),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("allow-{i}")))
                            .px(px(14.))
                            .h(px(36.))
                            .rounded_full()
                            .bg(theme::text())
                            .text_color(gpui::rgb(0x000000))
                            .font_weight(FontWeight::BOLD)
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .cursor_pointer()
                            .on_click(move |_, _, cx| t2.update(cx, |t, cx| t.answer(a2.clone(), true, cx)))
                            .child(icon("couch", 18.).text_color(gpui::rgb(0x000000)))
                            .child("Let them in"),
                    ),
            );
        }
        if let Some((_, _, name)) = &tr.asking {
            let t3 = t.clone();
            col = col.child(
                card()
                    .child(div().flex_1().text_size(px(15.)).child(format!("waiting for {name} to let you in…")))
                    .child(
                        div()
                            .id("ask-cancel")
                            .px(px(14.))
                            .h(px(34.))
                            .rounded_full()
                            .bg(theme::white(0.12))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .on_click(move |_, _, cx| t3.update(cx, |t, cx| t.cancel_ask(cx)))
                            .child("cancel"),
                    ),
            );
        }
        col
    }
}

/// minimize / maximize / close, drawn by couch (the title bar is ours)
fn window_controls() -> impl IntoElement {
    let btn = |id: &'static str, glyph: &'static str, area: WindowControlArea, danger: bool| {
        div()
            .id(id)
            .w(px(46.))
            .h(px(34.))
            .flex()
            .items_center()
            .justify_center()
            .text_color(theme::label2())
            .text_size(px(14.))
            .hover(move |s| if danger { s.bg(gpui::rgb(0xc42b1c)).text_color(theme::text()) } else { s.bg(theme::white(0.1)).text_color(theme::text()) })
            .window_control_area(area)
            .child(glyph)
    };
    div()
        .absolute()
        .top_0()
        .right_0()
        .flex()
        .child(btn("win-min", "—", WindowControlArea::Min, false))
        .child(btn("win-max", "▢", WindowControlArea::Max, false))
        .child(btn("win-close", "✕", WindowControlArea::Close, true))
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // mpv's window follows ours (resize, full screen, DPI)
        let size = window.viewport_size();
        if size != self.last_size {
            self.last_size = size;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            couch::player::refit_video(couch::state::get().core.host.main_window());
        }
        let playing = self.route == Route::Player;
        let page = match &self.route {
            Route::Home => self.home.clone().into_any_element(),
            Route::Player => self.player.clone().into_any_element(),
            Route::Calendar => self.calendar.get_or_insert_with(|| cx.new(views::calendar::Calendar::new)).clone().into_any_element(),
            Route::Couch => self.couch.get_or_insert_with(|| cx.new(views::couch::Couch::new)).clone().into_any_element(),
            Route::Settings => self.settings.get_or_insert_with(|| cx.new(views::settings::Settings::new)).clone().into_any_element(),
            Route::SignIn => self.signin.get_or_insert_with(|| cx.new(views::signin::SignIn::new)).clone().into_any_element(),
            Route::Person(name, id) => {
                let key = format!("{name}{id:?}");
                if self.person.as_ref().map_or(true, |p| p.read(cx).key != key) {
                    let (n, i) = (name.clone(), *id);
                    self.person = Some(cx.new(|cx| views::person::Person::new(n, i, cx)));
                }
                self.person.clone().unwrap().into_any_element()
            }
            Route::Lists => self.library.get_or_insert_with(|| cx.new(views::library::Library::new)).clone().into_any_element(),
            Route::Search => {
                let s = self.search.get_or_insert_with(|| cx.new(views::search::Search::new)).clone();
                let f = s.read(cx).input.read(cx).focus_handle(cx);
                if !f.contains_focused(window, cx) {
                    window.focus(&f);
                }
                s.into_any_element()
            }
            Route::Shows => self.shows.get_or_insert_with(|| cx.new(|cx| views::home::Home::new("series", cx))).clone().into_any_element(),
            Route::Movies => self.movies.get_or_insert_with(|| cx.new(|cx| views::home::Home::new("movie", cx))).clone().into_any_element(),
            Route::Detail(m) => {
                let fresh = self.detail.as_ref().map_or(true, |d| d.read(cx).base.id != m.id);
                if fresh {
                    let m = m.clone();
                    self.detail = Some(cx.new(|cx| views::detail::Detail::new(m, cx)));
                }
                self.detail.clone().unwrap().into_any_element()
            }
            other => views::soon(match other {
                Route::Shows => "shows",
                Route::Movies => "movies",
                Route::Lists => "my lists",
                Route::Calendar => "calendar",
                Route::Couch => "couch",
                Route::Search => "search",
                _ => "",
            }),
        };
        div()
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key))
            .when(!playing, |d| d.bg(theme::bg()))
            .font_family(theme::FONT)
            .text_color(theme::text())
            .child(page)
            .when(!playing && self.route != Route::SignIn, |d| d.child(self.nav_bar(cx)))
            .when(self.route == Route::SignIn, |d| d.child(window_controls()))
            .when(playing, |d| d.child(window_controls()))
            .child(self.together_cards(cx))
            .when(!playing && self.route != Route::SignIn, |d| d.child(self.planner.clone()).child(self.invite.clone()))
            .children(self.toast.clone().map(|t| {
                div().absolute().bottom(px(28.)).left_0().w_full().flex().justify_center().child(
                    div()
                        .px(px(18.))
                        .py(px(12.))
                        .rounded(px(14.))
                        .bg(theme::surface2())
                        .border_1()
                        .border_color(theme::white(0.1))
                        .text_size(px(15.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(t),
                )
            }))
    }
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,librqbit=warn,tracing=warn,naga=warn,blade_graphics=warn,cosmic_text=warn")).try_init();

    // the same folders as the webview app: settings, addons, library and
    // sign-ins carry straight over
    let data_dir = dirs::data_dir().unwrap_or_default().join("com.midnite.couch");
    let cache_dir = dirs::cache_dir().unwrap_or_default().join("com.midnite.couch");
    let (host, mut events) = host::GpuiHost::new();
    let host = Arc::new(host);
    let st = couch::engine::boot(host.clone(), data_dir, cache_dir).expect("couch engine");

    Application::new().with_assets(assets::Assets).run(move |cx: &mut App| {
        cx.text_system().add_fonts(assets::fonts()).expect("fonts");
        {
            use gpui::KeyBinding;
            use ui::input::*;
            let c = Some("TextInput");
            cx.bind_keys([
                KeyBinding::new("backspace", Backspace, c),
                KeyBinding::new("delete", Delete, c),
                KeyBinding::new("left", Left, c),
                KeyBinding::new("right", Right, c),
                KeyBinding::new("shift-left", SelectLeft, c),
                KeyBinding::new("shift-right", SelectRight, c),
                KeyBinding::new("ctrl-a", SelectAll, c),
                KeyBinding::new("ctrl-v", Paste, c),
                KeyBinding::new("ctrl-c", Copy, c),
                KeyBinding::new("ctrl-x", Cut, c),
                KeyBinding::new("home", Home, c),
                KeyBinding::new("end", End, c),
                KeyBinding::new("enter", Enter, c),
            ]);
        }
        cx.set_http_client(net::CouchHttp::new(st.core.clone()));
        playback::init(cx);
        together::init(cx);
        parties::init(cx);
        update::start(cx);

        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions { title: Some("midnite™ couch".into()), appears_transparent: true, traffic_light_position: None }),
                    window_background: WindowBackgroundAppearance::Opaque,
                    window_min_size: Some(size(px(900.), px(560.))),
                    app_id: Some("com.midnite.couch".into()),
                    ..Default::default()
                },
                |window, cx| {
                    // mpv draws into this window
                    #[cfg(target_os = "windows")]
                    {
                        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                        if let Ok(h) = HasWindowHandle::window_handle(window) {
                            if let RawWindowHandle::Win32(w) = h.as_raw() {
                                host.hwnd.store(w.hwnd.get(), std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    }
                    let _ = window;
                    let mark = Arc::new(Image::from_bytes(ImageFormat::Png, assets::bytes("brand/couch-mark.png").unwrap_or_default().to_vec()));
                    let root = cx.new(|cx| {
                        cx.observe(&together::get(cx), |_, _, cx| cx.notify()).detach();
                        let focus = cx.focus_handle();
                        window.focus(&focus);
                        // dev: start on a page (COUCH_START=detail:series:tt0903747)
                        let start = std::env::var("COUCH_START").ok().and_then(|v| {
                            let p: Vec<&str> = v.split(':').collect();
                            match p.as_slice() {
                                ["detail", k, i] => Some(Route::Detail(Meta { kind: (*k).into(), id: (*i).into(), ..Default::default() })),
                                ["search"] => Some(Route::Search),
                                ["lists"] => Some(Route::Lists),
                                ["calendar"] => Some(Route::Calendar),
                                ["couch"] => Some(Route::Couch),
                                ["settings"] => Some(Route::Settings),
                                ["shows"] => Some(Route::Shows),
                                ["home"] => Some(Route::Home),
                                ["movies"] => Some(Route::Movies),
                                _ => None,
                            }
                        });
                        // not signed in yet: Discord first
                        let me: serde_json::Value = couch::state::get().core.db.get("discord_user").unwrap_or_default();
                        let first = if me.is_null() { Route::SignIn } else { Route::Home };
                        Root {
                            route: start.unwrap_or(first),
                            back: vec![],
                            home: cx.new(|cx| views::home::Home::new("all", cx)),
                            shows: None,
                            movies: None,
                            toast: None,
                            mark,
                            player: cx.new(views::player::PlayerView::new),
                            detail: None,
                            search: None,
                            library: None,
                            calendar: None,
                            couch: None,
                            settings: None,
                            signin: None,
                            person: None,
                            invite: cx.new(views::invite::Invite::new),
                            planner: cx.new(views::planner::Planner::new),
                            me: me.clone(),
                            focus,
                            last_size: gpui::Size::default(),
                        }
                    });
                    cx.set_global(Nav(root.downgrade()));
                    root
                },
            )
            .expect("window");
        let _ = window;
        cx.activate(true);

        // the engine's events: open links, toasts for alerts
        cx.spawn(async move |cx| {
            while let Some(m) = events.next().await {
                let _ = cx.update(|cx| match m {
                    host::Msg::OpenUrl(u) => cx.open_url(&u),
                    host::Msg::Notify(t, b) => Nav::toast(format!("{t} · {b}"), cx),
                    host::Msg::Event(..) | host::Msg::Window(..) => {}
                });
            }
        })
        .detach();
    });
}
