// No console window behind the app on Windows (release builds).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod assets;
mod host;
mod net;
mod theme;
mod ui;
mod views;

use api::Meta;
use futures::StreamExt;
use gpui::{
    div, img, prelude::*, px, size, App, Application, Bounds, Context, Entity, FontWeight, Global, Image, ImageFormat, SharedString,
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
        Self::toast(format!("the player comes next in this build: {}", m.name), cx);
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
    toast: Option<String>,
    mark: Arc<Image>,
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
                    .absolute()
                    .right(px(theme::GUTTER + 108.))
                    .top(px(20.))
                    .size(px(36.))
                    .rounded(px(10.))
                    .bg(theme::neon())
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_weight(FontWeight::BLACK)
                    .text_color(theme::text())
                    .child(couch::state::get().core.db.get::<String>("gpui_me_initial").unwrap_or_else(|| "S".into())),
            )
            .child(window_controls())
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = match &self.route {
            Route::Home => self.home.clone().into_any_element(),
            Route::Detail(m) => views::detail_stub(m),
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
            .bg(theme::bg())
            .font_family(theme::FONT)
            .text_color(theme::text())
            .child(page)
            .child(self.nav_bar(cx))
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
        cx.set_http_client(net::CouchHttp::new(st.core.clone()));

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
                    let root = cx.new(|cx| Root { route: Route::Home, back: vec![], home: cx.new(views::home::Home::new), toast: None, mark });
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
