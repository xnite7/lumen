// No console window behind the app on Windows (release builds).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod assets;
mod host;
mod net;
mod parties;
mod pip;
mod playback;
mod together;
mod update;
mod theme;
mod mini;
mod jump;
mod tile;
mod packaged;
mod uninstall;
mod media;
mod notify;
mod timer;
mod ui;
mod views;
mod seen;
mod tour;

thread_local! {
    static FPS_ON: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FRAMES: std::cell::RefCell<std::collections::VecDeque<std::time::Instant>> = std::cell::RefCell::new(std::collections::VecDeque::new());
}

static ON_PLAYER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// couch's window to the front (restored if minimised). Call it only when
/// nothing is mid-update: Windows messages the window synchronously.
fn bring_to_front() {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};
        let h = windows::Win32::Foundation::HWND(mini::main_hwnd() as *mut _);
        if h.0.is_null() {
            return;
        }
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        let _ = SetForegroundWindow(h);
    }
}

// the intro's frames (and how long each shows), decoded off the main thread at launch
thread_local! {
    /// when the film went to the corner (the mini bar slides up from then)
    static MINI_AT: std::cell::RefCell<Option<std::time::Instant>> = const { std::cell::RefCell::new(None) };
}

static INTRO: std::sync::OnceLock<Vec<(std::sync::Arc<gpui::RenderImage>, f32)>> = std::sync::OnceLock::new();

/// the loading couch (the old app's wobble), for the player
static WOBBLE: std::sync::OnceLock<Vec<(std::sync::Arc<gpui::RenderImage>, f32)>> = std::sync::OnceLock::new();

fn frames_of(path: &str) -> Vec<(std::sync::Arc<gpui::RenderImage>, f32)> {
    use image::AnimationDecoder;
    let Some(b) = assets::bytes(path) else { return vec![] };
    let Ok(mut d) = image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(b)) else { return vec![] };
    let _ = d.set_background_color(image::Rgba([0, 0, 0, 0]));
    let mut out = vec![];
    for f in d.into_frames().flatten() {
        let (n, dn) = f.delay().numer_denom_ms();
        let secs = (n as f32 / dn.max(1) as f32 / 1000.).max(0.016);
        let mut f = f;
        for px in f.buffer_mut().chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        out.push((std::sync::Arc::new(gpui::RenderImage::new(vec![f])), secs));
    }
    out
}

fn decode_intro() {
    std::thread::spawn(|| {
        let out = frames_of("brand/intro.webp");
        if !out.is_empty() {
            let _ = INTRO.set(out);
        }
        let j = frames_of("brand/nav-jiggle.webp");
        if !j.is_empty() {
            let _ = JIGGLE.set(j);
        }
        let w = frames_of("brand/couch-wobble.webp");
        if !w.is_empty() {
            let _ = WOBBLE.set(w);
        }
    });
}

/// the couch mark's jiggle (hovering it), on a loop: the frame `t` seconds in
pub fn couch_jiggle_frame(t: f32) -> Option<std::sync::Arc<gpui::RenderImage>> {
    frame_at(JIGGLE.get()?, t)
}

/// the intro's couch turning, on a loop (the trailer waits with it)
pub fn couch_spin_frame(t: f32) -> Option<std::sync::Arc<gpui::RenderImage>> {
    let fr = INTRO.get()?;
    // a short rest on the last frame each turn
    let total: f32 = fr.iter().map(|x| x.1).sum::<f32>() + 0.5;
    frame_at(fr, t % total)
}

fn frame_at(fr: &[(std::sync::Arc<gpui::RenderImage>, f32)], t: f32) -> Option<std::sync::Arc<gpui::RenderImage>> {
    let total: f32 = fr.iter().map(|x| x.1).sum();
    if total <= 0. {
        return None;
    }
    let t = t % total;
    let mut acc = 0.;
    fr.iter().find(|x| {
        acc += x.1;
        acc > t
    })
    .or(fr.last())
    .map(|x| x.0.clone())
}

static JIGGLE: std::sync::OnceLock<Vec<(std::sync::Arc<gpui::RenderImage>, f32)>> = std::sync::OnceLock::new();

/// The loading couch, on a loop: the frame for this moment (None while it decodes).
pub fn couch_loading_frame() -> Option<std::sync::Arc<gpui::RenderImage>> {
    static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let fr = WOBBLE.get()?;
    let total: f32 = fr.iter().map(|x| x.1).sum();
    if total <= 0. {
        return None;
    }
    let t = T0.get_or_init(std::time::Instant::now).elapsed().as_secs_f32() % total;
    let mut acc = 0.;
    fr.iter().find(|x| {
        acc += x.1;
        acc > t
    })
    .or(fr.last())
    .map(|x| x.0.clone())
}

/// The intro's sound (a warm swell that lands with the couch, air rushing round you into the app, a deep tail). Off with "intro sound" in settings.
fn play_intro_sound() {
    if !couch::state::get().core.db.get::<bool>("gpui_intro_sound").unwrap_or(true) {
        return;
    }
    #[cfg(target_os = "windows")]
    if let Some(b) = assets::bytes("brand/intro.wav") {
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
        unsafe {
            let _ = PlaySoundW(windows::core::PCWSTR(b.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
        }
    }
}

use api::Meta;
use futures::StreamExt;
use gpui::{
    Focusable, div, img, prelude::*, px, size, App, Bounds, Context, Entity, FontWeight, Global, Image, ImageFormat, SharedString,
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
    Member(String),
    Studio(u64, String),
    Catalog(views::catalog::Spec),
    Vibe,
    Following,
}

/// Getting around from anywhere: `Nav::detail(&meta, cx)`.
pub struct Nav(WeakEntity<Root>);
impl Global for Nav {}

impl Nav {
    fn root(cx: &App) -> Option<Entity<Root>> {
        cx.try_global::<Nav>().and_then(|n| n.0.upgrade())
    }
    // Everything here changes Root, so it happens just after whatever is
    // running now (which may be Root's own handler or render): never "Root is
    // already being updated".
    pub fn go(route: Route, cx: &mut App) {
        cx.defer(move |cx| Self::go_now(route, cx));
    }
    fn go_now(route: Route, cx: &mut App) {
        ui::ambient::follow(&route, cx);
        let v = crate::ui::video::get(cx);
        v.update(cx, |v, cx| v.stop_prefix("trailer:", cx));
        if let Some(r) = Self::root(cx) {
            r.update(cx, |r, cx| {
                if r.route != route {
                    r.back.push(r.route.clone());
                    r.route = route;
                    r.turned = Some((std::time::Instant::now(), false));
                    // the billboard / preview clip belongs to the page we left
                    let v = ui::video::get(cx);
                    let owner = v.read(cx).owner.clone();
                    v.update(cx, |v, cx| v.stop(&owner, cx));
                    // back home: continue watching shows where you just stopped
                    if r.route == Route::Home {
                        r.home.update(cx, |h, cx| h.reload_cw(cx));
                    }
                    // lists are fresh every visit
                    if r.route == Route::Lists {
                        if let Some(l) = &r.library {
                            l.update(cx, |_, cx| views::library::Library::load(cx));
                        }
                    }
                    if r.route == Route::Following {
                        if let Some(f) = &r.following {
                            f.update(cx, |_, cx| views::following::Following::load(cx));
                        }
                    }
                }
                cx.notify();
            });
        }
    }
    pub fn back(cx: &mut App) {
        cx.defer(|cx| {
            if let Some(r) = Self::root(cx) {
                if let Route::Detail(m) = &r.read(cx).route {
                    ui::zoom::back(&m.id);
                }
                let v = ui::video::get(cx);
                let owner = v.read(cx).owner.clone();
                v.update(cx, |v, cx| v.stop(&owner, cx));
                r.update(cx, |r, cx| {
                    r.route = r.back.pop().unwrap_or(Route::Home);
                    r.turned = Some((std::time::Instant::now(), true));
                    if r.route == Route::Home {
                        r.home.update(cx, |h, cx| h.reload_cw(cx));
                    }
                    cx.notify();
                });
                let route = r.read(cx).route.clone();
                ui::ambient::follow(&route, cx);
            }
        });
    }
    /// where we are (for things that act differently per page)
    pub fn route(cx: &App) -> Option<Route> {
        Self::root(cx).map(|r| r.read(cx).route.clone())
    }
    /// is this title's page the one on screen?
    pub fn showing_detail(id: &str, cx: &App) -> bool {
        Self::root(cx).map_or(false, |r| matches!(&r.read(cx).route, Route::Detail(m) if m.id == id))
    }
    /// the mini player, full size again
    pub fn expand(cx: &mut App) {
        Self::go(Route::Player, cx);
    }
    /// the right-click menu for a title
    pub fn menu(m: &Meta, at: gpui::Point<gpui::Pixels>, cx: &mut App) {
        let m = m.clone();
        cx.defer(move |cx| {
            if let Some(r) = Self::root(cx) {
                let me = r.read(cx).menu.clone();
                me.update(cx, |me, cx| me.show(m, at, cx));
            }
        });
    }
    pub fn detail(m: &Meta, cx: &mut App) {
        // from a card: its picture grows into the page
        ui::zoom::open(m, m.background.clone().or(m.poster.clone()).map(|u| crate::net::fit(&api::sized(&u, "original"), ui::view_w())));
        // a title always opens at its top, even one you'd scrolled down before
        views::detail::to_top();
        Self::go(Route::Detail(m.clone()), cx);
    }
    pub fn play(m: &Meta, cx: &mut App) {
        // on someone's couch, what's on is theirs to pick
        if !views::player::may("title", None, cx) {
            return;
        }
        playback::quick_play(m.clone(), None, None, cx);
    }
    pub fn play_episode(m: &Meta, video_id: String, cx: &mut App) {
        if !views::player::may("episode", None, cx) {
            return;
        }
        playback::quick_play(m.clone(), Some(video_id), None, cx);
    }
    /// is the player the page on screen?
    pub fn on_player(_cx: &App) -> bool {
        // (a flag, not a read of Root: this is asked from inside Root's own handlers)
        ON_PLAYER.load(std::sync::atomic::Ordering::Relaxed)
    }
    /// the + on a title: My List, your lists, or a new one
    pub fn add(m: &Meta, cx: &mut App) {
        let m = m.clone();
        cx.defer(move |cx| {
            if let Some(r) = Self::root(cx) {
                let a = r.read(cx).addto.clone();
                a.update(cx, |a, cx| a.open(m, cx));
            }
        });
    }
    /// open "throw a watch party" for a title
    pub fn plan_party(m: &Meta, cx: &mut App) {
        let m = m.clone();
        cx.defer(move |cx| {
            if let Some(r) = Self::root(cx) {
                let p = r.read(cx).planner.clone();
                p.update(cx, |p, cx| p.open(m, cx));
            }
        });
    }
    /// your month on the couch, as a card to share
    pub fn month(cx: &mut App) {
        cx.defer(|cx| {
            if let Some(r) = Self::root(cx) {
                let m = r.read(cx).month.clone();
                m.update(cx, |m, cx| m.show(cx));
            }
        });
    }
    /// your year on the couch
    pub fn wrapped(cx: &mut App) {
        cx.defer(|cx| {
            if let Some(r) = Self::root(cx) {
                let w = r.read(cx).wrapped.clone();
                w.update(cx, |w, cx| w.show(cx));
            }
        });
    }
    pub fn toast(msg: String, cx: &mut App) {
        cx.defer(move |cx| Self::toast_now(msg, cx));
    }
    fn toast_now(msg: String, cx: &mut App) {
        if let Some(r) = Self::root(cx) {
            r.update(cx, |r, cx| {
                r.toast = Some(msg);
                cx.notify();
                cx.spawn(async move |this, cx| {
                    crate::timer::after(std::time::Duration::from_secs(3)).await;
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
    #[allow(dead_code)]
    mark: Arc<Image>,
    /// the pointer on the couch mark up top (it jiggles)
    mark_hover: Option<std::time::Instant>,
    player: Entity<views::player::PlayerView>,
    detail: Option<Entity<views::detail::Detail>>,
    search: Option<Entity<views::search::Search>>,
    library: Option<Entity<views::library::Library>>,
    calendar: Option<Entity<views::calendar::Calendar>>,
    couch: Option<Entity<views::couch::Couch>>,
    settings: Option<Entity<views::settings::Settings>>,
    signin: Option<Entity<views::signin::SignIn>>,
    /// not (or no longer) allowed on the couch: the server said so
    locked: bool,
    person: Option<Entity<views::person::Person>>,
    member: Option<Entity<views::member::Member>>,
    studio: Option<Entity<views::studio::Studio>>,
    catalog: Option<Entity<views::catalog::Catalog>>,
    vibe: Option<Entity<views::vibe::Vibe>>,
    following: Option<Entity<views::following::Following>>,
    invite: Entity<views::invite::Invite>,
    pub planner: Entity<views::planner::Planner>,
    pub wrapped: Entity<views::wrapped::Wrapped>,
    pub month: Entity<views::month::Month>,
    pub addto: Entity<views::addto::AddTo>,
    pub menu: Entity<views::menu::Menu>,
    me: serde_json::Value,
    focus: gpui::FocusHandle,
    last_size: gpui::Size<gpui::Pixels>,
    /// the launch intro: when it started (None once it's done)
    splash: Option<std::time::Instant>,
    /// the first-time tour's step (None: not showing)
    tour: Option<usize>,
    /// when the intro's animation started playing
    intro_play: Option<std::time::Instant>,
    /// when the page last changed, and whether that was a step back (pages ease in)
    turned: Option<(std::time::Instant, bool)>,
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
    fn nav_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let route = self.route.clone();
        div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h(px(96.))
            // once you scroll: black behind the whole bar, fading out just below it;
            // all the way at the top there's none (the picture shows clean)
            .child({
                let k = (ui::smooth::scrolled() / 120.).clamp(0., 1.);
                // a soft shade even at the top: the tabs stay readable over a bright picture
                let k = 0.5 + 0.5 * (k * k * (3. - 2. * k));
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h(px(150.))
                    .bg(gpui::linear_gradient(180., gpui::linear_color_stop(theme::black(0.94 * k), 0.58), gpui::linear_color_stop(theme::black(0.0), 1.)))
            })
            .child(div().absolute().top_0().left_0().size_full().window_control_area(WindowControlArea::Drag))
            .child({
                // the couch mark (281×160 canvas; the couch itself starts 39px in)
                let (mw, mh) = (281. * 0.27, 160. * 0.27);
                let jig = self.mark_hover.and_then(|t| couch_jiggle_frame(t.elapsed().as_secs_f32()));
                div()
                    .id("nav-mark")
                    .occlude()
                    .absolute()
                    .left(px(theme::GUTTER - 39. * 0.27))
                    .top(px(22.))
                    .w(px(mw))
                    .h(px(mh))
                    .cursor_pointer()
                    .on_hover(cx.listener(|r, on: &bool, _, cx| {
                        r.mark_hover = on.then(std::time::Instant::now);
                        cx.notify();
                    }))
                    .on_click(|_, _, cx| Nav::go(Route::Home, cx))
                    .when(jig.is_some(), |d| d.child(gpui::canvas(|_, _, _| (), |_, _, w, _| w.request_animation_frame()).absolute().size_0()))
                    .child(match jig {
                        Some(f) => img(gpui::ImageSource::Render(f)).size_full().object_fit(gpui::ObjectFit::Contain).into_any_element(),
                        None => img("brand/nav-mark.png").size_full().object_fit(gpui::ObjectFit::Contain).into_any_element(),
                    })
            })
            .child(
                div().absolute().top(px(22.)).left_0().w_full().flex().justify_center().child(
                    div()
                        .relative()
                        .child(tour::spot("tabs"))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            div()
                                .id("tab-search")
                                .occlude()
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
                                .occlude()
                                .text_size(px(16.))
                                .font_weight(if on { FontWeight::MEDIUM } else { FontWeight::NORMAL })
                                .cursor_pointer()
                                .text_color(if on { theme::text() } else { theme::label2() })
                                .when(on, |d| d.bg(theme::white(0.16)))
                                .when(!on, |d| d.hover(|s| s.text_color(theme::text())))
                                .on_click(move |_, _, cx| Nav::go(target.clone(), cx))
                                .when(*label == "Couch", |d| d.relative().child(tour::spot("couch")))
                                .child(*label)
                        })),
                ),
            )
            .child(
                div()
                    .id("nav-settings")
                    .occlude()
                    .absolute()
                    .right(px(theme::GUTTER + 108.))
                    .top(px(24.))
                    .cursor_pointer()
                    .on_click({
                        let id = self.me["id"].as_str().map(String::from);
                        move |_, _, cx| match id.clone() {
                            Some(id) => Nav::go(Route::Member(id), cx),
                            None => Nav::go(Route::Settings, cx),
                        }
                    })
                    .size(px(36.))
                    .rounded_full()
                    // (already placed absolutely: that holds the tour's marker too)
                    .child(tour::spot("me"))
                    .when(self.me["avatar"].as_str().is_none(), |d| d.bg(theme::neon()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_weight(FontWeight::BLACK)
                    .text_color(theme::text())
                    .overflow_hidden()
                    .child(match self.me["avatar"].as_str() {
                        Some(u) => img(crate::net::face(u, 36.)).size_full().into_any_element(),
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
    fn key_up(&mut self, e: &gpui::KeyUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if e.keystroke.key != "space" || self.route != Route::Player {
            return;
        }
        // held: that was 2×, now over; a tap: play / pause
        if !views::player::hold_end(cx) {
            let pb = playback::get(cx);
            let paused = pb.read(cx).state.paused;
            if views::player::may(if paused { "play" } else { "pause" }, None, cx) {
                pb.read(cx).cmd("pause", json!({ "value": !paused }));
            }
        }
    }

    fn key(&mut self, e: &gpui::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = e.keystroke.key.as_str();
        // ctrl+shift+f: a frame-rate meter in the corner (how smooth is it here?)
        if k == "f" && e.keystroke.modifiers.control && e.keystroke.modifiers.shift {
            FPS_ON.with(|f| f.set(!f.get()));
            cx.notify();
            return;
        }
        // esc closes a menu or sheet first
        if k == "escape" && self.menu.read(cx).open.is_some() {
            self.menu.update(cx, |m, cx| {
                m.open = None;
                cx.notify();
            });
            return;
        }
        if k == "escape" && self.addto.read(cx).meta.is_some() {
            self.addto.update(cx, |a, cx| {
                a.meta = None;
                cx.notify();
            });
            return;
        }
        if k == "escape" && self.month.read(cx).open {
            self.month.update(cx, |m, cx| {
                m.open = false;
                cx.notify();
            });
            return;
        }
        if self.wrapped.read(cx).open {
            let w = self.wrapped.clone();
            match k {
                "right" | "space" => w.update(cx, |w, cx| w.step(1, cx)),
                "left" => w.update(cx, |w, cx| w.step(-1, cx)),
                "escape" => w.update(cx, |w, cx| {
                    w.open = false;
                    cx.notify();
                }),
                _ => {}
            }
            return;
        }
        if self.route == Route::Player {
            let pb = playback::get(cx);
            // the same rules as the buttons (on someone's couch: theirs to do)
            let (paused, pos) = (pb.read(cx).state.paused, pb.read(cx).state.pos);
            let step = match k {
                "left" => Some(-5.),
                "j" => Some(-10.),
                "right" => Some(5.),
                "l" => Some(10.),
                _ => None,
            };
            // space: a tap plays / pauses, held it's 2× until let go (decided on release)
            if k == "space" {
                if !e.is_held {
                    views::player::hold_begin(cx);
                }
                return;
            }
            if matches!(k, "k") {
                if views::player::may(if paused { "play" } else { "pause" }, None, cx) {
                    pb.read(cx).cmd("pause", json!({ "value": !paused }));
                }
                return;
            }
            if let Some(d) = step {
                if views::player::may("seek", Some((pos + d).max(0.)), cx) {
                    pb.read(cx).cmd("seek", json!({ "to": d, "relative": true }));
                }
                return;
            }
            let p = pb.read(cx);
            match k {
                "up" => p.cmd("volume", json!({ "value": (p.state.volume + 5.).min(130.) })),
                "down" => p.cmd("volume", json!({ "value": (p.state.volume - 5.).max(0.) })),
                "m" => p.cmd("mute", json!({})),
                "f" => window.toggle_fullscreen(),
                "p" | "i" => pip::toggle_in(window, cx),
                "escape" => {
                    if pip::is_on() {
                        pip::toggle(cx);
                    } else if window.is_fullscreen() {
                        window.toggle_fullscreen();
                    } else {
                        // (Nav updates Root: after this key press, not inside it)
                        cx.defer(|cx| Nav::back(cx));
                    }
                }
                _ => {}
            }
            self.player.update(cx, |v, cx| v.poke(cx));
            return;
        }
        // backspace while typing (a review, a search) is just backspace
        if k == "escape" || (k == "backspace" && matches!(self.route, Route::Detail(_)) && self.focus.is_focused(window)) {
            cx.defer(|cx| Nav::back(cx));
        }
    }
}

impl Root {
    /// The launch intro, like the webview's: the midnite sign holds, redraws
    /// itself into the couch, holds, and fades into home.
    fn intro(&mut self, window: &mut Window) -> Option<gpui::AnyElement> {
        // a short beat on the first frame (longer only while the animation is
        // still being decoded), the animation, a blink on the last frame, then
        // it rushes at you: zoom blur and fade
        const HOLD: f32 = 0.35;
        const END: f32 = 0.15;
        const FADE: f32 = 0.7;
        let start = self.splash?;
        let t = start.elapsed().as_secs_f32();
        let frames = INTRO.get();
        // the sound leads; the couch holds its first frame a beat longer so the
        // drawing lands with the sound's landing
        const LEAD: f32 = 0.4;
        thread_local! {
            static SOUNDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        if !SOUNDED.with(|x| x.get()) && t >= HOLD && (frames.is_some() || t > 3.) {
            SOUNDED.with(|x| x.set(true));
            play_intro_sound();
        }
        if self.intro_play.is_none() && t >= HOLD + LEAD && (frames.is_some() || t > 3.) {
            self.intro_play = Some(std::time::Instant::now());
        }
        let play: f32 = frames.map_or(0., |f| f.iter().map(|x| x.1).sum());
        let p = self.intro_play.map(|a| a.elapsed().as_secs_f32());
        if p.map_or(false, |p| p > play + END + FADE) {
            self.splash = None;
            self.intro_play = None;
            return None;
        }
        window.request_animation_frame();
        let first = || img("brand/intro-first.png").size_full().object_fit(gpui::ObjectFit::Contain).into_any_element();
        // the last frame, already decoded (a path image here can stall the zoom while it loads)
        let last_r = frames.and_then(|f| f.last()).map(|x| x.0.clone());
        let last = || match last_r.clone() {
            Some(r) => img(gpui::ImageSource::Render(r)).size_full().object_fit(gpui::ObjectFit::Contain).into_any_element(),
            None => img("brand/intro-last.png").size_full().object_fit(gpui::ObjectFit::Contain).into_any_element(),
        };
        let (pic, fade) = match (p, frames) {
            (None, _) => (first(), 0.),
            (Some(p), Some(fr)) if p < play => {
                let mut acc = 0.;
                let f = fr.iter().find(|x| {
                    acc += x.1;
                    acc > p
                });
                let f = f.or(fr.last()).map(|x| x.0.clone());
                (f.map(|r| img(gpui::ImageSource::Render(r)).size_full().object_fit(gpui::ObjectFit::Contain).into_any_element()).unwrap_or_else(first), 0.)
            }
            (Some(p), _) => (last(), ((p - play - END) / FADE).clamp(0., 1.)),
        };
        let (w, h) = (240., 216.);
        // ease in, then away: slow to start so the first frames of the zoom don't jump
        let e = if fade < 0.5 { 4. * fade * fade * fade } else { 1. - (-2. * fade + 2.).powi(3) / 2. };
        let mut stage = div().relative().w(px(w)).h(px(h));
        if fade > 0. {
            // zoom blur: a few copies of the last frame, each further forward and fainter
            for k in (0..4).rev() {
                let s = 1. + e * (0.6 + k as f32 * 0.22);
                let a = ((1. - e) * if k == 0 { 1. } else { 0.38 / k as f32 }).clamp(0., 1.);
                stage = stage.child(
                    div()
                        .absolute()
                        .left(px(w * (1. - s) / 2.))
                        .top(px(h * (1. - s) / 2.))
                        .w(px(w * s))
                        .h(px(h * s))
                        .opacity(a)
                        .child(last()),
                );
            }
        } else {
            stage = stage.child(pic);
        }
        Some(
            div()
                .id("intro")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .bg(theme::black((1. - e * 1.05).clamp(0., 1.)))
                .flex()
                .items_center()
                .justify_center()
                .child(stage)
                .into_any_element(),
        )
    }

    /// over the mini picture: what's playing, play/pause, full size, stop
    fn mini_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pb = playback::get(cx);
        let p = pb.read(cx);
        let st = p.state.clone();
        let title = p.meta.as_ref().map(|m| m.name.clone()).unwrap_or_default();
        let logo = p.meta.as_ref().and_then(|m| m.logo.clone()).map(|u| crate::net::logo(&api::sized(&u, "w500"), 200.));
        let label = p.full.as_ref().and_then(|f| {
            let v = playback::ordered(f).into_iter().find(|v| v["id"].as_str() == Some(p.video_id.as_str()))?;
            Some(format!("S{}:E{}", v["season"].as_i64()?, v["episode"].as_i64().or(v["number"].as_i64())?))
        });
        let line = [label.unwrap_or_default(), p.episode_title.clone()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        let paused = st.paused;
        let frac = if st.duration > 0. { (st.pos / st.duration).clamp(0., 1.) as f32 } else { 0. };
        let times = if st.duration > 0. { format!("{} / {}", views::player::clock(st.pos), views::player::clock(st.duration)) } else { String::new() };
        // slides up into place when the film goes to the corner
        let a = MINI_AT.with(|m| {
            let on_since = *m.borrow();
            match on_since {
                Some(t) => {
                    let x = (t.elapsed().as_secs_f32() / 0.45).min(1.);
                    if x < 1. {
                        window.request_animation_frame();
                    }
                    1. - (1. - x).powi(4)
                }
                None => {
                    *m.borrow_mut() = Some(std::time::Instant::now());
                    window.request_animation_frame();
                    0.
                }
            }
        });
        let b = |id: &'static str, ic: gpui::Svg, size: f32| {
            div()
                .id(id)
                .size(px(size))
                .flex_none()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(theme::white(0.14)))
                .active(|s| s.bg(theme::white(0.22)))
                .child(ic)
        };
        let (p1, p3, p4, p5) = (pb.clone(), pb.clone(), pb.clone(), pb.clone());
        let seek = move |pb: &Entity<playback::Playback>, by: f64, cx: &mut App| {
            let st = pb.read(cx).state.clone();
            let to = (st.pos + by).clamp(0., st.duration.max(0.));
            if views::player::may("seek", Some(to), cx) {
                pb.read(cx).cmd("seek", json!({ "to": to }));
            }
        };
        let left_edge = f32::from(window.viewport_size().width) - mini::M - mini::W;
        let t0 = MINI_AT.with(|m| m.borrow().map_or(0., |t| t.elapsed().as_secs_f32()));
        if !paused && views::player::bar_moves(views::player::bar_style()) {
            window.request_animation_frame();
        }
        div()
            .absolute()
            .right(px(mini::M))
            // under the picture (the picture sits above it, see mini::BAR)
            .bottom(px(mini::M - (1. - a) * 16.))
            .h(px(mini::BAR - 8.))
            .opacity(a)
            .w(px(mini::W))
            .rounded(px(18.))
            .overflow_hidden()
            // clicks on it stay on it (nothing behind it gets them)
            .id("mini-card")
            .occlude()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .bg(gpui::linear_gradient(180., gpui::linear_color_stop(gpui::rgba(0x1d181cf2), 0.), gpui::linear_color_stop(gpui::rgba(0x0e0b0df2), 1.)))
            .border_1()
            .border_color(theme::white(0.10))
            .shadow(vec![gpui::BoxShadow { color: theme::black(0.55), offset: gpui::point(px(0.), px(14.)), blur_radius: px(36.), spread_radius: px(0.), inset: false }])
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(8.))
            .px(px(14.))
            .child(
                // the progress first: a click anywhere on it jumps there
                div()
                    .id("mini-track")
                    .h(px(12.))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, move |e, _, cx| {
                        let x = (f32::from(e.position.x) - left_edge - 14.) / (mini::W - 28.);
                        let d = p5.read(cx).state.duration;
                        if d > 0. {
                            let to = x.clamp(0., 1.) as f64 * d;
                            if views::player::may("seek", Some(to), cx) {
                                p5.read(cx).cmd("seek", json!({ "to": to }));
                            }
                        }
                    })
                    .child(
                        div().w_full().h(px(4.)).rounded_full().bg(theme::white(0.16)).child(
                            div().relative().h_full().rounded_full().overflow_hidden().w(px((mini::W - 28.) * frac)).child({
                                // the colours of the film's logo, like the big player's bar
                                let [c1, c2] = views::player::play_tint().unwrap_or([theme::neon().into(), theme::neon_soft().into()]);
                                views::player::bar_fill(views::player::bar_style(), c1, c2, t0)
                            }),
                        ),
                    ),
            )
            .child(
                // what's on (left), the controls (middle), the time (right)
                div()
                    .flex()
                    .items_center()
                    .child(
                        div().w(px(120.)).min_w_0().flex().flex_col().children(match logo {
                            Some(u) => {
                                let t = title.clone();
                                Some(img(u).h(px(20.)).max_w(px(120.)).object_fit(gpui::ObjectFit::Contain).with_fallback(move || div().text_size(px(13.)).font_weight(FontWeight::BLACK).line_clamp(1).child(t.clone()).into_any_element()).into_any_element())
                            }
                            None => Some(div().text_size(px(13.)).font_weight(FontWeight::BLACK).line_clamp(1).text_ellipsis().child(title.clone()).into_any_element()),
                        }).children((!line.is_empty()).then(|| div().text_size(px(11.)).text_color(theme::label3()).line_clamp(1).text_ellipsis().child(line))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(6.))
                            .child(b("mini-b10", ui::icon("replay10", 18.), 32.).on_click(move |_, _, cx| seek(&p3, -10., cx)))
                            .child(
                                div()
                                    .id("mini-play")
                                    .size(px(34.))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .bg(theme::text())
                                    .hover(|s| s.opacity(0.88))
                                    .active(|s| s.opacity(0.75))
                                    .child(if paused { ui::icon_fill("play", 15.) } else { ui::icon_fill("pause", 15.) }.text_color(gpui::rgb(0x000000)))
                                    .on_click(move |_, _, cx| {
                                        let paused = p1.read(cx).state.paused;
                                        if views::player::may(if paused { "play" } else { "pause" }, None, cx) {
                                            p1.read(cx).cmd("pause", json!({ "value": !paused }));
                                        }
                                    }),
                            )
                            .child(b("mini-f10", ui::icon("forward10", 18.), 32.).on_click(move |_, _, cx| seek(&p4, 10., cx))),
                    )
                    .child(div().w(px(120.)).flex().justify_end().text_size(px(11.5)).text_color(theme::label3()).child(times)),
            )
    }

    /// Not on the couch (no couch role in the midnite server): nothing else works.
    fn lock_screen(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let btn = |id: &'static str, label: &'static str, strong: bool| {
            div()
                .id(id)
                .px(px(20.))
                .h(px(44.))
                .flex()
                .items_center()
                .rounded(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .when(strong, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
                .when(!strong, |d| d.bg(theme::white(0.1)).hover(|s| s.bg(theme::white(0.16))))
                .child(label)
        };
        div()
            .id("locked")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .bg(gpui::rgb(0x07050a))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .child(div().absolute().top_0().left_0().w_full().h(px(40.)).window_control_area(WindowControlArea::Drag))
            .child(window_controls())
            .child(img("brand/nav-mark.png").w(px(120.)).h(px(68.)).object_fit(gpui::ObjectFit::Contain).opacity(0.6))
            .child(div().text_size(px(30.)).font_weight(FontWeight::BLACK).child("you're off the couch"))
            .child(div().max_w(px(460.)).text_center().text_size(px(15.)).line_height(px(22.)).text_color(theme::label2()).child("couch is for the midnite crew. your account doesn't have the couch role in the server anymore. get it back, then check again."))
            .child(ui::join_discord("lock-join", "open the server"))
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .gap(px(10.))
                    .child(btn("lock-again", "check again", true).on_click(cx.listener(|_, _, _, cx| {
                        cx.spawn(async move |this, cx| {
                            let m = api::call::<String>("discord_membership", json!({})).await.unwrap_or_default();
                            let _ = this.update(cx, |r, cx| {
                                if m == "member" {
                                    r.locked = false;
                                } else {
                                    Nav::toast("still no couch role".into(), cx);
                                }
                                cx.notify();
                            });
                        })
                        .detach();
                    })))
                    .child(btn("lock-other", "use another account", false).on_click(cx.listener(|r, _, _, cx| {
                        r.locked = false;
                        cx.spawn(async move |_, cx| {
                            let _ = api::call::<serde_json::Value>("discord_logout", json!({})).await;
                            let _ = cx.update(|cx| Nav::go(Route::SignIn, cx));
                        })
                        .detach();
                        cx.notify();
                    }))),
            )
    }

    /// people asking to join my couch; my own ask while it waits
    fn together_cards(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = together::get(cx);
        let asks = t.read(cx).asks_now(cx);
        if !asks.is_empty() {
            // the time-left line runs down; the card goes when it's out
            window.request_animation_frame();
        }
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
        };
        let mut col = div().absolute().top(px(70.)).right(px(theme::GUTTER)).flex().flex_col().gap(px(10.)).w(px(420.));
        for (i, a) in asks.iter().enumerate() {
            let name = a["fromName"].as_str().unwrap_or("someone").to_string();
            let (a1, a2) = (a.clone(), a.clone());
            let (t1, t2) = (t.clone(), t.clone());
            let left = tr.ask_left(a["id"].as_str().unwrap_or(""));
            col = col.child(
                card()
                    .relative()
                    .overflow_hidden()
                    .child(div().absolute().left_0().bottom_0().h(px(3.)).w(gpui::relative(left)).bg(theme::neon()))
                    .child(div().size(px(36.)).rounded(px(10.)).bg(theme::neon()).flex().items_center().justify_center().font_weight(FontWeight::BLACK).child(name.chars().next().unwrap_or('?').to_uppercase().to_string()))
                    .child(div().flex_1().flex().flex_col().child(div().font_weight(FontWeight::SEMIBOLD).child(name.clone())).child(div().text_size(px(13.)).text_color(theme::label2()).child("wants to join your couch")))
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
                            .on_click(move |_, _, cx| {
                                // "not now": the card goes; no answer is sent, it just runs out
                                let id = a1["id"].as_str().unwrap_or("").to_string();
                                t1.update(cx, |t, cx| t.hide_ask(&id, cx))
                            })
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
                            .font_weight(FontWeight::SEMIBOLD)
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
            // on top of the top bar's drag area: Windows must see the button, not the drag
            .occlude()
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
        .child(btn("win-min", "─", WindowControlArea::Min, false))
        .child(btn("win-max", "▢", WindowControlArea::Max, false))
        .child(btn("win-close", "✕", WindowControlArea::Close, true))
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        ui::video::begin_frame();
        let fps = FPS_ON.with(|f| f.get()).then(|| {
            // frames drawn in the last second (while it's on, it keeps drawing)
            window.request_animation_frame();
            FRAMES.with(|fr| {
                let mut fr = fr.borrow_mut();
                let now = std::time::Instant::now();
                fr.push_back(now);
                while fr.front().map_or(false, |t| now.duration_since(*t).as_secs_f32() > 1.) {
                    fr.pop_front();
                }
                fr.len()
            })
        });
        ON_PLAYER.store(self.route == Route::Player, std::sync::atomic::Ordering::Relaxed);
        // full screen is for the film: leaving the player (back, or to the corner)
        // brings the window back, with its buttons
        if self.route != Route::Player && window.is_fullscreen() && !pip::is_on() {
            // (once: Windows takes a moment to report it changed)
            thread_local! {
                static LEFT: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
            }
            if LEFT.with(|l| l.get()).map_or(true, |t| t.elapsed().as_millis() > 1500) {
                LEFT.with(|l| l.set(Some(std::time::Instant::now())));
                window.toggle_fullscreen();
            }
        }
        let scope = match self.route {
            Route::Home => "all",
            Route::Shows => "series",
            Route::Movies => "movie",
            _ => "",
        };
        cx.defer(move |cx| ui::video::set_home_scope(scope, cx));
        ui::set_scale(window.scale_factor(), f32::from(window.viewport_size().width));
        // nothing has the keyboard (the box that had it is gone): couch takes it,
        // so Esc and the other keys keep working
        if window.focused(cx).is_none() {
            window.focus(&self.focus, cx);
        }
        // mpv's window follows ours (resize, full screen, DPI)
        let size = window.viewport_size();
        if size != self.last_size {
            self.last_size = size;
            let s = window.scale_factor();
            cx.defer(move |_| mini::fit(s));
        }
        // sheets and menus over the page keep the wheel for themselves
        ui::smooth::set_overlay(self.menu.read(cx).open.is_some() || self.addto.read(cx).meta.is_some() || self.wrapped.read(cx).open || self.planner.read(cx).meta.is_some());
        // the film keeps playing in the corner while you browse
        let want_mini = playback::get(cx).read(cx).open && self.route != Route::Player && !pip::is_on();
        if want_mini != mini::is_on() {
            // Windows calls (they message the window) wait until this update is done
            let s = window.scale_factor();
            cx.defer(move |_| mini::set(want_mini, s));
        }
        let playing = self.route == Route::Player;
        if playing {
            cx.defer(|_| mini::place_clip(None));
        }
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
            Route::Member(id) => {
                if self.member.as_ref().map_or(true, |m| &m.read(cx).id != id) {
                    let i = id.clone();
                    self.member = Some(cx.new(|cx| views::member::Member::new(i, cx)));
                }
                self.member.clone().unwrap().into_any_element()
            }
            Route::Vibe => {
                let v = self.vibe.get_or_insert_with(|| cx.new(views::vibe::Vibe::new)).clone();
                let f = v.read(cx).input.read(cx).focus_handle(cx);
                if !f.contains_focused(window, cx) {
                    window.focus(&f, cx);
                }
                v.into_any_element()
            }
            Route::Catalog(spec) => {
                if self.catalog.as_ref().map_or(true, |c| &c.read(cx).spec != spec) {
                    let sp = spec.clone();
                    self.catalog = Some(cx.new(|cx| views::catalog::Catalog::new(sp, cx)));
                }
                self.catalog.clone().unwrap().into_any_element()
            }
            Route::Studio(id, name) => {
                if self.studio.as_ref().map_or(true, |s| s.read(cx).id != *id) {
                    let (i, n) = (*id, name.clone());
                    self.studio = Some(cx.new(|cx| views::studio::Studio::new(i, n, cx)));
                }
                self.studio.clone().unwrap().into_any_element()
            }
            Route::Following => {
                // always fresh: you may have followed someone since
                let f = self.following.get_or_insert_with(|| cx.new(views::following::Following::new)).clone();
                f.into_any_element()
            }
            Route::Lists => self.library.get_or_insert_with(|| cx.new(views::library::Library::new)).clone().into_any_element(),
            Route::Search => {
                let s = self.search.get_or_insert_with(|| cx.new(views::search::Search::new)).clone();
                let f = s.read(cx).input.read(cx).focus_handle(cx);
                if !f.contains_focused(window, cx) {
                    window.focus(&f, cx);
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
        };
        // a page change eases in: it fades up from a little below (from a little
        // above when going back) instead of snapping
        const TURN: f32 = 0.32;
        let (e, dy) = match self.turned {
            // opening from a card: the picture is the motion, the page holds still
            Some((at, back)) if !playing && at.elapsed().as_secs_f32() < TURN && !(ui::zoom::is_opening() && !back) => {
                let t = at.elapsed().as_secs_f32() / TURN;
                let e = 1. - (1. - t).powi(3);
                window.request_animation_frame();
                (e, (1. - e) * if back { -14. } else { 22. })
            }
            _ => (1., 0.),
        };
        // (always the same wrapper, so nothing inside loses its state when the ease ends)
        let page = div().absolute().left_0().w_full().top(px(dy)).h_full().opacity(e).child(page);
        div()
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key))
            .on_key_up(cx.listener(Self::key_up))
            // the mouse's back button: back a page (from the player: the film goes to the corner)
            .on_mouse_down(gpui::MouseButton::Navigate(gpui::NavigationDirection::Back), cx.listener(|r, _, _, cx| {
                if r.wrapped.read(cx).open || r.menu.read(cx).open.is_some() || r.addto.read(cx).meta.is_some() {
                    r.wrapped.update(cx, |w, cx| {
                        w.open = false;
                        cx.notify();
                    });
                    r.menu.update(cx, |m, cx| {
                        m.open = None;
                        cx.notify();
                    });
                    r.addto.update(cx, |a, cx| {
                        a.meta = None;
                        cx.notify();
                    });
                    return;
                }
                cx.defer(|cx| Nav::back(cx));
            }))
            // the background, with a see-through spot where a GPU clip plays
            .when(!playing, |d| d.child(ui::video::paint_bg(theme::bg().into())))
            // the artwork you're on, glowing behind the page
            .when(!playing && self.route != Route::SignIn, |d| d.child(ui::ambient::layer(window)))
            .font_family(theme::FONT)
            .text_color(theme::text())
            .child(page)
            .when(!playing && self.route != Route::SignIn && !views::detail::theater_on(), |d| d.child(gpui::deferred(self.nav_bar(cx)).with_priority(2)))
            .when(self.route == Route::SignIn, |d| d.child(window_controls()))
            .children(fps.map(|n| div().absolute().bottom(px(12.)).left(px(12.)).px(px(10.)).py(px(4.)).rounded(px(8.)).bg(theme::black(0.7)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(if n >= 130 { gpui::rgb(0x7ee08a) } else if n >= 90 { gpui::rgb(0xf4c76a) } else { gpui::rgb(0xff6b6b) }).child(format!("{n} fps"))))
            // while playing: a strip along the top moves the window (it never pauses the film)
            .when(playing && !pip::is_on(), |d| d.child(div().absolute().top_0().left_0().right(px(150.)).h(px(24.)).window_control_area(WindowControlArea::Drag)))
            // full screen with the controls tucked away: nothing but the film
            .when(playing && !pip::is_on() && !(window.is_fullscreen() && self.player.read(cx).chrome_hidden()), |d| d.child(window_controls()))
            .child(self.together_cards(window, cx))
            .when(!playing && self.route != Route::SignIn, |d| d.child(self.planner.clone()).child(self.invite.clone()))
            .when(self.menu.read(cx).open.is_some(), |d| d.child(gpui::deferred(self.menu.clone()).with_priority(3)))
            .when(self.addto.read(cx).meta.is_some(), |d| d.child(gpui::deferred(self.addto.clone()).with_priority(3)))
            .when(self.wrapped.read(cx).open, |d| d.child(self.wrapped.clone()))
            .when(self.month.read(cx).open, |d| d.child(gpui::deferred(self.month.clone()).with_priority(4)))
            .when(!mini::is_on(), |d| {
                MINI_AT.with(|m| *m.borrow_mut() = None);
                d
            })
            .when(mini::is_on(), |d| d.child(gpui::deferred(self.mini_bar(window, cx)).with_priority(2)))
            .children(if playing { None } else { update::card(cx).map(|c| gpui::deferred(c).with_priority(2)) })
            .children(ui::zoom::overlay(window, cx))
            .children(self.intro(window))
            .when(self.locked, |d| d.child(gpui::deferred(self.lock_screen(cx)).with_priority(9)))
            .children({
                // the first time on Home (signed in, intro over): the tour
                if self.tour.is_none() && self.splash.is_none() && self.route == Route::Home && !playing && !tour::done() {
                    self.tour = Some(0);
                }
                let vs = window.viewport_size();
                self.tour.filter(|_| self.route == Route::Home && !playing).map(|i| {
                    let me = cx.entity().downgrade();
                    gpui::deferred(tour::view(i, f32::from(vs.width), f32::from(vs.height), move |next, cx| {
                        if next.is_none() {
                            tour::finish();
                        }
                        let _ = me.update(cx, |r, cx| {
                            r.tour = next;
                            cx.notify();
                        });
                    }))
                    .with_priority(5)
                })
            })
            .children(self.toast.clone().map(|t| {
                div().absolute().bottom(px(28.)).left_0().w_full().flex().justify_center().child(
                    div()
                        .px(px(18.))
                        .py(px(12.))
                        .rounded(px(14.))
                        .bg(theme::surface2())
                        .text_size(px(15.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(t),
                )
            }))
    }
}

/// A page by name: "detail:series:tt123", "search", "couch", …
fn route_of(spec: &str) -> Option<Route> {
    // (a couch: link from a notification carries the same spec)
    let spec = spec.trim().trim_matches('"');
    let spec = spec.strip_prefix("couch://").or(spec.strip_prefix("couch:")).unwrap_or(spec).trim_end_matches('/');
    let p: Vec<&str> = spec.split(':').collect();
    match p.as_slice() {
        ["detail", k, i @ ..] if !i.is_empty() => Some(Route::Detail(Meta { kind: (*k).into(), id: i.join(":"), ..Default::default() })),
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
}

/// Right-click on couch's taskbar icon: what you're in the middle of, what's
/// on your list, and quick ways in.
async fn refresh_jump_list() {
    let cw = api::call::<Vec<serde_json::Value>>("continue_watching", json!({})).await.unwrap_or_default();
    let watching: Vec<jump::Item> = cw
        .iter()
        .filter_map(|v| {
            let id = v["_id"].as_str().or(v["id"].as_str())?;
            let name = v["name"].as_str()?;
            let vid = v["state"]["video_id"].as_str().unwrap_or("");
            let p: Vec<&str> = vid.split(':').collect();
            let title = if p.len() == 3 { format!("{name} · S{} E{}", p[1], p[2]) } else { name.to_string() };
            Some(jump::Item { title, open: format!("detail:{}:{id}", v["type"].as_str().unwrap_or("movie")) })
        })
        .take(5)
        .collect();
    let lists = api::call::<serde_json::Value>("my_lists", json!({})).await.unwrap_or_default();
    let plan: Vec<jump::Item> = lists["plan"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| Some(jump::Item { title: m["name"].as_str()?.to_string(), open: format!("detail:{}:{}", m["type"].as_str().unwrap_or("movie"), m["id"].as_str()?) }))
        .take(4)
        .collect();
    let tasks = vec![
        jump::Item { title: "Search".into(), open: "search".into() },
        jump::Item { title: "Calendar".into(), open: "calendar".into() },
        jump::Item { title: "The couch".into(), open: "couch".into() },
        jump::Item { title: "My lists".into(), open: "lists".into() },
    ];
    jump::set(vec![("Continue watching".into(), watching), ("On your list".into(), plan), (String::new(), tasks)]);
}

fn main() {
    // from Windows' Apps list: remove couch (a script finishes once this closes)
    if uninstall::requested() {
        uninstall::run();
        return;
    }
    // started by the update's "restart now": the old couch is still closing,
    // so wait for it to go (else this one would hand over to it and quit too)
    if std::env::args().any(|a| a == "--after-update") {
        jump::wait_gone(std::time::Duration::from_secs(15));
    }
    // couch already open? hand it the page asked for, and stop here
    if jump::forward(Some(jump::requested().unwrap_or_else(|| "show".into()).as_str())) {
        return;
    }
    decode_intro();
    let launches = std::cell::Cell::new(Some(jump::listen()));
    // Sharp on scaled screens (125%, 150%…): without this Windows draws couch
    // at 100% and stretches the picture, which makes text and icons rough.
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    // if couch ever crashes, say why in a file (%LOCALAPPDATA%\\midnite couch (new)\\crash.log)
    std::panic::set_hook(Box::new(|info| {
        // (the installed couch's own folder is read-only: its log goes to %LOCALAPPDATA%\\midnite couch)
        let dir = if packaged::is_packaged() {
            dirs::data_local_dir().map(|d| d.join("midnite couch")).unwrap_or_else(std::env::temp_dir)
        } else {
            std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(std::env::temp_dir)
        };
        let _ = std::fs::create_dir_all(&dir);
        let msg = format!("{}\n{info}\n{}\n\n", update::VERSION, std::backtrace::Backtrace::force_capture());
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("crash.log")) {
            let _ = f.write_all(msg.as_bytes());
        }
        eprintln!("{msg}");
    }));
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,librqbit=warn,tracing=warn,naga=warn,blade_graphics=warn,cosmic_text=warn")).try_init();

    // the same folders as the webview app: settings, addons, library and
    // sign-ins carry straight over
    let data_dir = dirs::data_dir().unwrap_or_default().join("com.midnite.couch");
    let cache_dir = dirs::cache_dir().unwrap_or_default().join("com.midnite.couch");
    let (host, mut events) = host::GpuiHost::new();
    let host = Arc::new(host);
    let st = couch::engine::boot(host.clone(), data_dir, cache_dir).expect("couch engine");

    gpui_platform::application().with_assets(assets::Assets).run(move |cx: &mut App| {
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
        ui::video::init(cx);
        together::init(cx);
        parties::init(cx);
        update::start(cx);
        tile::start(cx);
        media::start(cx);
        notify::start(cx);
        // billboard + previews play with sound in the new couch (once; the speaker button changes it)
        {
            let core = couch::state::get().core.clone();
            if core.db.get::<bool>("gpui_sound_v1").is_none() {
                let mut st = core.settings();
                st.general.hero_muted = false;
                st.general.preview_muted = false;
                let _ = core.db.set("settings", &st);
                *core.settings.write() = st;
                let _ = core.db.set("gpui_sound_v1", &true);
            }
        }
        theme::apply_text_mode(cx);
        // smooth text: no DirectWrite contrast boost, so glyph edges fade like a browser's
        if theme::text_mode() == "smooth" {
            unsafe { std::env::set_var("COUCH_TEXT_CONTRAST", "0") };
            // and not snapped to whole pixels: soft, even edges
            unsafe { std::env::set_var("COUCH_TEXT_SOFT", "1") };
        }

        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions { title: Some("midnite™ couch".into()), appears_transparent: true, traffic_light_position: None }),
                    // see-through: mpv (the film, clips) draws in windows under the UI, and
                    // the new GPUI fills an "opaque" window with white first
                    window_background: WindowBackgroundAppearance::Transparent,
                    window_min_size: Some(size(px(900.), px(560.))),
                    // (installed as a package, Windows names the window itself)
                    app_id: (!packaged::is_packaged()).then(|| "com.midnite.couch".into()),
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
                                mini::init(w.hwnd.get());
                            }
                        }
                    }
                    let _ = window;
                    let mark = Arc::new(Image::from_bytes(ImageFormat::Png, assets::bytes("brand/couch-mark.png").unwrap_or_default().to_vec()));
                    let root = cx.new(|cx| {
                        cx.observe(&together::get(cx), |_, _, cx| cx.notify()).detach();
                        cx.observe(&playback::get(cx), |_, _, cx| cx.notify()).detach();
                        let addto = cx.new(views::addto::AddTo::new);
                        cx.observe(&addto, |_, _, cx| cx.notify()).detach();
                        let menu = cx.new(views::menu::Menu::new);
                        cx.observe(&menu, |_, _, cx| cx.notify()).detach();
                        // the mini player follows the window around
                        cx.observe_window_bounds(window, |_, window, cx| {
                            let s = window.scale_factor();
                            cx.defer(move |_| mini::fit(s));
                            // only a real move or resize: coming back to the window
                            // also reports its bounds, and that click must still count
                            thread_local! {
                                static LAST: std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>> = const { std::cell::Cell::new(None) };
                            }
                            let b = window.bounds();
                            if LAST.with(|l| l.replace(Some(b))).map_or(false, |o| o != b) {
                                views::player::window_moved();
                            }
                        })
                        .detach();
                        // still allowed in? (the role can be taken away any time): checked
                        // soon after launch, then every minute
                        cx.spawn(async move |this, cx| {
                            crate::timer::after(std::time::Duration::from_secs(6)).await;
                            loop {
                                let m = api::call::<String>("discord_membership", json!({})).await.unwrap_or_else(|_| "unknown".into());
                                let go_on = this
                                    .update(cx, |r: &mut Root, cx| {
                                        let was = r.locked;
                                        match m.as_str() {
                                            "outsider" => r.locked = true,
                                            "member" => r.locked = false,
                                            "signin" if r.route != Route::SignIn => {
                                                r.locked = false;
                                                Nav::go(Route::SignIn, cx);
                                            }
                                            _ => {}
                                        }
                                        if r.locked && !was {
                                            // the film stops too
                                            playback::get(cx).update(cx, |p, cx| p.close(cx));
                                        }
                                        cx.notify();
                                    })
                                    .is_ok();
                                if !go_on {
                                    break;
                                }
                                crate::timer::after(std::time::Duration::from_secs(60)).await;
                            }
                        })
                        .detach();
                        // a click on the mini picture: full size again
                        cx.spawn(async move |this, cx| loop {
                            crate::timer::after(std::time::Duration::from_millis(150)).await;
                            if this.upgrade().is_none() {
                                break;
                            }
                            // (not inside an update of Root: Nav updates Root itself)
                            if mini::CLICKED.swap(false, std::sync::atomic::Ordering::Relaxed) {
                                let _ = cx.update(|cx| Nav::expand(cx));
                            }
                            if mini::CLOSE.swap(false, std::sync::atomic::Ordering::Relaxed) {
                                mini::CLICKED.store(false, std::sync::atomic::Ordering::Relaxed);
                                let _ = cx.update(|cx| playback::get(cx).update(cx, |p, cx| p.close(cx)));
                            }
                        })
                        .detach();
                        let focus = cx.focus_handle();
                        window.focus(&focus, cx);
                        // a page asked for at launch (the jump list, COUCH_START=detail:series:tt0903747)
                        let start = jump::requested().and_then(|v| route_of(&v));
                        // later launches hand their page over here
                        let launches = launches.take();
                        cx.spawn_in(window, async move |this, cx| {
                            let Some(rx) = launches else { return };
                            loop {
                                crate::timer::after(std::time::Duration::from_millis(300)).await;
                                while let Ok(spec) = rx.try_recv() {
                                    if this.upgrade().is_none() {
                                        return;
                                    }
                                    // Nav updates Root itself, so not from inside an update of Root
                                    if let Some(route) = route_of(&spec) {
                                        let _ = cx.update(|_, cx| Nav::go(route, cx));
                                    }
                                    // to the front: Windows messages couch's window right away,
                                    // so only here, with nothing of couch's in the middle of an update
                                    bring_to_front();
                                }
                            }
                        })
                        .detach();
                        // the jump list: soon after start, then every 10 minutes
                        cx.spawn(async move |_, _| loop {
                            crate::timer::after(std::time::Duration::from_secs(8)).await;
                            refresh_jump_list().await;
                            crate::timer::after(std::time::Duration::from_secs(600)).await;
                        })
                        .detach();
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
                            mark_hover: None,
                            player: cx.new(views::player::PlayerView::new),
                            detail: None,
                            search: None,
                            library: None,
                            calendar: None,
                            couch: None,
                            settings: None,
                            signin: None,
                            person: None,
                            member: None,
                            studio: None,
                            catalog: None,
                            vibe: None,
                            following: None,
                            invite: cx.new(views::invite::Invite::new),
                            planner: cx.new(views::planner::Planner::new),
                            wrapped: cx.new(views::wrapped::Wrapped::new),
                            month: cx.new(views::month::Month::new),
                            addto,
                            menu,
                            me: me.clone(),
                            focus,
                            last_size: gpui::Size::default(),
                            splash: Some(std::time::Instant::now()),
                            tour: None,
                            intro_play: None,
                            locked: false,
                            turned: None,
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
