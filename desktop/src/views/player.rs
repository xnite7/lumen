//! The player: mpv draws the picture underneath (its own child window);
//! this view is the see-through layer on top with couch's controls.
use crate::api::sized;
use crate::playback::{self, Playback};
#[allow(unused_imports)]
use crate::playback::Sleep;
use crate::theme;
use crate::ui::{icon, icon_fill};
use gpui::{
    SharedString, div, img, linear_color_stop, linear_gradient, prelude::*, px, Context, Entity, FontWeight, MouseMoveEvent, ObjectFit, Pixels, Window,
};
use serde_json::json;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
pub enum Menu {
    Audio,
    Subs,
    Speed,
    Episodes,
    Sources,
    /// host: who's on my couch (kick, hand over the remote)
    Couch,
}

pub struct PlayerView {
    pb: Entity<Playback>,
    menu: Option<Menu>,
    /// the menu last shown, and when it opened (it rises in)
    menu_seen: Option<Menu>,
    menu_at: Instant,
    /// seek-bar stills, every 10 s of the film
    stills: std::collections::HashMap<i64, Option<std::sync::Arc<gpui::Image>>>,
    vol_hud: Option<Instant>,
    last_move: Instant,
    /// where the pointer is (height), and the window's height
    mouse_y: f32,
    mouse_now: f32,
    view_h: f32,
    shown: bool,
    hover_x: Option<f32>,
    reactions: Entity<super::reactions::Reactions>,
    /// pointing at "next": show what it is
    next_hover: bool,
    /// where the "next" button sits (its middle, x): the preview card rises above it
    next_x: std::rc::Rc<std::cell::Cell<f32>>,
    /// the mouse-up that ended a 2× hold isn't a click on the film
    ate_click: bool,
    /// the pause blur last sent to mpv (0..1)
    blur_sent: f32,
    /// the credits countdown to the next episode: (that episode, when it began)
    countdown: Option<(String, Instant)>,
    /// you said no to it for this episode
    countdown_off: Option<String>,
    vol_acc: f64,
    vol_target: Option<f64>,
    /// dragging the scrub bar (hover_x is where)
    drag: bool,
    track_w: f32,
    /// the volume slider slides out of its button while you point at it
    vol_open: bool,
    /// clicks on the picture (a double click cancels the single's pause)
    click_gen: u64,
    /// when the volume pop-out opened
    vol_since: Option<Instant>,
    /// clicked open (stays until clicked again or elsewhere)
    vol_pinned: bool,
    /// when "next" started being pointed at (its card fades in)
    next_since: Option<Instant>,
    /// the scrub bar's place on screen last frame (left edge, width)
    scrub_at: std::rc::Rc<std::cell::Cell<(f32, f32)>>,
    /// the controls: showing or not, and since when (they slide in and out)
    chrome_on: bool,
    /// the paused screen (which episode, big), and when it last came or went
    pause_on: bool,
    pause_at: Instant,
    chrome_at: Instant,
    /// the progress bar's colours, from the title's logo (or its picture): (for which url, two colours)
    tint: Option<(String, [gpui::Hsla; 2])>,
    /// immersive light: the picture, soft, glowing in the black bars (latest, the one before, when it changed)
    glow: Option<std::sync::Arc<gpui::Image>>,
    glow_prev: Option<std::sync::Arc<gpui::Image>>,
    glow_at: Instant,
    vol_x: std::rc::Rc<std::cell::Cell<f32>>,
    /// for the bar's living colours
    born: Instant,
    /// the menus' lists, for gliding scroll (audio, subs, episodes, streams)
    ms: [gpui::ScrollHandle; 4],
    aspect: &'static str,
    /// the season the episodes menu is showing (None: the one playing)
    ep_season: Option<i64>,
    /// how well the stream is coming in while it loads: 0-4 bars (None: not that kind of stream)
    signal: Option<u8>,
}

impl PlayerView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let pb = playback::get(cx);
        cx.observe(&pb, |_, _, cx| cx.notify()).detach();
        // controls fade away after a few still seconds
        cx.spawn(async move |this, cx| loop {
            crate::timer::after(Duration::from_millis(400)).await;
            let Ok(()) = this.update(cx, |v, cx| {
                let paused = v.pb.read(cx).state.paused;
                // resting on the controls (top or bottom bar, an open slider) keeps them up
                let (my, vh) = (v.mouse_y, v.view_h);
                let on_bar = vh > 0. && my >= 0. && (my > vh - 130. || my < 110.);
                let busy = v.vol_open || v.next_hover || v.hover_x.is_some();
                // resting on the controls keeps them up a while longer, not forever
                // (a pointer left lying at the bottom of the screen kept them up)
                let idle = v.last_move.elapsed();
                let held = (on_bar || busy) && idle < Duration::from_secs(10);
                // paused and left alone a while: the controls go too (the title stays)
                let paused_fresh = paused && (idle < Duration::from_secs(4) || held);
                let show = paused_fresh || v.menu.is_some() || v.drag || v.vol_pinned || held || idle < Duration::from_millis(3200) || v.pb.read(cx).loading;
                // the little picture-in-picture window: controls only while the pointer is on it
                let show = if crate::pip::is_on() { crate::pip::cursor_inside() && !v.pb.read(cx).loading } else { show };
                if show != v.shown {
                    v.shown = show;
                    // the pointer goes away with the controls (back on the next move)
                    #[cfg(target_os = "windows")]
                    if !show {
                        unsafe {
                            let _ = windows::Win32::UI::WindowsAndMessaging::SetCursor(None);
                        }
                    }
                    cx.notify();
                }
            }) else {
                break;
            };
        })
        .detach();
        // while it loads: how strong the connection is, as bars
        cx.spawn(async move |this, cx| loop {
            crate::timer::after(Duration::from_millis(900)).await;
            let Ok(loading) = this.update(cx, |v, cx| v.pb.read(cx).loading) else { break };
            let sig = if loading {
                let st = crate::api::call::<serde_json::Value>("torrent_stats", json!({})).await.unwrap_or_default();
                (!st.is_null()).then(|| {
                    let peers = st["peers"].as_u64().unwrap_or(0);
                    let sp = st["downloadMbps"].as_f64().unwrap_or(0.);
                    match (peers, sp) {
                        (0, _) => 0,
                        (_, s) if s < 0.2 => 1,
                        (_, s) if s < 0.8 => 2,
                        (_, s) if s < 2.5 => 3,
                        _ => 4,
                    }
                })
            } else {
                None
            };
            let Ok(()) = this.update(cx, |v, cx| {
                if v.signal != sig {
                    v.signal = sig;
                    cx.notify();
                }
            }) else {
                break;
            };
        })
        .detach();
        // immersive light: every couple of seconds, a small still of what's on screen
        cx.spawn(async move |this, cx| loop {
            crate::timer::after(Duration::from_millis(1800)).await;
            let Ok(want) = this.update(cx, |v, cx| {
                let p = v.pb.read(cx);
                let on = couch::state::get().core.db.get::<bool>("gpui_immersive").unwrap_or(true);
                (on && p.open && !p.loading && p.state.duration > 0.).then_some(p.state.pos)
            }) else {
                break;
            };
            let Some(pos) = want else { continue };
            let Some(pic) = api_still(pos + 0.5).await else { continue };
            // soften it off the main thread
            let soft = couch::rt::spawn_blocking(move || soft_light(&pic)).await.ok().flatten();
            let Some(soft) = soft else { continue };
            let _ = this.update(cx, |v, cx| {
                v.glow_prev = v.glow.replace(soft);
                v.glow_at = Instant::now();
                cx.notify();
            });
        })
        .detach();
        let reactions = cx.new(super::reactions::Reactions::new);
        cx.observe(&reactions, |_, _, cx| cx.notify()).detach();
        Self { pb, menu: None, stills: Default::default(), vol_hud: None, last_move: Instant::now(), mouse_y: -1., mouse_now: -1., view_h: 0., shown: true, hover_x: None, reactions, next_hover: false, vol_acc: 0., vol_target: None, drag: false, track_w: 1., vol_open: false, click_gen: 0, vol_since: None, vol_pinned: false, next_since: None, scrub_at: std::rc::Rc::new(std::cell::Cell::new((0., 600.))), chrome_on: true, chrome_at: Instant::now(), pause_on: false, pause_at: Instant::now(), menu_seen: None, menu_at: Instant::now(), tint: None, glow: None, glow_prev: None, glow_at: Instant::now(), vol_x: Default::default(), born: Instant::now(), ms: Default::default(), aspect: match couch::state::get().core.settings().playback.aspect.as_str() { "fill" => "fill", "stretch" => "stretch", _ => "fit" }, ep_season: None, signal: None, countdown: None, countdown_off: None, blur_sent: 0., ate_click: false, next_x: Default::default() }
    }

    /// the controls are tucked away (the film alone on screen)
    pub fn chrome_hidden(&self) -> bool {
        !self.chrome_on
    }

    pub fn poke(&mut self, cx: &mut Context<Self>) {
        self.last_move = Instant::now();
        self.mouse_y = self.mouse_now;
        if !self.shown {
            self.shown = true;
            cx.notify();
        }
    }
}

thread_local! {
    static MOVED: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
}
/// the window was just moved or resized: the mouse-up that ends it isn't a click on the film
pub fn window_moved() {
    MOVED.with(|m| m.set(Some(Instant::now())));
}
fn just_moved() -> bool {
    MOVED.with(|m| m.get()).map_or(false, |t| t.elapsed() < Duration::from_millis(500))
}

/// Watching on someone's couch: pause/seek go to the host (when they've
/// shared the remote); otherwise they stay with the host. True = do it here too.
/// May I do this on watch together? Not a guest: yes. A guest the host shared
/// the remote with: play / pause / seek go to the host (and yes). Anything
/// else from a guest: no, and the player says so.
pub fn may(action: &str, pos: Option<f64>, cx: &mut gpui::App) -> bool {
    let t = crate::together::get(cx);
    if t.read(cx).following.is_none() {
        return true;
    }
    let steerable = matches!(action, "pause" | "play" | "seek");
    let sent = steerable && t.update(cx, |t, _| t.steer(action, pos));
    if !sent {
        t.update(cx, |t, cx| t.deny(action, cx));
    }
    sent
}

/// headphone surround (saved): 5.1 / 7.1 placed around your head
pub fn headphones_on() -> bool {
    couch::state::get().core.db.get::<bool>("gpui_headphones").unwrap_or(false)
}

/// night mode (saved): loud scenes tamed, dialogue lifted
pub fn night_on() -> bool {
    couch::state::get().core.db.get::<bool>("gpui_night").unwrap_or(false)
}

thread_local! {
    /// subtitle auto-sync running
    static SYNCING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

thread_local! {
    /// held down (space or the picture): 2× until let go. (the speed to go back to)
    static BOOST: std::cell::Cell<Option<(f64, bool)>> = const { std::cell::Cell::new(None) };
    /// a hold turned into 2×: the click that ends it is not a click (no pause)
    static ATE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// when the hold began, and how many presses so far (a newer press cancels an older wait)
    static HOLD: std::cell::Cell<(Option<Instant>, u64)> = const { std::cell::Cell::new((None, 0)) };
}

/// Hold for 2×: on someone's couch (yours or theirs) speed belongs to sync, so not there.
pub fn boost_start(cx: &mut gpui::App) -> bool {
    if BOOST.with(|b| b.get()).is_some() {
        return true;
    }
    let t = crate::together::get(cx);
    if t.read(cx).following.is_some() || t.read(cx).hosting.is_some() {
        return false;
    }
    let pb = crate::playback::get(cx);
    let p = pb.read(cx);
    if !p.open || p.loading {
        return false;
    }
    // paused: it plays at 2× while held, and pauses again after
    let was_paused = p.state.paused;
    BOOST.with(|b| b.set(Some((p.state.speed.max(0.25), was_paused))));
    ATE.with(|a| a.set(true));
    p.cmd("speed", json!({ "value": 2.0 }));
    if was_paused {
        p.cmd("pause", json!({ "value": false }));
    }
    cx.refresh_windows();
    true
}
/// Let go: back to the speed before. True if it was speeding.
pub fn boost_end(cx: &mut gpui::App) -> bool {
    let Some((prev, was_paused)) = BOOST.with(|b| b.take()) else { return false };
    let pb = crate::playback::get(cx);
    pb.read(cx).cmd("speed", json!({ "value": prev }));
    if was_paused {
        pb.read(cx).cmd("pause", json!({ "value": true }));
    }
    cx.refresh_windows();
    true
}
pub fn boosting() -> bool {
    BOOST.with(|b| b.get()).is_some()
}
/// a press began (space or the picture): after 350 ms still held, 2×
pub fn hold_begin(cx: &mut gpui::App) {
    ATE.with(|a| a.set(false));
    let n = HOLD.with(|h| {
        let n = h.get().1 + 1;
        h.set((Some(Instant::now()), n));
        n
    });
    cx.spawn(async move |cx| {
        crate::timer::after(Duration::from_millis(350)).await;
        let still = HOLD.with(|h| h.get()) .1 == n && HOLD.with(|h| h.get()).0.is_some();
        if still {
            let _ = cx.update(|cx| boost_start(cx));
        }
    })
    .detach();
}
/// the press let go: true when it was a hold (so it isn't also a click / pause)
pub fn hold_end(cx: &mut gpui::App) -> bool {
    let long = HOLD.with(|h| {
        let (at, n) = h.get();
        h.set((None, n));
        at.map_or(false, |t| t.elapsed() >= Duration::from_millis(350))
    });
    // a long press is never a tap, even if 2× couldn't start (watch together)
    boost_end(cx) || long
}
/// the click that arrives with letting go of a 2× hold: swallow it (once)
pub fn hold_ate_click() -> bool {
    ATE.with(|a| a.replace(false))
}

/// the "guests can't …" sign over the player: the control crossed out, and why
fn denied_sign(cx: &mut Context<PlayerView>, window: &mut Window) -> Option<gpui::AnyElement> {
    let (ic, words, at) = crate::together::get(cx).read(cx).denied.clone()?;
    let t = at.elapsed().as_secs_f32();
    if t > 1.8 {
        return None;
    }
    window.request_animation_frame();
    let a = if t < 0.12 { t / 0.12 } else if t > 1.4 { 1. - (t - 1.4) / 0.4 } else { 1. };
    let s = if t < 0.12 { 0.9 + 0.1 * (t / 0.12) } else { 1. };
    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .opacity(a)
            .child(
                div()
                    .size(px(104. * s))
                    .rounded_full()
                    .bg(theme::black(0.62))
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_fill(ic, 46. * s))
                    .child(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().child(icon("slash", 74. * s))),
            )
            .child(div().px(px(16.)).py(px(8.)).rounded_full().bg(theme::black(0.62)).text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(words))
            .into_any_element(),
    )
}

/// a still of the film, made soft and dim for the light around the picture
fn soft_light(pic: &std::sync::Arc<gpui::Image>) -> Option<std::sync::Arc<gpui::Image>> {
    let im = image::load_from_memory(&pic.bytes).ok()?;
    let small = image::imageops::resize(&im.to_rgba8(), 40, 24, image::imageops::FilterType::Triangle);
    let mut soft = image::imageops::blur(&small, 2.6);
    for p in soft.pixels_mut() {
        let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
        let l = 0.299 * r + 0.587 * g + 0.114 * b;
        let sat = |c: f32| ((l + (c - l) * 1.25) * 0.62).clamp(0., 255.);
        p[0] = sat(r) as u8;
        p[1] = sat(g) as u8;
        p[2] = sat(b) as u8;
        p[3] = 255;
    }
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(soft).write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(std::sync::Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, out.into_inner())))
}

/// The progress bar styles: (key, name in settings, moves?)
pub const BAR_STYLES: [(&str, &str, bool); 13] = [
    ("mesh", "shimmer", true),
    ("chameleon", "chameleon", false),
    ("fade", "logo fade", false),
    ("white", "plain white", false),
    ("steel", "steel", true),
    ("pulse", "pulse", true),
    ("aurora", "aurora", true),
    ("ember", "ember", true),
    ("ocean", "ocean", true),
    ("midnite", "midnite", true),
    ("gold", "gold", true),
    ("blocks", "blocks", false),
    ("rainbow", "rainbow", true),
];

/// these two take the colours of what you're watching
pub fn bar_dynamic(style: &str) -> bool {
    matches!(style, "mesh" | "chameleon" | "fade")
}

thread_local! {
    /// the colours of the last title logo looked at (settings' samples use them)
    static LAST_TINT: std::cell::Cell<Option<[gpui::Hsla; 2]>> = const { std::cell::Cell::new(None) };
}
pub fn set_last_tint(t: [gpui::Hsla; 2]) {
    LAST_TINT.with(|c| c.set(Some(t)));
}
thread_local! {
    /// the colours of what's playing now (the mini player's bar uses them too)
    static PLAY_TINT: std::cell::Cell<Option<[gpui::Hsla; 2]>> = const { std::cell::Cell::new(None) };
}
pub fn play_tint() -> Option<[gpui::Hsla; 2]> {
    PLAY_TINT.with(|c| c.get())
}
pub fn last_tint() -> Option<[gpui::Hsla; 2]> {
    LAST_TINT.with(|c| c.get())
}

/// Which progress bar is picked in settings (living colours unless chosen).
pub fn bar_style() -> &'static str {
    let k = couch::state::get().core.db.get::<String>("gpui_bar_style").unwrap_or_default();
    BAR_STYLES.iter().find(|s| s.0 == k).map_or("mesh", |s| s.0)
}

/// does this style animate (it needs frames)?
pub fn bar_moves(style: &str) -> bool {
    BAR_STYLES.iter().find(|s| s.0 == style).map_or(false, |s| s.2)
}

/// the progress bar's fill, filling its parent
pub fn bar_fill(style: &str, c1: gpui::Hsla, c2: gpui::Hsla, t: f32) -> gpui::AnyElement {
    let h = |hh: f32, s: f32, l: f32| gpui::hsla(hh / 360., s, l, 1.);
    match style {
        "white" => div().absolute().size_full().bg(gpui::white()).into_any_element(),
        "chameleon" => div().absolute().size_full().bg(linear_gradient(90., linear_color_stop(c1, 0.), linear_color_stop(c2, 1.))).into_any_element(),
        // the logo's colour, top to bottom: a light edge on top (1%), the colour
        // itself (15%), deepening to dark at the bottom (100%)
        "fade" => {
            let light = gpui::hsla(c1.h, c1.s, (c1.l + (1. - c1.l) * 0.7).min(0.95), 1.);
            let dark = gpui::hsla(c1.h, c1.s, c1.l * 0.3, 1.);
            div()
                .absolute()
                .size_full()
                .flex()
                .flex_col()
                .child(div().w_full().h(gpui::relative(0.01)).bg(light))
                .child(div().w_full().h(gpui::relative(0.14)).bg(linear_gradient(180., linear_color_stop(light, 0.), linear_color_stop(c1, 1.))))
                .child(div().w_full().flex_1().bg(linear_gradient(180., linear_color_stop(c1, 0.), linear_color_stop(dark, 1.))))
                .into_any_element()
        }
        // brushed metal with a light running along it
        "steel" => sheen(h(220., 0.06, 0.62), h(220., 0.08, 0.82), t, 0.9).into_any_element(),
        // plain white, a soft brighter band breathing along it
        "pulse" => sheen(h(0., 0., 0.86), h(0., 0., 1.), t, 1.6).into_any_element(),
        "aurora" => mesh3(h(150., 0.75, 0.55), h(185., 0.8, 0.55), h(275., 0.6, 0.6), t, false).into_any_element(),
        "ember" => mesh3(h(18., 0.95, 0.55), h(38., 1., 0.58), h(355., 0.85, 0.45), t, true).into_any_element(),
        "ocean" => mesh3(h(200., 0.85, 0.5), h(215., 0.9, 0.62), h(180., 0.7, 0.45), t, false).into_any_element(),
        "midnite" => mesh3(h(320., 0.9, 0.6), h(280., 0.75, 0.55), h(340., 0.95, 0.68), t, true).into_any_element(),
        "gold" => sheen(h(42., 0.85, 0.5), h(48., 1., 0.78), t, 0.7).into_any_element(),
        "blocks" => blocks().into_any_element(),
        "rainbow" => rainbow(t).into_any_element(),
        _ => mesh(c1, c2, t).into_any_element(),
    }
}

fn mesh3(a: gpui::Hsla, b: gpui::Hsla, c: gpui::Hsla, t: f32, sparks: bool) -> impl IntoElement {
    mesh_with(a, b, Some(c), t, sparks)
}

/// one colour with a brighter band gliding along it (`speed`: passes per second, roughly)
fn sheen(base: gpui::Hsla, light: gpui::Hsla, t: f32, speed: f32) -> impl IntoElement {
    gpui::canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            let (x0, y0, w, hh) = (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height));
            if w < 1. {
                return;
            }
            window.paint_quad(gpui::fill(b, base));
            // the band moves across the whole bar's length, ~220px wide, softly edged
            let band = 220f32.min(w);
            let span = 1400f32.max(w + band);
            let cx_ = ((t * speed * 0.25).fract()) * span - band / 2.;
            let step = 3.;
            let n = (band / step) as i32;
            let (a, l) = (base.to_rgb(), light.to_rgb());
            for k in 0..n {
                let x = cx_ - band / 2. + k as f32 * step;
                if x < 0. || x > w {
                    continue;
                }
                let d = ((k as f32 / n as f32) - 0.5).abs() * 2.;
                let m = (1. - d * d).max(0.) * 0.85;
                let col = gpui::Rgba { r: a.r + (l.r - a.r) * m, g: a.g + (l.g - a.g) * m, b: a.b + (l.b - a.b) * m, a: 1. };
                window.paint_quad(gpui::fill(gpui::Bounds::new(gpui::point(px(x0 + x), px(y0)), gpui::size(px(step + 0.5), px(hh))), col));
            }
        },
    )
    .absolute()
    .size_full()
}

/// white blocks with small gaps, like an old VU meter
fn blocks() -> impl IntoElement {
    gpui::canvas(
        |_, _, _| (),
        |b, _, window, _| {
            let (x0, y0, w, hh) = (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height));
            let (bw, gap) = (10f32, 3f32);
            let mut x = 0.;
            while x < w {
                let ww = bw.min(w - x);
                window.paint_quad(gpui::fill(gpui::Bounds::new(gpui::point(px(x0 + x), px(y0)), gpui::size(px(ww), px(hh))), gpui::white()).corner_radii(px(1.5)));
                x += bw + gap;
            }
        },
    )
    .absolute()
    .size_full()
}

/// the whole spectrum, slowly sliding
fn rainbow(t: f32) -> impl IntoElement {
    gpui::canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            let (x0, y0, w, hh) = (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height));
            let step = 3f32;
            let mut x = 0f32;
            while x < w {
                let hue = ((x / 900.) - t * 0.06).rem_euclid(1.);
                window.paint_quad(gpui::fill(gpui::Bounds::new(gpui::point(px(x0 + x), px(y0)), gpui::size(px(step.min(w - x) + 0.5), px(hh))), gpui::hsla(hue, 0.85, 0.6, 1.)));
                x += step;
            }
        },
    )
    .absolute()
    .size_full()
}

/// The progress fill: the title's colours as a living mesh, waves of them
/// drifting through each other, film grain over it, a few sparks riding along.
fn mesh(c1: gpui::Hsla, c2: gpui::Hsla, t: f32) -> impl IntoElement {
    mesh_with(c1, c2, None, t, true)
}

/// (`third`: a third colour; none = a deeper shade of the first; `sparks`: the riding lights)
fn mesh_with(c1: gpui::Hsla, c2: gpui::Hsla, third: Option<gpui::Hsla>, t: f32, sparks: bool) -> impl IntoElement {
    gpui::canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            let (x0, y0) = (f32::from(b.origin.x), f32::from(b.origin.y));
            let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
            if w < 1. || h < 1. {
                return;
            }
            let a = c1.to_rgb();
            let bb = c2.to_rgb();
            // a third, deeper shade between them
            let c3 = third.unwrap_or(gpui::hsla((c1.h + 0.94) % 1., c1.s, (c1.l * 0.7).max(0.3), 1.)).to_rgb();
            let mixc = |p: &gpui::Rgba, q: &gpui::Rgba, k: f32| gpui::Rgba { r: p.r + (q.r - p.r) * k, g: p.g + (q.g - p.g) * k, b: p.b + (q.b - p.b) * k, a: 1. };
            // grain changes at 24 fps, like film
            let frame = (t * 24.) as u32;
            let hash = |x: u32, y: u32| {
                let mut v = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ frame.wrapping_mul(2246822519);
                v = (v ^ (v >> 13)).wrapping_mul(1274126177);
                ((v ^ (v >> 16)) & 1023) as f32 / 1023.
            };
            let cw = 3.;
            let rows = if h >= 6. { 2 } else { 1 };
            let rh = h / rows as f32;
            let cols = (w / cw).ceil() as u32;
            for i in 0..cols {
                let x = i as f32 * cw;
                for r in 0..rows {
                    let y = r as f32 * rh;
                    // two slow waves across the bar, a third that breathes
                    let u = 0.5 + 0.5 * (x * 0.011 + t * 1.1 + y * 0.4).sin() * (x * 0.0037 - t * 0.55).cos();
                    let v = 0.5 + 0.5 * (x * 0.006 - t * 0.8 + 1.7 + y * 0.3).sin();
                    let mut c = mixc(&mixc(&a, &bb, u), &c3, v * 0.45);
                    let g = (hash(i, r as u32) - 0.5) * 0.16;
                    c.r = (c.r + g).clamp(0., 1.);
                    c.g = (c.g + g).clamp(0., 1.);
                    c.b = (c.b + g).clamp(0., 1.);
                    let cell = gpui::Bounds::new(gpui::point(px(x0 + x), px(y0 + y)), gpui::size(px(cw.min(w - x) + 0.5), px(rh + 0.5)));
                    window.paint_quad(gpui::fill(cell, c));
                }
            }
            // sparks: a few soft lights riding the waves
            for k in 0..if sparks { 12u32 } else { 0 } {
                let sp = 0.035 + (k as f32 * 0.37).fract() * 0.06;
                let px_ = ((k as f32 * 0.618 + t * sp).fract()) * w;
                let life = 0.5 + 0.5 * (t * (1.3 + k as f32 * 0.21) + k as f32).sin();
                let sz = (h * 0.55).clamp(1.5, 3.);
                let py = y0 + h / 2. - sz / 2. + (t * 2. + k as f32).sin() * h * 0.2;
                let spark = gpui::Bounds::new(gpui::point(px(x0 + px_), px(py)), gpui::size(px(sz), px(sz)));
                window.paint_quad(gpui::fill(spark, gpui::hsla(0., 0., 1., 0.25 + 0.5 * life * life)).corner_radii(px(sz / 2.)));
            }
        },
    )
    .absolute()
    .size_full()
}

/// two colours for the progress bar from a picture: its liveliest one, and a neighbour
pub fn tint_of(bytes: &[u8]) -> Option<[gpui::Hsla; 2]> {
    let im = image::load_from_memory(bytes).ok()?.to_rgba8();
    let small = image::imageops::resize(&im, 32, 32, image::imageops::FilterType::Triangle);
    let mut best: Option<(f32, gpui::Hsla)> = None;
    for p in small.pixels() {
        if p[3] < 160 {
            continue;
        }
        let c: gpui::Hsla = gpui::Rgba { r: p[0] as f32 / 255., g: p[1] as f32 / 255., b: p[2] as f32 / 255., a: 1. }.into();
        let score = c.s * (1. - (c.l - 0.55).abs() * 1.6);
        if best.map_or(true, |b| score > b.0) {
            best = Some((score, c));
        }
    }
    let (score, c) = best?;
    if score < 0.18 {
        return None;
    }
    let a = gpui::hsla(c.h, c.s.max(0.55), c.l.clamp(0.5, 0.65), 1.);
    let b = gpui::hsla((c.h + 0.09) % 1., (c.s * 0.9).max(0.5), (c.l + 0.08).clamp(0.55, 0.75), 1.);
    Some([a, b])
}

async fn api_still(t: f64) -> Option<std::sync::Arc<gpui::Image>> {
    use base64::Engine;
    let url = crate::api::call::<Option<String>>("player_thumb", json!({ "t": t })).await.ok()??;
    let b64 = url.split(',').nth(1)?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    Some(std::sync::Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Jpeg, bytes)))
}

pub fn clock(t: f64) -> String {
    let t = t.max(0.) as u64;
    let (h, m, s) = (t / 3600, (t % 3600) / 60, t % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn lang(code: &str) -> String {
    let c = code.to_lowercase();
    let name = match c.as_str() {
        "en" | "eng" => "English",
        "ar" | "ara" => "Arabic",
        "fr" | "fre" | "fra" => "French",
        "es" | "spa" => "Spanish",
        "de" | "ger" | "deu" => "German",
        "it" | "ita" => "Italian",
        "pt" | "por" => "Portuguese",
        "ru" | "rus" => "Russian",
        "ja" | "jpn" => "Japanese",
        "ko" | "kor" => "Korean",
        "zh" | "chi" | "zho" => "Chinese",
        "hi" | "hin" => "Hindi",
        "tr" | "tur" => "Turkish",
        "nl" | "dut" | "nld" => "Dutch",
        "pl" | "pol" => "Polish",
        "sv" | "swe" => "Swedish",
        "" | "und" => "Unknown",
        other => return other.to_string(),
    };
    name.into()
}

/// "English 1", "English 2", "Arabic" — simple names for the tracks
fn named(list: &[serde_json::Value]) -> Vec<(serde_json::Value, String)> {
    let mut total = std::collections::HashMap::new();
    for t in list {
        *total.entry(lang(t["lang"].as_str().unwrap_or(""))).or_insert(0) += 1;
    }
    let mut seen = std::collections::HashMap::new();
    list.iter()
        .map(|t| {
            let l = lang(t["lang"].as_str().unwrap_or(""));
            let n = seen.entry(l.clone()).or_insert(0);
            *n += 1;
            let label = if total[&l] > 1 { format!("{l} {n}") } else { l };
            (t.clone(), label)
        })
        .collect()
}

fn pill(id: &'static str, label: String) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(52.))
        .px(px(24.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(px(14.))
        .bg(theme::text())
        .text_color(gpui::rgb(0x000000))
        .text_size(px(17.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|s| s.opacity(0.88))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(label)
}

/// TheIntroDB's parts, each in its own colour (the bar and the skip buttons)
pub fn seg_color(kind: &str) -> gpui::Rgba {
    match kind {
        "intro" => gpui::rgb(0x3b82f6),
        "recap" => gpui::rgb(0xf97316),
        "credits" => gpui::rgb(0xa855f7),
        "preview" => gpui::rgb(0x14b8a6),
        _ => gpui::rgb(0xffffff),
    }
}

/// a skip button in its part's colour: tinted glass, coloured words and edge
fn seg_pill(id: &'static str, kind: &str, label: String) -> gpui::Stateful<gpui::Div> {
    let c = seg_color(kind);
    let mix = |a: f32| gpui::Rgba { a, ..c };
    div()
        .id(id)
        .relative()
        .overflow_hidden()
        .h(px(52.))
        .px(px(24.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(px(14.))
        .bg(gpui::rgba(0x0a0c14d9))
        .border_1()
        .border_color(mix(0.45))
        .shadow(vec![gpui::BoxShadow { color: mix(0.45).into(), offset: gpui::point(px(0.), px(10.)), blur_radius: px(30.), spread_radius: px(-12.), inset: false }])
        .text_color(c)
        .text_size(px(17.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(move |s| s.bg(gpui::Rgba { a: 0.28, ..c }))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(div().absolute().top_0().left_0().size_full().bg(mix(0.16)))
        .child(div().relative().child(label))
}

/// the next episode, from the credits: with a countdown when it plays by itself
fn next_pill(pb: Entity<Playback>, label: String, kind: &str, left: Option<f32>, on_cancel: Option<Box<dyn Fn(&mut gpui::App)>>) -> gpui::AnyElement {
    let c = seg_color(kind);
    let main = seg_pill("pl-upnext", kind, match left {
        Some(l) => format!("next episode · {label} · {}", l.ceil() as i64),
        None => format!("next episode · {label}"),
    })
    // the countdown drains along the bottom
    .children(left.map(|l| div().absolute().left_0().bottom_0().h(px(3.)).w(gpui::relative((l / COUNTDOWN).clamp(0., 1.))).bg(c)))
    .on_click(move |_, _, cx| {
        cx.stop_propagation();
        if may("episode", None, cx) {
            pb.update(cx, |p, cx| p.play_next(cx))
        }
    });
    let mut row = div().flex().items_center().gap(px(8.)).child(main);
    if let Some(f) = on_cancel {
        row = row.child(
            div()
                .id("pl-upnext-x")
                .size(px(52.))
                .rounded(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .bg(gpui::rgba(0x0a0c14d9))
                .border_1()
                .border_color(theme::white(0.12))
                .hover(|s| s.bg(theme::white(0.12)))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    f(cx)
                })
                .child(icon("close", 18.)),
        );
    }
    row.into_any_element()
}

/// seconds the next episode waits in the credits before it plays by itself
const COUNTDOWN: f32 = 10.;

/// the dark glass the controls sit on
fn glass(d: gpui::Div) -> gpui::Div {
    d.bg(gpui::rgba(0x151114cc)).border_1().border_color(theme::white(0.10)).shadow(vec![gpui::BoxShadow {
        color: theme::black(0.45),
        offset: gpui::point(px(0.), px(10.)),
        blur_radius: px(30.),
        spread_radius: px(0.),
        inset: false,
    }])
}

/// a button in the controls bar; lit while its menu is open
fn bar_btn(id: &'static str, child: impl IntoElement, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .size(px(44.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .cursor_pointer()
        .when(on, |d| d.bg(theme::white(0.16)))
        // the hover light fades in and out instead of switching
        .child(
            gpui::canvas(
                |_, _, _| (),
                move |b, _, window, _| {
                    let over = b.contains(&window.mouse_position());
                    let since = HOVER.with(|m| {
                        let mut m = m.borrow_mut();
                        let e = m.entry(id).or_insert((false, Instant::now() - Duration::from_secs(5)));
                        if e.0 != over {
                            *e = (over, Instant::now());
                        }
                        e.1
                    });
                    let x = (since.elapsed().as_secs_f32() / 0.2).min(1.);
                    if x < 1. {
                        window.request_animation_frame();
                    }
                    let e = 1. - (1. - x).powi(3);
                    let a = if over { e } else { 1. - e };
                    if a > 0.001 {
                        let r = b.size.width / 2.;
                        window.paint_quad(gpui::fill(b, theme::white(0.15 * a)).corner_radii(r));
                    }
                },
            )
            .absolute()
            .size_full(),
        )
        .child(child)
}

thread_local! {
    /// each control-bar button: is the pointer on it, since when
    static HOVER: std::cell::RefCell<std::collections::HashMap<&'static str, (bool, Instant)>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// ±seconds, by the watch-together rules
fn seek_by(pb: &Entity<Playback>, d: f64, cx: &mut gpui::App) {
    let pos = pb.read(cx).state.pos;
    if may("seek", Some((pos + d).max(0.)), cx) {
        pb.read(cx).cmd("seek", json!({ "to": d, "relative": true }));
    }
}

fn round_btn(id: &'static str, child: impl IntoElement, size: f32) -> gpui::Stateful<gpui::Div> {
    // a press on a button never reaches the picture (which would play/pause)
    div()
        .id(id)
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .cursor_pointer()
        .hover(|s| s.bg(theme::white(0.14)))
        .child(child)
}

impl PlayerView {
    fn toggle_menu(&mut self, m: Menu, cx: &mut Context<Self>) {
        self.menu = if self.menu == Some(m) { None } else { Some(m) };
        if self.menu == Some(Menu::Couch) {
            crate::together::get(cx).read(cx).ask_roster();
        }
        // one thing open at a time: the volume slider and the reactions go
        if self.menu.is_some() {
            self.vol_pinned = false;
            self.next_hover = false;
            self.reactions.update(cx, |r, cx| {
                if r.open {
                    r.open = false;
                    cx.notify();
                }
            });
        }
        self.ep_season = None;
        cx.notify();
    }

    /// let go of the scrub bar: go there
    fn finish_drag(&mut self, cx: &mut Context<Self>) {
        if !self.drag {
            return;
        }
        self.drag = false;
        let d = self.pb.read(cx).state.duration;
        if let Some(x) = self.hover_x {
            let to = (x / self.track_w.max(1.)).clamp(0., 1.) as f64 * d;
            if d > 0. && may("seek", Some(to), cx) {
                self.pb.read(cx).cmd("seek", json!({ "to": to }));
            }
        }
        cx.notify();
    }

    fn set_volume_at(&mut self, x: f32, cx: &mut Context<Self>) {
        let max = if couch::state::get().core.settings().playback.volume_boost { 130. } else { 100. };
        // (x is the pointer's height; vol_x the track's bottom)
        let vol = (((self.vol_x.get() - x) / 130.).clamp(0., 1.) as f64 * max * 10.).round() / 10.;
        self.vol_target = Some(vol);
        self.vol_hud = Some(Instant::now());
        self.pb.read(cx).cmd("volume", json!({ "value": vol }));
        cx.notify();
    }

    fn want_still(&mut self, t: f64, cx: &mut Context<Self>) -> Option<std::sync::Arc<gpui::Image>> {
        // (the stills are made per 2 s)
        let k = (t / 2.).floor() as i64;
        // not there yet: the nearest one that is (never an empty box)
        let nearest = |stills: &std::collections::HashMap<i64, Option<std::sync::Arc<gpui::Image>>>| {
            stills.iter().filter_map(|(kk, v)| v.as_ref().map(|i| ((kk - k).abs(), i.clone()))).min_by_key(|x| x.0).map(|x| x.1)
        };
        if let Some(v) = self.stills.get(&k) {
            return v.clone().or_else(|| nearest(&self.stills));
        }
        let near = nearest(&self.stills);
        self.stills.insert(k, None);
        cx.spawn(async move |this, cx| {
            let r = api_still(k as f64 * 2. + 1.).await;
            let _ = this.update(cx, |v, cx| {
                match r {
                    Some(i) => {
                        v.stills.insert(k, Some(i));
                    }
                    // skipped (you'd moved on) or not ready: asked again next time
                    None => {
                        v.stills.remove(&k);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        near
    }

    fn menu_panel(&self, m: Menu, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let p = self.pb.read(cx);
        let row_f = |id: SharedString, label: String, on: bool, flag: Option<String>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(12.))
                .h(px(40.))
                .rounded(px(10.))
                .cursor_pointer()
                .text_size(px(15.))
                .font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                .hover(|s| s.bg(theme::white(0.1)))
                .child(div().w(px(18.)).flex_none().children(on.then(|| icon("check", 18.))))
                .children(flag.map(|u| img(u).h(px(16.)).flex_none().object_fit(ObjectFit::Contain)))
                .child(div().min_w_0().truncate().child(label))
        };
        let head = |t: &str| div().px(px(12.)).pt(px(6.)).pb(px(6.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child(t.to_string());

        let (width, body): (f32, gpui::AnyElement) = match m {
            // audio and subtitles, side by side
            Menu::Audio | Menu::Subs => {
                let col = |kind: &'static str| -> Vec<gpui::AnyElement> {
                    let list: Vec<serde_json::Value> = p.tracks.iter().filter(|t| t["type"] == kind).cloned().collect();
                    let mut rows: Vec<gpui::AnyElement> = vec![];
                    if kind == "sub" {
                        let off = !list.iter().any(|t| t["selected"].as_bool() == Some(true));
                        let pb = self.pb.clone();
                        rows.push(row_f("sub-off".into(), "Off".into(), off, None).on_click(move |_, _, cx| pb.update(cx, |p, cx| p.pick_track("sub", None, cx))).into_any_element());
                    }
                    for (i, (t, label)) in named(&list).into_iter().enumerate() {
                        let on = t["selected"].as_bool() == Some(true);
                        let id = t["id"].as_i64().unwrap_or(0);
                        let pb = self.pb.clone();
                        let flag = crate::ui::badges::flag(t["lang"].as_str().unwrap_or(""));
                        rows.push(row_f(SharedString::from(format!("{kind}-{i}")), label, on, flag).on_click(move |_, _, cx| pb.update(cx, |p, cx| p.pick_track(kind, Some(id), cx))).into_any_element());
                    }
                    if list.is_empty() && kind == "audio" {
                        rows.push(div().px(px(12.)).py(px(8.)).text_size(px(14.)).text_color(theme::label3()).child("just the one").into_any_element());
                    }
                    rows
                };
                let subs_on = p.tracks.iter().any(|t| t["type"] == "sub" && t["selected"].as_bool() == Some(true));
                let d = p.state.sub_delay;
                let nudge = |id: &'static str, label: &'static str, by: f64| {
                    let pb = self.pb.clone();
                    div()
                        .id(id)
                        .px(px(12.))
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .bg(theme::white(0.1))
                        .hover(|s| s.bg(theme::white(0.18)))
                        .cursor_pointer()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label)
                        .on_click(move |_, _, cx| {
                            let now = pb.read(cx).state.sub_delay;
                            pb.read(cx).cmd("sub-delay", json!({ "value": ((now + by) * 10.).round() / 10. }));
                        })
                };
                let left = faded(div().id("pm-audio").max_h(px(380.)).overflow_y_scroll().track_scroll(&self.ms[0]).child(crate::ui::smooth::wheel(&self.ms[0], "pm-audio")).flex().flex_col().children(col("audio")), &self.ms[0]);
                let right = faded(div().id("pm-subs").max_h(px(380.)).overflow_y_scroll().track_scroll(&self.ms[1]).child(crate::ui::smooth::wheel(&self.ms[1], "pm-subs")).flex().flex_col().children(col("sub")), &self.ms[1]);
                // subtitle timing, kept at the bottom: the delay, nudges, and auto sync
                let busy = SYNCING.with(|c| c.get());
                let sync_btn = div()
                    .id("sd-auto")
                    .relative()
                    .group("sd-auto")
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .cursor_pointer()
                    .bg(if busy { theme::white(0.22) } else { theme::white(0.1) })
                    .hover(|s| s.bg(theme::white(0.18)))
                    .child(icon(if busy { "sparkles-fill" } else { "sparkles" }, 16.))
                    // pointing at it: what it does
                    .child(
                        div()
                            .absolute()
                            .bottom(px(36.))
                            .right(px(-6.))
                            .px(px(10.))
                            .py(px(5.))
                            .rounded(px(10.))
                            .bg(gpui::rgba(0x0c090bf2))
                            .border_1()
                            .border_color(theme::white(0.12))
                            .whitespace_nowrap()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .opacity(0.)
                            .group_hover("sd-auto", |s| s.opacity(1.))
                            .child(if busy { "syncing… listening to the film" } else { "auto sync: listens and lines the subtitles up" }),
                    )
                    .on_click(|_, _, cx| {
                        if SYNCING.with(|c| c.replace(true)) {
                            return;
                        }
                        cx.refresh_windows();
                        cx.spawn(async move |cx| {
                            let r = crate::api::call::<serde_json::Value>("subs_autosync", json!({})).await;
                            SYNCING.with(|c| c.set(false));
                            let msg = match r {
                                Ok(v) => {
                                    let off = v["offset"].as_f64().unwrap_or(0.);
                                    let ratio = v["ratio"].as_f64().unwrap_or(1.);
                                    if (ratio - 1.).abs() > 1e-4 {
                                        format!("subtitles synced (they were made for a different frame rate, {:+.1} s)", off)
                                    } else if off.abs() < 0.1 {
                                        "subtitles were already in sync".to_string()
                                    } else {
                                        format!("subtitles synced: moved {:.1} s {}", off.abs(), if off < 0. { "earlier" } else { "later" })
                                    }
                                }
                                Err(e) => e,
                            };
                            let _ = cx.update(|cx| {
                                crate::Nav::toast(msg, cx);
                                cx.refresh_windows();
                            });
                        })
                        .detach();
                    });
                let timing = div()
                    .mt(px(4.))
                    .pt(px(8.))
                    .px(px(8.))
                    .border_t_1()
                    .border_color(theme::white(0.08))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(div().flex_1().text_size(px(12.)).text_color(theme::label3()).child(format!("subtitle timing {}{:.1} s", if d > 0. { "+" } else { "" }, d)))
                    .child(nudge("sd-minus", "−0.1", -0.1))
                    .child(nudge("sd-plus", "+0.1", 0.1))
                    .child(sync_btn);
                (
                    470.,
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .gap(px(6.))
                                .child(div().flex_1().min_w_0().flex().flex_col().child(head("audio")).child(left))
                                .child(div().w(px(1.)).my(px(8.)).bg(theme::white(0.08)))
                                .child(div().flex_1().min_w_0().flex().flex_col().child(head("subtitles")).child(right)),
                        )
                        .when(subs_on, |c| c.child(timing))
                        .into_any_element(),
                )
            }
            // episodes: their pictures, a season at a time
            Menu::Episodes => {
                let Some(f) = p.full.clone() else { return div().into_any_element() };
                let playing = playback::ordered(&f).into_iter().find(|v| v["id"].as_str() == Some(p.video_id.as_str())).and_then(|v| v["season"].as_i64()).unwrap_or(1);
                let cur = self.ep_season.unwrap_or(playing);
                let mut seasons: Vec<i64> = playback::ordered(&f).into_iter().filter_map(|v| v["season"].as_i64()).collect();
                seasons.sort();
                seasons.dedup();
                let meta = p.meta.clone();
                let bg = meta.as_ref().and_then(|m| m.background.clone()).map(|u| sized(&u, "w300"));
                let picker = (seasons.len() > 1).then(|| {
                    div()
                        .id("ep-seasons")
                        .px(px(6.))
                        .pb(px(8.))
                        .flex()
                        .gap(px(6.))
                        .overflow_x_scroll()
                        .children(seasons.iter().map(|n| {
                            let n = *n;
                            div()
                                .id(("ep-s", n as usize))
                                .flex_none()
                                .px(px(12.))
                                .h(px(30.))
                                .flex()
                                .items_center()
                                .rounded_full()
                                .cursor_pointer()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .when(n == cur, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
                                .when(n != cur, |d| d.bg(theme::white(0.1)).hover(|s| s.bg(theme::white(0.18))))
                                .on_click(cx.listener(move |v, _, _, cx| {
                                    cx.stop_propagation();
                                    v.ep_season = Some(n);
                                    cx.notify();
                                }))
                                .child(if n == 0 { "specials".to_string() } else { format!("season {n}") })
                        }))
                });
                // what's been watched, and where you stopped (the one playing: live)
                let lib = meta.as_ref().and_then(|m| couch::library::get(&couch::state::get().core, &m.id));
                let seen_ids: Vec<String> = lib.as_ref().and_then(|i| serde_json::from_value(i["state"]["watchedIds"].clone()).ok()).unwrap_or_default();
                let lib_at = lib.as_ref().map(|i| (i["state"]["video_id"].as_str().unwrap_or("").to_string(), i["state"]["timeOffset"].as_f64().unwrap_or(0.), i["state"]["duration"].as_f64().unwrap_or(0.)));
                let live = (p.state.duration > 0.).then(|| (p.state.pos / p.state.duration).clamp(0., 1.));
                let rows = playback::ordered(&f).into_iter().filter(|v| v["season"].as_i64() == Some(cur)).map(|v| {
                    let id = v["id"].as_str().unwrap_or("").to_string();
                    let on = id == p.video_id;
                    let prog = if on {
                        live.unwrap_or(0.)
                    } else {
                        lib_at.as_ref().filter(|l| l.0 == id && l.2 > 0.).map_or(0., |l| (l.1 / l.2).clamp(0., 1.))
                    };
                    // under 10% to go counts as watched
                    let seen = seen_ids.contains(&id) || prog >= 0.9;
                    let num = v["episode"].as_i64().or(v["number"].as_i64()).unwrap_or(0);
                    let name = playback::episode_name(&v);
                    let over = v["overview"].as_str().or(v["description"].as_str()).unwrap_or("").to_string();
                    let imdb = crate::api::imdb_episode(&id);
                    let still = v["thumbnail"].as_str().map(String::from).or(imdb.clone()).or(bg.clone());
                    let alt = imdb.clone().or(bg.clone());
                    let m = meta.clone();
                    let i2 = id.clone();
                    div()
                        .id(SharedString::from(format!("ep-{id}")))
                        .flex()
                        .gap(px(12.))
                        .p(px(6.))
                        .rounded(px(12.))
                        .cursor_pointer()
                        .when(on, |d| d.bg(theme::white(0.1)))
                        .hover(|s| s.bg(theme::white(0.08)))
                        .on_click(move |_, _, cx| {
                            if let Some(m) = m.clone() {
                                crate::Nav::play_episode(&m, i2.clone(), cx);
                            }
                        })
                        .child(
                            div()
                                .relative()
                                .w(px(128.))
                                .h(px(72.))
                                .flex_none()
                                .rounded(px(12.))
                                .overflow_hidden()
                                .bg(theme::white(0.06))
                                .children(still.map(|u| {
                                    let alt = alt.clone().filter(|a| a != &u);
                                    img(u).size_full().rounded(px(12.)).object_fit(ObjectFit::Cover).with_fallback(move || match alt.clone() {
                                        Some(a) => img(a).size_full().rounded(px(12.)).object_fit(ObjectFit::Cover).into_any_element(),
                                        None => div().into_any_element(),
                                    })
                                }))
                                // watched: the soft shade from the top right, and a tick
                                .when(seen && !on, |d| {
                                    d.child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(225., linear_color_stop(theme::black(0.6), 0.), linear_color_stop(theme::black(0.0), 0.65))))
                                        .child(div().absolute().top(px(5.)).right(px(5.)).size(px(20.)).rounded_full().bg(theme::black(0.55)).flex().items_center().justify_center().child(icon("check", 12.)))
                                })
                                // started, not finished: how far, along the bottom
                                .when(!seen && prog > 0.01, |d| {
                                    d.child(div().absolute().left_0().bottom_0().w_full().h(px(3.)).bg(theme::white(0.25)).child(div().h_full().w(px(128. * prog as f32)).bg(gpui::white())))
                                })
                                .when(on, |d| d.child(div().absolute().size_full().flex().items_center().justify_center().bg(theme::black(0.4)).child(icon_fill("play", 22.)))),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .justify_center()
                                .gap(px(2.))
                                .child(div().w_full().truncate().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(format!("{num}. {name}")))
                                .children((!over.is_empty()).then(|| div().w_full().text_size(px(12.)).line_height(px(16.)).text_color(theme::label3()).line_clamp(2).child(over))),
                        )
                        .into_any_element()
                });
                (
                    460.,
                    div()
                        .flex()
                        .flex_col()
                        .child(head("episodes"))
                        .children(picker)
                        .child(faded(div().id("ep-list").max_h(px(440.)).overflow_y_scroll().track_scroll(&self.ms[2]).child(crate::ui::smooth::wheel(&self.ms[2], "ep-list")).flex().flex_col().gap(px(2.)).children(rows), &self.ms[2]))
                        .into_any_element(),
                )
            }
            // speed, the sleep timer and the picture
            Menu::Speed => {
                const SPEEDS: [f64; 7] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
                let cur = snap(p.state.speed);
                let si = SPEEDS.iter().position(|v| (v - cur).abs() < 0.01).unwrap_or(2);
                let series = p.meta.as_ref().map_or(false, |m| m.kind == "series");
                let mut sleeps: Vec<(i64, &str)> = vec![(0, "off")];
                if series {
                    sleeps.push((-1, "ep"));
                }
                sleeps.extend([(15, "15"), (30, "30"), (45, "45"), (60, "60"), (90, "90"), (120, "120")]);
                let sl_now = match p.sleep {
                    None => 0,
                    Some(playback::Sleep::Episode) => -1,
                    Some(playback::Sleep::At(_, m)) => m,
                };
                let sl_i = sleeps.iter().position(|s| s.0 == sl_now).unwrap_or(0);
                let sl_text = match p.sleep {
                    None => "off".to_string(),
                    Some(playback::Sleep::Episode) => "after this episode".to_string(),
                    Some(playback::Sleep::At(at, _)) => format!("pauses in {} min", (at.saturating_duration_since(Instant::now()).as_secs_f64() / 60.).ceil() as i64),
                };

                let speed = self.stops(
                    "sp",
                    SPEEDS.iter().map(|v| if v.fract() == 0. || *v == 0.5 || *v == 1.5 { format!("{v}") } else { String::new() }).collect(),
                    si,
                    std::rc::Rc::new(|v: &mut Self, i: usize, cx: &mut Context<Self>| {
                        if may("speed", None, cx) {
                            v.pb.read(cx).cmd("speed", json!({ "value": SPEEDS[i] }));
                        }
                    }),
                    window,
                    cx,
                );
                let vals: Vec<i64> = sleeps.iter().map(|s| s.0).collect();
                let sleep = self.stops(
                    "sl",
                    sleeps.iter().map(|s| s.1.to_string()).collect(),
                    sl_i,
                    std::rc::Rc::new(move |v: &mut Self, i: usize, cx: &mut Context<Self>| {
                        let n = vals[i];
                        v.pb.update(cx, |p, cx| p.set_sleep(n, cx));
                    }),
                    window,
                    cx,
                );
                let label = |t: &str, val: String| div().px(px(4.)).flex().justify_between().items_baseline().child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child(t.to_string())).child(div().text_size(px(17.)).font_weight(FontWeight::BLACK).child(val));
                let picture = div().flex().gap(px(8.)).children([("fit", "fit"), ("fill", "fill"), ("stretch", "stretch")].into_iter().map(|(k, t)| {
                    let on = self.aspect == k;
                    div()
                        .id(SharedString::from(format!("as-{k}")))
                        .flex_1()
                        .py(px(10.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(12.))
                        .cursor_pointer()
                        .when(on, |d| d.bg(theme::white(0.16)).border_1().border_color(theme::white(0.3)))
                        .when(!on, |d| d.bg(theme::white(0.05)).hover(|s| s.bg(theme::white(0.1))))
                        .on_click(cx.listener(move |me, _, _, cx| {
                            me.aspect = k;
                            me.pb.read(cx).cmd("aspect", json!({ "value": k }));
                            cx.notify();
                        }))
                        .child(aspect_icon(k, on))
                        .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(if on { theme::text().into() } else { theme::label2() }).child(t))
                }));
                (
                    360.,
                    div()
                        .px(px(8.))
                        .py(px(6.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(label("speed", if (cur - 1.).abs() < 0.01 { "normal".into() } else { speed_label(cur) }))
                        .child(speed)
                        .child(div().h(px(1.)).my(px(6.)).bg(theme::white(0.08)))
                        .child(label("sleep timer", sl_text))
                        .child(sleep)
                        .child(div().h(px(1.)).my(px(6.)).bg(theme::white(0.08)))
                        .child(label("picture", String::new()))
                        .child(picture)
                        // immersive light: the film's colours glowing in the black bars
                        .child({
                            let on = couch::state::get().core.db.get::<bool>("gpui_immersive").unwrap_or(true);
                            div()
                                .id("pl-immersive")
                                .mt(px(4.))
                                .flex()
                                .items_center()
                                .justify_between()
                                .px(px(4.))
                                .h(px(36.))
                                .cursor_pointer()
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    let _ = couch::state::get().core.db.set("gpui_immersive", &!on);
                                    cx.notify();
                                }))
                                .child(div().flex().flex_col().child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child("immersive light")).child(div().text_size(px(12.)).text_color(theme::label3()).child("the film's colours glow in the black bars")))
                                .child(
                                    div()
                                        .w(px(38.))
                                        .h(px(22.))
                                        .rounded_full()
                                        .p(px(2.))
                                        .flex()
                                        .when(on, |d| d.justify_end().bg(theme::text()))
                                        .when(!on, |d| d.bg(theme::white(0.18)))
                                        .child(div().size(px(18.)).rounded_full().bg(if on { gpui::rgb(0x000000) } else { gpui::rgb(0xffffff) })),
                                )
                        })
                        .into_any_element(),
                )
            }
            Menu::Sources => {
                // every stream found, best first; the one playing is ticked
                // (not AIOStreams' 🦆 ones)
                let cur = p.stream.as_ref().map(playback::stream_key).unwrap_or_default();
                let duck = |s: &serde_json::Value| {
                    let text = [&s["name"], &s["title"], &s["description"], &s["_addonName"]].iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" ");
                    text.contains('🦆') || text.to_lowercase().contains("aiostreams")
                };
                let rows = p
                    .all
                    .iter()
                    .filter(|s| !duck(s))
                    .take(40)
                    .enumerate()
                    .map(|(i, s)| {
                        let c = &s["_couch"];
                        let mut bits: Vec<String> = vec![];
                        bits.push(c["res"].as_str().filter(|r| *r != "Unknown").unwrap_or("?").to_string());
                        if let Some(h) = c["hdr"].as_array().and_then(|h| h.first()).and_then(|h| h.as_str()) {
                            bits.push(h.to_string());
                        }
                        if let Some(b) = c["sizeBytes"].as_f64() {
                            bits.push(format!("{:.1} GB", b / 1e9));
                        }
                        if c["cached"].as_bool() == Some(true) {
                            bits.push("⚡ instant".into());
                        } else if let Some(n) = c["seeders"].as_u64() {
                            bits.push(format!("{n} seeds"));
                        }
                        let from = s["_addonName"].as_str().unwrap_or("").to_string();
                        let key = playback::stream_key(s);
                        let on = key == cur;
                        let pb = self.pb.clone();
                        let st = s.clone();
                        div()
                            .id(SharedString::from(format!("src-{i}")))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(10.))
                            .cursor_pointer()
                            .hover(|d| d.bg(theme::white(0.1)))
                            .child(div().w(px(18.)).flex_none().children(on.then(|| icon("check", 18.))))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(div().truncate().text_size(px(15.)).font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM }).child(bits.join(" · ")))
                                    .child(div().text_size(px(12.)).text_color(theme::label3()).truncate().child(from)),
                            )
                            .on_click(move |_, _, cx| {
                                if !on {
                                    pb.update(cx, |p, cx| p.pick(st.clone(), cx));
                                }
                            })
                            .into_any_element()
                    })
                    .collect::<Vec<_>>();
                (340., div().flex().flex_col().child(head("streams")).child(faded(div().id("src-list").max_h(px(430.)).overflow_y_scroll().track_scroll(&self.ms[3]).child(crate::ui::smooth::wheel(&self.ms[3], "src-list")).flex().flex_col().children(rows), &self.ms[3])).into_any_element())
            }
            Menu::Couch => {
                let t = crate::together::get(cx);
                let tr = t.read(cx);
                let shared = tr.shared;
                let mut rows: Vec<gpui::AnyElement> = vec![];
                // the live list from the server; until it arrives, who the room says is here
                let roster: Vec<crate::together::Guest> = if !tr.roster.is_empty() {
                    tr.roster.clone()
                } else {
                    self.reactions
                        .read(cx)
                        .people
                        .iter()
                        .filter(|x| x["host"] != true)
                        .map(|x| crate::together::Guest {
                            id: x["id"].as_str().unwrap_or("").into(),
                            name: x["name"].as_str().unwrap_or("someone").into(),
                            avatar: x["avatar"].as_str().map(String::from),
                            live: true,
                            remote: shared,
                            buf: false,
                        })
                        .collect()
                };
                for (i, g) in roster.iter().enumerate() {
                    let (t1, t2, id1, id2, remote) = (t.clone(), t.clone(), g.id.clone(), g.id.clone(), g.remote);
                    let status = if !g.live { "reconnecting…" } else if g.buf { "loading, you'll wait for them" } else if g.remote { "has the remote" } else { "watching" };
                    let dot = if !g.live { gpui::rgb(0x8a8a8a) } else if g.buf { gpui::rgb(0xffc83d) } else { gpui::rgb(0x3ddc84) };
                    rows.push(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px(px(10.))
                            .py(px(8.))
                            .rounded(px(12.))
                            .child(
                                div().relative().size(px(36.)).flex_none()
                                    .child(match &g.avatar {
                                        Some(a) => img(a.clone()).size(px(36.)).rounded_full().into_any_element(),
                                        None => div().size(px(36.)).rounded_full().bg(theme::neon()).flex().items_center().justify_center().font_weight(FontWeight::BLACK).child(g.name.chars().next().unwrap_or('?').to_uppercase().to_string()).into_any_element(),
                                    })
                                    .child(div().absolute().right(px(-1.)).bottom(px(-1.)).size(px(12.)).rounded_full().bg(dot).border_2().border_color(gpui::rgb(0x151114))),
                            )
                            .child(div().flex_1().min_w_0().flex().flex_col().child(div().truncate().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(g.name.clone())).child(div().text_size(px(12.)).text_color(theme::label3()).child(status)))
                            .when(!shared, |d| {
                                d.child(
                                    div()
                                        .id(SharedString::from(format!("cr-rm-{i}")))
                                        .h(px(30.))
                                        .px(px(11.))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .gap(px(5.))
                                        .cursor_pointer()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .when(remote, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
                                        .when(!remote, |d| d.bg(theme::white(0.1)).hover(|s| s.bg(theme::white(0.16))))
                                        .child(if remote { "remote ✓" } else { "give remote" })
                                        .on_click(move |_, _, cx| t1.update(cx, |t, cx| t.give_remote(&id1, !remote, cx))),
                                )
                            })
                            .child(
                                div()
                                    .id(SharedString::from(format!("cr-kick-{i}")))
                                    .size(px(30.))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .bg(theme::white(0.08))
                                    .hover(|s| s.bg(gpui::rgba(0xff4d5a55)))
                                    .child(icon("signout", 15.))
                                    .on_click(move |_, _, cx| t2.update(cx, |t, cx| t.kick(&id2, cx))),
                            )
                            .into_any_element(),
                    );
                }
                if rows.is_empty() {
                    rows.push(div().px(px(12.)).py(px(10.)).text_size(px(14.)).text_color(theme::label2()).child("nobody's on your couch right now").into_any_element());
                }
                let t3 = t.clone();
                let everyone = div()
                    .id("cr-all")
                    .mt(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .h(px(42.))
                    .rounded(px(12.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::white(0.08)))
                    .child(div().w(px(18.)).flex_none().children(shared.then(|| icon("check", 18.))))
                    .child(div().text_size(px(15.)).font_weight(FontWeight::MEDIUM).child("everyone gets the remote"))
                    .on_click(move |_, _, cx| t3.update(cx, |t, cx| t.set_shared(!shared, cx)));
                (360., div().flex().flex_col().child(head("on your couch")).children(rows).child(div().mx(px(10.)).my(px(4.)).h(px(1.)).bg(theme::white(0.08))).child(everyone).into_any_element())
            }
        };
        div()
            .id("pl-menu")
            .occlude()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .absolute()
            .right(px(theme::GUTTER))
            .bottom(px(100.))
            .w(px(width))
            .p(px(8.))
            .rounded(px(22.))
            .bg(gpui::rgba(0x151114f2))
            .border_1()
            .border_color(theme::white(0.10))
            .shadow(vec![gpui::BoxShadow { color: theme::black(0.5), offset: gpui::point(px(0.), px(14.)), blur_radius: px(40.), spread_radius: px(0.), inset: false }])
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(body)
            .into_any_element()
    }

    /// A row of stops to slide along (speed, the sleep timer): click or drag.
    fn stops(
        &self,
        key: &'static str,
        labels: Vec<String>,
        at: usize,
        set: std::rc::Rc<dyn Fn(&mut Self, usize, &mut Context<Self>)>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        const W: f32 = 300.;
        let n = labels.len().max(2);
        let step = W / (n - 1) as f32;
        let origin: std::rc::Rc<std::cell::Cell<f32>> = Default::default();
        let o2 = origin.clone();
        let pick = move |x: f32| (((x).clamp(0., W)) / step).round() as usize;
        let (s1, s2) = (set.clone(), set.clone());
        let (oa, ob) = (origin.clone(), origin.clone());
        let fx = at as f32 * step;
        div()
            .id(key)
            .relative()
            .w(px(W + 20.))
            .h(px(46.))
            .px(px(10.))
            .cursor_pointer()
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |v, e: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                let i = pick(f32::from(e.position.x) - oa.get());
                s1(v, i, cx);
                cx.notify();
            }))
            .on_mouse_move(cx.listener(move |v, e: &MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(gpui::MouseButton::Left) {
                    let i = pick(f32::from(e.position.x) - ob.get());
                    s2(v, i, cx);
                    cx.notify();
                }
            }))
            .child(
                div()
                    .absolute()
                    .left(px(10.))
                    .top(px(10.))
                    .w(px(W))
                    .h(px(6.))
                    .rounded_full()
                    .bg(theme::white(0.14))
                    .child(gpui::canvas(move |b, _, _| o2.set(f32::from(b.origin.x)), |_, _, _, _| {}).absolute().size_full())
                    .child(div().absolute().h_full().w(px(fx)).rounded_full().bg(theme::neon())),
            )
            // the stops, and their names under them
            .children(labels.into_iter().enumerate().map(move |(i, l)| {
                let x = 10. + i as f32 * step;
                div()
                    .absolute()
                    .left(px(x - 20.))
                    .top(px(8.))
                    .w(px(40.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.))
                    .child(div().size(px(10.)).rounded_full().bg(if i <= at { theme::neon().into() } else { theme::white(0.3) }))
                    .child(div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(if i == at { theme::text().into() } else { theme::label3() }).child(l))
            }))
            .child(div().absolute().left(px(10. + fx - 9.)).top(px(4.)).size(px(18.)).rounded_full().bg(theme::text()).shadow(vec![gpui::BoxShadow { color: theme::black(0.5), offset: gpui::point(px(0.), px(2.)), blur_radius: px(6.), spread_radius: px(0.), inset: false }]))
            .into_any_element()
    }
}

impl Render for PlayerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pb.read(cx);
        let vw = window.viewport_size().width;
        self.view_h = f32::from(window.viewport_size().height);
        let g = theme::GUTTER;
        let bar_w: Pixels = vw - px(g * 2.);
        let st = p.state.clone();
        // the film's shape, for the picture-in-picture window's resizing
        if st.dw > 0. && st.dh > 0. {
            crate::pip::set_aspect((st.dw / st.dh) as f32);
        }
        let frac = if st.duration > 0. { (st.pos / st.duration).clamp(0., 1.) as f32 } else { 0. };
        let ahead = if st.duration > 0. { ((st.pos + st.cache_ahead) / st.duration).clamp(0., 1.) as f32 } else { 0. };
        let title = p.meta.as_ref().map(|m| m.name.clone()).unwrap_or_default();
        let ep = p.episode_title.clone();
        let label = p.full.as_ref().and_then(|f| {
            let v = playback::ordered(f).into_iter().find(|v| v["id"].as_str() == Some(p.video_id.as_str()))?;
            Some(format!("S{} E{}", v["season"].as_i64()?, v["episode"].as_i64().or(v["number"].as_i64())?))
        });
        let shown = self.shown || p.loading || !p.error.is_empty();
        let is_series_now = p.meta.as_ref().map_or(false, |m| m.kind == "series");
        let art = p.meta.as_ref().and_then(|m| m.background.clone().or(m.poster.clone())).map(|u| sized(&u, "original"));
        let logo = p.meta.as_ref().and_then(|m| m.logo.clone()).map(|u| crate::net::logo(&sized(&u, "large"), 480.));
        // the loading screen until the picture is really there (never see-through to the desktop)
        let starting = p.loading || (p.open && st.duration <= 0. && p.error.is_empty());

        let mut root = div()
            .id("player")
            .size_full()
            .relative()
            .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _, cx| {
                v.mouse_now = f32::from(e.position.y);
                if v.drag {
                    if e.pressed_button == Some(gpui::MouseButton::Left) {
                        v.hover_x = Some(f32::from(e.position.x) - v.scrub_at.get().0);
                        cx.notify();
                    } else {
                        v.finish_drag(cx);
                    }
                }
                v.poke(cx)
            }))
            // holding the picture down: 2× while held
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|v, _, _, cx| {
                if !v.drag {
                    hold_begin(cx);
                }
            }))
            .on_mouse_up(gpui::MouseButton::Left, cx.listener(|v, _, _, cx| {
                if hold_end(cx) {
                    v.ate_click = true;
                }
                v.finish_drag(cx)
            }))
            .on_click(cx.listener(|v, e: &gpui::ClickEvent, window, cx| {
                if just_moved() {
                    return;
                }
                // that was a hold for 2×, not a click
                if std::mem::take(&mut v.ate_click) | hold_ate_click() {
                    return;
                }
                if e.click_count() == 2 {
                    // double click: full screen; the first click's pause never happens
                    v.click_gen += 1;
                    window.toggle_fullscreen();
                    return;
                }
                // a click on the picture: play / pause (double click: full screen)
                if v.menu.is_some() || v.vol_pinned {
                    v.menu = None;
                    v.vol_pinned = false;
                    cx.notify();
                } else if e.click_count() == 1 && !crate::together::get(cx).read(cx).locked() {
                    // wait a moment: a second click makes it full screen instead
                    v.click_gen += 1;
                    let g = v.click_gen;
                    cx.spawn(async move |this, cx| {
                        crate::timer::after(Duration::from_millis(320)).await;
                        let _ = this.update(cx, |v, cx| {
                            if v.click_gen != g || boosting() {
                                return;
                            }
                            let paused = v.pb.read(cx).state.paused;
                            if may(if paused { "play" } else { "pause" }, None, cx) {
                                v.pb.read(cx).cmd("pause", json!({ "value": !paused }));
                            }
                        });
                    })
                    .detach();
                }
                v.poke(cx);
            }))
            .on_scroll_wheel(cx.listener(|v, e: &gpui::ScrollWheelEvent, w, cx| {
                // scroll = volume (not while a menu is open: that scrolls the menu)
                if v.menu.is_some() {
                    return;
                }
                // 5% a notch. A mouse wheel sends whole notches (sometimes as 3
                // "lines" each, sometimes as 120 px): count notches, not their size,
                // so one click of the wheel is never a jump to 0 or 100
                let change = match e.delta {
                    gpui::ScrollDelta::Lines(l) => {
                        v.vol_acc = 0.;
                        if l.y.abs() < 0.01 {
                            return;
                        }
                        5. * l.y.signum() as f64
                    }
                    gpui::ScrollDelta::Pixels(p) => {
                        // a touchpad: smooth, 5% per 60 px of travel
                        v.vol_acc += f32::from(p.y) as f64 / 60.;
                        if v.vol_acc.abs() < 1. {
                            return;
                        }
                        let n = v.vol_acc.trunc();
                        v.vol_acc -= n;
                        5. * n.clamp(-2., 2.)
                    }
                };
                let _ = w;
                let p = v.pb.read(cx);
                let max = if couch::state::get().core.settings().playback.volume_boost { 130. } else { 100. };
                // build on the volume we last asked for (mpv's reply lags behind fast spins)
                let base = v.vol_target.filter(|_| v.vol_hud.map_or(false, |t| t.elapsed() < Duration::from_millis(800))).unwrap_or(p.state.volume);
                // on the fives, so it lands on 50, 55, 60…
                let vol = (((base + change) / 5.).round() * 5.).clamp(0., max);
                v.vol_target = Some(vol);
                p.cmd("volume", json!({ "value": vol }));
                v.vol_hud = Some(Instant::now());
                cx.notify();
            }));

        // the progress bar takes its colours from the title's logo (or its picture)
        let tint_src = p.meta.as_ref().and_then(|m| m.logo.clone().or(m.background.clone()));
        if let Some(src) = tint_src.filter(|u| self.tint.as_ref().map_or(true, |t| &t.0 != u)) {
            self.tint = Some((src.clone(), [theme::neon().into(), theme::neon().into()]));
            PLAY_TINT.with(|c| c.set(None));
            let back = p.meta.as_ref().and_then(|m| m.background.clone());
            cx.spawn(async move |this, cx| {
                let found = couch::rt::spawn(async move {
                    let core = couch::state::get().core.clone();
                    for u in [Some(src.clone()), back].into_iter().flatten() {
                        if let Ok((b, _)) = couch::images::load(&core, &sized(&u, "w500")).await {
                            if let Some(t) = tint_of(&b) {
                                return Some((src.clone(), t));
                            }
                        }
                    }
                    None
                })
                .await
                .ok()
                .flatten();
                if let Some((k, t)) = found {
                    let _ = this.update(cx, |v, cx| {
                        if v.tint.as_ref().map_or(false, |x| x.0 == k) {
                            set_last_tint(t);
                            PLAY_TINT.with(|c| c.set(Some(t)));
                            v.tint = Some((k, t));
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
        // immersive light: the picture's own colours, softly, in the black bars around it
        if !starting && st.dw > 0. && st.dh > 0. && self.aspect == "fit" && couch::state::get().core.db.get::<bool>("gpui_immersive").unwrap_or(true) {
            if let Some(cur) = self.glow.clone() {
                let vs = window.viewport_size();
                let (ww, wh) = (f32::from(vs.width), f32::from(vs.height));
                let a = (st.dw / st.dh) as f32;
                let fade = (self.glow_at.elapsed().as_secs_f32() / 1.2).min(1.);
                if fade < 1. {
                    window.request_animation_frame();
                }
                let bars: Vec<gpui::Bounds<Pixels>> = if ww / wh > a + 0.01 {
                    let pw = (ww - wh * a) / 2.;
                    if pw < 6. { vec![] } else { vec![gpui::Bounds::new(gpui::point(px(0.), px(0.)), gpui::size(px(pw + 1.), px(wh))), gpui::Bounds::new(gpui::point(px(ww - pw - 1.), px(0.)), gpui::size(px(pw + 1.), px(wh)))] }
                } else {
                    let ph = (wh - ww / a) / 2.;
                    if ph < 6. { vec![] } else { vec![gpui::Bounds::new(gpui::point(px(0.), px(0.)), gpui::size(px(ww), px(ph + 1.))), gpui::Bounds::new(gpui::point(px(0.), px(wh - ph - 1.)), gpui::size(px(ww), px(ph + 1.)))] }
                };
                if !bars.is_empty() {
                    let layer = |pic: std::sync::Arc<gpui::Image>, bars: Vec<gpui::Bounds<Pixels>>| {
                        gpui::canvas(
                            |_, _, _| (),
                            move |b, _, window, cx| {
                                let Some(data) = pic.clone().use_render_image(window, cx) else { return };
                                for r in &bars {
                                    let _ = window.paint_image(*r, b, gpui::Corners::default(), data.clone(), 0, false);
                                }
                            },
                        )
                        .absolute()
                        .size_full()
                    };
                    root = root
                        .children(self.glow_prev.clone().filter(|_| fade < 1.).map(|p| div().absolute().top_0().left_0().size_full().opacity(1. - fade).child(layer(p, bars.clone()))))
                        .child(div().absolute().top_0().left_0().size_full().opacity(fade).child(layer(cur, bars)));
                }
            }
        }
        // while it starts: the title's art under a dim, the logo, a line
        if starting || !p.error.is_empty() {
            root = root
                .bg(theme::bg())
                .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).opacity(0.28)))
                .child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(180., linear_color_stop(theme::black(0.2), 0.), linear_color_stop(theme::black(0.85), 1.))))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(14.))
                        .child(match logo.clone() {
                            Some(u) => {
                                let t = title.clone();
                                img(u).max_w(px(420.)).max_h(px(130.)).object_fit(ObjectFit::Contain).with_fallback(move || div().text_size(px(48.)).font_weight(FontWeight::BLACK).child(t.clone()).into_any_element()).into_any_element()
                            }
                            None => div().text_size(px(48.)).font_weight(FontWeight::BLACK).child(title.clone()).into_any_element(),
                        })
                        // the couch, turning
                        .when(p.error.is_empty(), |d| {
                            window.request_animation_frame();
                            d.child(div().w(px(150.)).h(px(135.)).children(crate::couch_loading_frame().map(|f| img(gpui::ImageSource::Render(f)).size_full().object_fit(ObjectFit::Contain))))
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .children(self.signal.filter(|_| p.error.is_empty()).map(|n| {
                                    div().flex().items_end().gap(px(3.)).h(px(16.)).children((0..4u8).map(move |i| {
                                        div().w(px(4.)).h(px(5. + 3.7 * i as f32)).rounded(px(1.5)).bg(if i < n { theme::text().into() } else { theme::white(0.25) })
                                    }))
                                }))
                                .child(
                                    div()
                                        .text_size(px(16.))
                                        .text_color(if p.error.is_empty() { theme::label2() } else { gpui::rgb(0xff8a8a) })
                                        .child(if p.error.is_empty() { p.status.clone() } else { p.error.clone() }),
                                ),
                        )
                        // not this one? choose
                        .when(p.all.len() > 1, |d| {
                            d.child(
                                div()
                                    .id("pl-choose")
                                    .occlude()
                                    .px(px(16.))
                                    .h(px(36.))
                                    .flex()
                                    .items_center()
                                    .rounded_full()
                                    .bg(theme::white(0.12))
                                    .hover(|s| s.bg(theme::white(0.2)))
                                    .cursor_pointer()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                    .on_click(cx.listener(|v, _, _, cx| {
                                        cx.stop_propagation();
                                        v.menu = Some(Menu::Sources);
                                        cx.notify();
                                    }))
                                    .child(format!("choose a stream ({})", p.all.len())),
                            )
                        }),
                );
        }

        // subtitles, drawn by couch, the way they're set up in settings
        if !p.sub.is_empty() {
            let ss = couch::state::get().core.settings().subtitles.clone();
            let vh = f32::from(window.viewport_size().height);
            // above the controls while they show; otherwise where settings put them
            let lift = if crate::pip::is_on() || f32::from(vw) < 640. { 14. } else if shown { 160f32.max(vh * ss.bottom_offset / 100.) } else { vh * ss.bottom_offset / 100. };
            root = root.child(div().absolute().left_0().w_full().bottom(px(lift)).flex().justify_center().child(sub_block(&p.sub, f32::from(vw), &ss)));
        }

        if let Some(at) = self.vol_hud.filter(|a| a.elapsed() < Duration::from_millis(1200)) {
            let _ = at;
            root = root.child(
                div().absolute().top(px(90.)).left_0().w_full().flex().justify_center().child(
                    div()
                        .px(px(18.))
                        .py(px(10.))
                        .rounded_full()
                        .bg(gpui::rgba(0x151114cc))
                        .border_1()
                        .border_color(theme::white(0.10))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(icon(if st.volume <= 0. { "mute" } else { "volume" }, 22.))
                        .child(div().w(px(120.)).h(px(4.)).rounded_full().bg(theme::white(0.2)).child(div().h_full().rounded_full().bg(theme::text()).w(px(120. * (self.vol_target.unwrap_or(st.volume) / 100.).clamp(0., 1.) as f32))))
                        .child(div().w(px(34.)).font_weight(FontWeight::SEMIBOLD).child(format!("{}", self.vol_target.unwrap_or(st.volume).round() as i64))),
                ),
            );
            cx.spawn(async move |this, cx| {
                crate::timer::after(Duration::from_millis(1300)).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        let mut root = root.child(self.reactions.clone()).children(denied_sign(cx, window));
        // held for 2×: a small sign at the top
        if boosting() {
            root = root.child(
                div().absolute().top(px(40.)).left_0().w_full().flex().justify_center().child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .h(px(40.))
                        .px(px(16.))
                        .rounded_full()
                        .bg(gpui::rgba(0x151114d9))
                        .border_1()
                        .border_color(theme::white(0.1))
                        .text_size(px(15.))
                        .font_weight(FontWeight::BLACK)
                        .child(icon("forward", 18.))
                        .child("2×"),
                ),
            );
        }
        // the controls slide in and out
        if shown != self.chrome_on {
            self.chrome_on = shown;
            self.chrome_at = Instant::now();
        }
        let ct = self.chrome_at.elapsed().as_secs_f32();
        // in: quick then soft (ease-out quint); out: a slow, even fade (ease-in-out)
        let ca = if self.chrome_on {
            let x = (ct / 0.42).min(1.);
            1. - (1. - x).powi(5)
        } else {
            let x = (ct / 0.6).min(1.);
            1. - if x < 0.5 { 4. * x * x * x } else { 1. - (-2. * x + 2.).powi(3) / 2. }
        };
        if ct < 0.62 {
            window.request_animation_frame();
        }
        // paused: which episode, big, bottom left; each part rises in after the last
        // (not in picture-in-picture: too small for it)
        let want_pause = st.paused && !starting && self.pb.read(cx).error.is_empty() && self.menu.is_none() && !crate::pip::is_on() && f32::from(vw) >= 640.;
        if want_pause != self.pause_on {
            self.pause_on = want_pause;
            self.pause_at = Instant::now();
        }
        let pt = self.pause_at.elapsed().as_secs_f32();
        if pt < 1.3 {
            window.request_animation_frame();
        }
        let ein = |d: f32, dur: f32| 1. - (1. - ((pt - d) / dur).clamp(0., 1.)).powi(5);
        let eout = |d: f32, dur: f32| {
            let x = ((pt - d) / dur).clamp(0., 1.);
            1. - if x < 0.5 { 4. * x * x * x } else { 1. - (-2. * x + 2.).powi(3) / 2. }
        };
        // (shade, S:E, title, logo, the top logo stepping away)
        let (pv_shade, pv_label, pv_title, pv_logo, pv_top) = if self.pause_on {
            (ein(0., 0.7), ein(0.12, 0.7), ein(0.22, 0.75), ein(0.42, 0.8), ein(0., 0.35))
        } else {
            (eout(0.1, 0.6), eout(0.16, 0.45), eout(0.08, 0.45), eout(0., 0.45), eout(0.25, 0.45))
        };
        // the picture blurs in with the shade (mpv draws it; steps of 5% are plenty)
        {
            let want = (pv_shade * 20.).round() / 20.;
            if (want - self.blur_sent).abs() > 0.001 {
                self.blur_sent = want;
                self.pb.read(cx).cmd("pause-blur", json!({ "value": want }));
            }
        }
        if pv_shade > 0.001 {
            let has_ep = is_series_now && (label.is_some() || !ep.is_empty());
            let big_logo = logo.clone();
            let t2 = title.clone();
            let rise = |v: f32| px((1. - v) * 34.);
            root = root
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .opacity(pv_shade)
                        .bg(linear_gradient(40., linear_color_stop(theme::black(0.88), 0.), linear_color_stop(theme::black(0.0), 0.62))),
                )
                // left alone while paused (the controls gone): the lights go down a little more
                .child(div().absolute().top_0().left_0().size_full().bg(theme::black(0.32)).opacity(pv_shade * (1. - ca)))
                .child(
                    div()
                        .absolute()
                        .left(px(g))
                        .bottom(px(150.))
                        .max_w(px(900.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .when(has_ep, |d| {
                            d.children(label.clone().map(|l| {
                                div().relative().top(rise(pv_label)).opacity(pv_label).text_size(px(28.)).line_height(px(38.)).font_weight(FontWeight::BLACK).text_color(theme::white(0.72)).child(l.replace(' ', ":"))
                            }))
                            .children((!ep.is_empty()).then(|| {
                                div().relative().top(rise(pv_title)).opacity(pv_title).text_size(px(44.)).line_height(px(54.)).pb(px(4.)).font_weight(FontWeight::BLACK).line_clamp(2).child(ep.clone())
                            }))
                        })
                        .child(div().relative().top(rise(pv_logo)).opacity(pv_logo).mt(px(if has_ep { 18. } else { 0. })).child(match big_logo {
                            Some(u) => img(u)
                                .h(px(if has_ep { 110. } else { 170. }))
                                .max_w(px(520.))
                                .object_fit(ObjectFit::Contain)
                                .with_fallback(move || div().text_size(px(56.)).font_weight(FontWeight::BLACK).child(t2.clone()).into_any_element())
                                .into_any_element(),
                            None => div().text_size(px(if has_ep { 36. } else { 64. })).font_weight(FontWeight::BLACK).child(title.clone()).into_any_element(),
                        })),
                );
        }
        if ca <= 0.001 {
            return root;
        }

        // small window (picture in picture): just play/pause, ±10, a thin bar, and back to big
        // picture in picture (any size it's dragged to), or just a tiny window
        if crate::pip::is_on() || f32::from(vw) < 640. {
            let paused = st.paused;
            let (pa, pb, pc) = (self.pb.clone(), self.pb.clone(), self.pb.clone());
            return root
                // the picture itself moves the little window around
                .child(div().absolute().top_0().left_0().size_full().bg(theme::black(0.35)).window_control_area(gpui::WindowControlArea::Drag))
                .child(
                    div().absolute().top(px(8.)).right(px(8.)).flex().gap(px(4.)).occlude()
                        .child(round_btn("pip-big", icon("expand", 20.), 36.).on_click(|_, _, cx| crate::pip::toggle(cx)))
                        .child(round_btn("pip-close", icon("close", 18.), 36.).on_click(move |_, _, cx| {
                            crate::pip::toggle(cx);
                            pc.update(cx, |p, cx| p.close(cx));
                            crate::Nav::back(cx);
                        })),
                )
                .when(!crate::together::get(cx).read(cx).locked(), |d| d.child(
                    div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().gap(px(18.))
                        .child(round_btn("pip-b10", icon("replay10", 26.), 46.).on_click(move |_, _, cx| seek_by(&pa, -10., cx)))
                        .child(round_btn("pip-play", if paused { icon_fill("play", 34.) } else { icon_fill("pause", 34.) }, 60.).on_click({
                            let pb = self.pb.clone();
                            move |_, _, cx| {
                                let paused = pb.read(cx).state.paused;
                                if may(if paused { "play" } else { "pause" }, None, cx) {
                                    pb.read(cx).cmd("pause", json!({ "value": !paused }));
                                }
                            }
                        }))
                        .child(round_btn("pip-f10", icon("forward10", 26.), 46.).on_click(move |_, _, cx| seek_by(&pb, 10., cx))),
                ))
                                .child({
                    let [c1, c2] = self.tint.as_ref().map(|t| t.1).unwrap_or([theme::neon().into(), theme::neon().into()]);
                    let style = bar_style();
                    if !paused && bar_moves(style) {
                        window.request_animation_frame();
                    }
                    div().absolute().left(px(10.)).right(px(10.)).bottom(px(10.)).h(px(4.)).rounded_full().bg(theme::white(0.25)).child(
                        div().relative().h_full().rounded_full().overflow_hidden().w((vw - px(20.)) * frac).child(bar_fill(style, c1, c2, self.born.elapsed().as_secs_f32())),
                    )
                });
        }

        let paused = st.paused;
        let pb = self.pb.clone();
        let pb3 = self.pb.clone();
        let pb4 = self.pb.clone();
        let hover = self.hover_x;
        let still_img = match hover {
            Some(x) if st.duration > 0. => {
                let t = (x / f32::from(bar_w).max(1.)).clamp(0., 1.) as f64 * st.duration;
                self.want_still(t, cx)
            }
            _ => None,
        };
        let p = self.pb.read(cx);
        let segment = p.segment_at(st.pos);
        let next = p.next_video().map(|n| format!("S{} E{}", n["season"].as_i64().unwrap_or(0), n["episode"].as_i64().or(n["number"].as_i64()).unwrap_or(0)));
        let is_series = p.meta.as_ref().map_or(false, |m| m.kind == "series");
        // on someone's couch without the remote: no play / skip / episode / source controls
        let guest = crate::together::get(cx).read(cx).locked();
        let hosting = crate::together::get(cx).read(cx).hosting.is_some();
        let near_end = st.duration > 300. && st.duration - st.pos < 45. && next.is_some();
        // another stream to switch to (not counting AIOStreams' 🦆 ones)
        let has_streams = p
            .all
            .iter()
            .filter(|s| {
                let text = [&s["name"], &s["title"], &s["description"], &s["_addonName"]].iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" ");
                !(text.contains('🦆') || text.to_lowercase().contains("aiostreams"))
            })
            .count()
            > 1;
        if self.menu != self.menu_seen {
            self.menu_seen = self.menu;
            self.menu_at = Instant::now();
        }
        let mt = (self.menu_at.elapsed().as_secs_f32() / 0.32).min(1.);
        if mt < 1. {
            window.request_animation_frame();
        }
        let ma = 1. - (1. - mt).powi(4);
        let menu = self.menu.map(|m| self.menu_panel(m, window, cx)).map(|el| div().absolute().left_0().top(px((1. - ma) * 16.)).w_full().h_full().opacity(ma).child(el));
        // skip intro / recap; at the credits (or the last bit): next episode
        // the end of an episode: credits / preview (or the last bit when nobody
        // marked them). The next one counts down and plays by itself.
        let tail = matches!(segment.as_ref().map(|x| x.0.as_str()), Some("credits" | "preview")) || near_end;
        let next_id = if tail { self.pb.read(cx).next_video().and_then(|n| n["id"].as_str().map(String::from)) } else { None };
        let auto_next = couch::state::get().core.settings().playback.autoplay_next;
        match (&next_id, guest || !auto_next || st.paused) {
            (Some(id), false) if self.countdown_off.as_deref() != Some(id.as_str()) => {
                if self.countdown.as_ref().map(|c| &c.0) != Some(id) {
                    self.countdown = Some((id.clone(), Instant::now()));
                }
            }
            _ => self.countdown = None,
        }
        let left = self.countdown.as_ref().map(|c| COUNTDOWN - c.1.elapsed().as_secs_f32());
        if let Some(l) = left {
            if l <= 0. {
                self.countdown = None;
                self.countdown_off = next_id.clone();
                let pb = self.pb.clone();
                cx.defer(move |cx| pb.update(cx, |p, cx| p.play_next(cx)));
            } else {
                window.request_animation_frame();
            }
        }
        let tail_kind = segment.as_ref().map(|x| x.0.clone()).filter(|k| k == "credits" || k == "preview").unwrap_or_else(|| "credits".into());
        let skip: Option<gpui::AnyElement> = if guest { None } else { match &segment {
            Some((k, end)) if k == "intro" || k == "recap" || (k == "preview" && next.is_none()) => {
                let (end, pb) = (*end, self.pb.clone());
                let label = match k.as_str() { "intro" => "skip intro", "recap" => "skip recap", _ => "skip preview" };
                let id: &'static str = match k.as_str() { "intro" => "pl-skip-intro", "recap" => "pl-skip-recap", _ => "pl-skip-preview" };
                Some(seg_pill(id, k, label.into()).on_click(move |_, _, cx| {
                    if may("seek", Some(end), cx) {
                        pb.read(cx).cmd("seek", json!({ "to": end }));
                    }
                }).into_any_element())
            }
            _ if tail && next.is_some() => {
                let cancel: Option<Box<dyn Fn(&mut gpui::App)>> = left.map(|_| {
                    let me = cx.entity().downgrade();
                    let id = next_id.clone();
                    Box::new(move |cx: &mut gpui::App| {
                        let _ = me.update(cx, |v, cx| {
                            v.pb.update(cx, |p, _| p.no_autonext = true);
                            v.countdown = None;
                            v.countdown_off = id.clone();
                            cx.notify();
                        });
                    }) as Box<dyn Fn(&mut gpui::App)>
                });
                Some(next_pill(self.pb.clone(), next.clone().unwrap_or_default(), &tail_kind, left, cancel))
            }
            _ => None,
        } };
        let root = root.children(skip.map(|b| div().absolute().right(px(theme::GUTTER)).bottom(px(140.)).child(b)));
        // what's next, while you point at "next"
        let next_a = self.next_since.map_or(0., |t| {
            let x = (t.elapsed().as_secs_f32() / 0.22).min(1.);
            if x < 1. {
                window.request_animation_frame();
            }
            1. - (1. - x).powi(3)
        });
        let next_card = self.next_hover.then(|| self.pb.read(cx).next_video()).flatten().map(|n| {
            let imdb = crate::api::imdb_episode(n["id"].as_str().unwrap_or(""));
            let still = n["thumbnail"].as_str().map(String::from).or(imdb.clone());
            let over = n["overview"].as_str().or(n["description"].as_str()).unwrap_or("").to_string();
            // above the "next" button (kept inside the window)
            let w = f32::from(vw);
            let x = (self.next_x.get() - 170.).clamp(theme::GUTTER, (w - 340. - theme::GUTTER).max(theme::GUTTER));
            div()
                .absolute()
                .left(px(x))
                // above the scrub bar, rising a little as it fades in
                .bottom(px(100. + 10. * next_a))
                .opacity(next_a)
                .w(px(340.))
                .rounded(px(18.))
                .overflow_hidden()
                .bg(gpui::rgba(0x151114f2))
                .border_1()
                .border_color(theme::white(0.10))
                .flex()
                .flex_col()
                .children(still.map(|u| {
                    let alt = imdb.clone().filter(|a| a != &u);
                    img(u).w_full().h(px(191.)).object_fit(ObjectFit::Cover).with_fallback(move || match alt.clone() {
                        Some(a) => img(a).w_full().h(px(191.)).object_fit(ObjectFit::Cover).into_any_element(),
                        None => div().into_any_element(),
                    })
                }))
                .child(
                    div()
                        .p(px(14.))
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child(format!("NEXT · S{} E{}", n["season"].as_i64().unwrap_or(0), n["episode"].as_i64().or(n["number"].as_i64()).unwrap_or(0))))
                        .child(div().text_size(px(17.)).font_weight(FontWeight::BLACK).child(playback::episode_name(&n)))
                        .children((!over.is_empty()).then(|| div().text_size(px(13.)).line_height(px(18.)).text_color(theme::label2()).line_clamp(3).child(over))),
                )
        });
        let tw: f32 = self.scrub_at.get().1.max(100.);
        // a narrow window: fewer buttons, so the progress bar keeps its room
        let narrow1 = f32::from(vw) < 1180.;
        let narrow2 = f32::from(vw) < 940.;
        self.track_w = tw;
        let dragging = self.drag;
        let shown_frac = match (dragging, hover) {
            (true, Some(x)) => (x / tw.max(1.)).clamp(0., 1.),
            _ => frac,
        };
        let fat = dragging || hover.is_some();
        let segs: Vec<(f32, f32, gpui::Rgba)> = if st.duration > 0. {
            self.pb
                .read(cx)
                .segments
                .as_object()
                .into_iter()
                .flat_map(|o| o.iter())
                .flat_map(|(k, l)| l.as_array().into_iter().flatten().map(move |sg| (k.clone(), sg.clone())))
                .map(|(k, sg)| {
                    let a = sg["start"].as_f64().unwrap_or(0.);
                    let b = sg["end"].as_f64().unwrap_or(st.duration);
                    ((a / st.duration) as f32, ((b - a).max(0.) / st.duration) as f32, seg_color(&k))
                })
                .collect()
        } else {
            vec![]
        };
        let menu_on = self.menu;
        // what you asked for, while mpv catches up (no jumping back and forth)
        let vol_now = self.vol_target.filter(|_| self.vol_open || self.vol_pinned || self.vol_hud.map_or(false, |t| t.elapsed() < Duration::from_millis(900))).unwrap_or(st.volume);
        let vol_max = if couch::state::get().core.settings().playback.volume_boost { 130. } else { 100. };
        let vol_frac = (vol_now / vol_max).clamp(0., 1.) as f32;
        let vol_x = self.vol_x.clone();
        // the sound playing has more than two channels (5.1, 7.1…): headphone surround is offered
        let surround = self.pb.read(cx).tracks.iter().find(|t| t["type"] == "audio" && t["selected"] == true).map_or(false, |t| t["channels"].as_i64().map_or(false, |c| c > 2) || t["title"] == "Atmos · couch");
        let _ = &title;
        let top_logo = logo.clone();
        let top_title = title.clone();
        root
            // top: back, the title's logo
            .child(
                div()
                    .absolute()
                    .top(px(-(1. - ca) * 32.))
                    .opacity(ca)
                    .left_0()
                    .w_full()
                    .h(px(150.))
                    .bg(linear_gradient(180., linear_color_stop(theme::black(0.65), 0.), linear_color_stop(theme::black(0.0), 1.)))
                    .child(
                        // centred on the same line as the 4K / HDR badges (top 50, 32 tall)
                        div()
                            .absolute()
                            .left(px(g))
                            .top(px(42.))
                            .flex()
                            .items_center()
                            .gap(px(18.))
                            .child(
                                glass(div())
                                    .id("pl-back")
                                    .occlude()
                                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                    .size(px(48.))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme::white(0.18)))
                                    .on_click(move |_, _, cx| {
                                        // back keeps the film playing in the corner
                                        let _ = &pb;
                                        crate::Nav::back(cx);
                                    })
                                    .child(icon("back", 24.)),
                            )
                            .child(div().opacity(1. - pv_top).relative().top(px(-pv_top * 14.))
                                // a soft dark pool behind the logo, so it reads over a bright picture
                                .child(div().absolute().top(px(6.)).bottom(px(6.)).left(px(10.)).right(px(10.)).rounded_full().shadow(vec![gpui::BoxShadow { color: theme::black(0.28), offset: gpui::point(px(0.), px(2.)), blur_radius: px(30.), spread_radius: px(6.), inset: false }]))
                                .child(match top_logo {
                                Some(u) => {
                                    let t = top_title.clone();
                                    img(u)
                                        .h(px(46.))
                                        .max_w(px(320.))
                                        .object_fit(ObjectFit::Contain)
                                        .with_fallback(move || div().text_size(px(22.)).font_weight(FontWeight::BLACK).child(t.clone()).into_any_element())
                                        .into_any_element()
                                }
                                None => div().text_size(px(22.)).font_weight(FontWeight::BLACK).child(top_title).into_any_element(),
                            })),
                    ),
            )
            // what's playing, as badges (4K, Dolby Vision, Atmos...)
            .child({
                let p = self.pb.read(cx);
                let height = p.tracks.iter().find(|t| t["type"] == "video" && t["selected"] == true).and_then(|t| t["height"].as_i64()).unwrap_or(0);
                let codec = p.tracks.iter().find(|t| t["type"] == "audio" && t["selected"] == true).and_then(|t| t["codec"].as_str()).unwrap_or("").to_string();
                let c = p.stream.as_ref().map(|s| s["_couch"].clone()).unwrap_or_default();
                // who's watching with you, under the badges
                let people: Vec<(Option<String>, String)> = self.reactions.read(cx).people.iter().map(|x| (x["avatar"].as_str().map(String::from), x["name"].as_str().unwrap_or("someone").to_string())).collect();
                div()
                    .absolute()
                    .top(px(50. - (1. - ca) * 32.))
                    .opacity(ca)
                    .right(px(g))
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(12.))
                    .child({
                        let bs = crate::ui::badges::format(&c, height, &codec);
                        div()
                            .relative()
                            .flex()
                            .gap(px(10.))
                            // a soft dark pool under them, so they read on a bright picture
                            .when(!bs.is_empty(), |d| d.child(div().absolute().top(px(4.)).bottom(px(4.)).left(px(6.)).right(px(6.)).rounded_full().shadow(vec![gpui::BoxShadow { color: theme::black(0.25), offset: gpui::point(px(0.), px(2.)), blur_radius: px(22.), spread_radius: px(4.), inset: false }])))
                            .children(bs.into_iter().map(|b| img(b.src).h(px(32.)).flex_none().object_fit(ObjectFit::Contain)))
                    })
                    .when(people.len() > 1, |d| d.child(crate::ui::faces_below("party", people, 30.)))
            })
            // bottom: the scrub bar, then one glass bar with everything on it
            .child(
                div()
                    .absolute()
                    .bottom(px(-(1. - ca) * 48.))
                    .opacity(ca)
                    .left_0()
                    .w_full()
                    .h(px(210.))
                    .bg(linear_gradient(180., linear_color_stop(theme::black(0.0), 0.), linear_color_stop(theme::black(0.72), 1.)))
                    .child(
                        glass(div())
                            .absolute()
                            .left(px(g - 6.))
                            .right(px(g - 6.))
                            .bottom(px(24.))
                            .h(px(62.))
                            .px(px(9.))
                            .occlude()
                            .rounded_full()
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .when(!guest, |d| d.child(bar_btn("pl-play", if paused { icon_fill("play", 28.) } else { icon_fill("pause", 28.) }, false).on_click(
                                move |_, _, cx| {
                                    let paused = pb3.read(cx).state.paused;
                                    if may(if paused { "play" } else { "pause" }, None, cx) {
                                        pb3.read(cx).cmd("pause", json!({ "value": !paused }));
                                    }
                                },
                            ))
                            .child(bar_btn("pl-back10", icon("replay10", 25.), false).on_click({
                                let pb = self.pb.clone();
                                move |_, _, cx| seek_by(&pb, -10., cx)
                            }))
                            .child(bar_btn("pl-fwd10", icon("forward10", 25.), false).on_click({
                                let pb = self.pb.clone();
                                move |_, _, cx| seek_by(&pb, 10., cx)
                            })))
                            // a guest: whose remote it is, instead of the buttons
                            .when(guest, |d| d.child(couch_chip(cx)))
                            // the time, the bar, the time left
                            .child(div().ml(px(10.)).w(px(64.)).flex_none().flex().justify_end().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.5)).child(clock(st.pos)))
                            .child(
                                div()
                                    .id("pl-scrub")
                                    .flex_1()
                                    .min_w_0()
                                    .mx(px(12.))
                                    .h(px(40.))
                                    .flex()
                                    .items_center()
                                    .cursor_pointer()
                                    .child(gpui::canvas({ let at = self.scrub_at.clone(); move |b, _, _| at.set((f32::from(b.origin.x), f32::from(b.size.width))) }, |_, _, _, _| {}).absolute().size_full())
                                    // while dragging: the release ends it wherever it happens (over a
                                    // button, outside the window), not only over the picture
                                    .when(self.drag, |d| {
                                        let me = cx.entity().downgrade();
                                        let me2 = me.clone();
                                        let at = self.scrub_at.clone();
                                        d.child(
                                            gpui::canvas(
                                                |_, _, _| (),
                                                move |_, _, window, _| {
                                                    let me = me.clone();
                                                    window.on_mouse_event(move |_: &gpui::MouseUpEvent, phase, _, cx| {
                                                        if phase == gpui::DispatchPhase::Capture {
                                                            let _ = me.update(cx, |v, cx| v.finish_drag(cx));
                                                        }
                                                    });
                                                    let me2 = me2.clone();
                                                    let at = at.clone();
                                                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                                                        if phase == gpui::DispatchPhase::Capture {
                                                            let x = f32::from(e.position.x) - at.get().0;
                                                            let _ = me2.update(cx, |v, cx| {
                                                                if e.pressed_button != Some(gpui::MouseButton::Left) {
                                                                    v.finish_drag(cx);
                                                                } else {
                                                                    v.hover_x = Some(x);
                                                                    cx.notify();
                                                                }
                                                            });
                                                        }
                                                    });
                                                },
                                            )
                                            .absolute()
                                            .size_0(),
                                        )
                                    })
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |v, e: &gpui::MouseDownEvent, _, cx| {
                                        cx.stop_propagation();
                                        if crate::together::get(cx).read(cx).locked() {
                                            return;
                                        }
                                        v.drag = true;
                                        v.hover_x = Some(f32::from(e.position.x) - v.scrub_at.get().0);
                                        cx.notify();
                                    }))
                                    .on_mouse_move(cx.listener(move |v, e: &MouseMoveEvent, _, cx| {
                                        v.hover_x = Some(f32::from(e.position.x) - v.scrub_at.get().0);
                                        cx.notify();
                                    }))
                                    .on_hover(cx.listener(|v, on: &bool, _, cx| {
                                        if !*on && !v.drag {
                                            v.hover_x = None;
                                            cx.notify();
                                        }
                                    }))
                            .child(
                                div()
                                    .relative()
                                    .w_full()
                                    .h(px(if fat { 8. } else { 5. }))
                                    .rounded_full()
                                    .bg(theme::white(0.2))
                                    .child(div().absolute().top_0().left_0().h_full().w(px(tw * ahead)).rounded_full().bg(theme::white(0.3)))
                                    .child({
                                        // the fill, in the style picked in settings
                                        let [c1, c2] = self.tint.as_ref().map(|t| t.1).unwrap_or([theme::neon().into(), theme::neon().into()]);
                                        let style = bar_style();
                                        let t = self.born.elapsed().as_secs_f32();
                                        if ca > 0. && !paused && bar_moves(style) {
                                            window.request_animation_frame();
                                        }
                                        div()
                                            .absolute()
                                            .top_0()
                                            .left_0()
                                            .h_full()
                                            .w(px(tw * shown_frac))
                                            .rounded_full()
                                            .overflow_hidden()
                                            .child(bar_fill(style, c1, c2, t))
                                            .when(style != "white", |d| d.shadow(vec![gpui::BoxShadow { color: c1.opacity(0.55), offset: gpui::point(px(0.), px(0.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }]))
                                    })
                                    // intro / recap / credits / preview, each in its colour (over the fill,
                                    // so you still see them in the part you've watched)
                                    .children(segs.into_iter().map(|(a, w, c)| div().absolute().top_0().h_full().left(px(tw * a)).w(px((tw * w).max(2.))).rounded_full().bg(gpui::Rgba { a: 0.85, ..c })))
                                    .child(
                                        div()
                                            .absolute()
                                            .top(px(if fat { -5. } else { -5.5 }))
                                            .left(px(tw * shown_frac - if fat { 9. } else { 8. }))
                                            .size(px(if fat { 18. } else { 16. }))
                                            .rounded_full()
                                            .bg(theme::text())
                                            .shadow(vec![gpui::BoxShadow { color: theme::black(0.5), offset: gpui::point(px(0.), px(2.)), blur_radius: px(8.), spread_radius: px(0.), inset: false }]),
                                    )
                                    .children(hover.filter(|_| st.duration > 0.).map(|x| {
                                        let t = (x / tw.max(1.)).clamp(0., 1.) as f64 * st.duration;
                                        let still = still_img.clone();
                                        div()
                                            .absolute()
                                            .bottom(px(20.))
                                            .left(px((x - 104.).clamp(0., (tw - 208.).max(0.))))
                                            .w(px(208.))
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .gap(px(6.))
                                            .children(still.map(|i| {
                                                div().p(px(3.)).rounded(px(14.)).bg(gpui::rgba(0x151114e6)).border_1().border_color(theme::white(0.12)).child(img(i).w(px(200.)).h(px(112.)).rounded(px(11.)).object_fit(ObjectFit::Cover))
                                            }))
                                            .child(glass(div()).px(px(12.)).py(px(4.)).rounded_full().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(clock(t)))
                                    })),
                            )
                            )
                            .child(div().w(px(64.)).flex_none().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.5)).child(if st.duration > 0. { format!("-{}", clock(st.duration - st.pos)) } else { String::new() }))
                            .children(next.clone().filter(|_| !narrow2 && !guest).map(|_| {
                                let pb = self.pb.clone();
                                let nx = self.next_x.clone();
                                bar_btn("pl-next", icon("next", 22.), false)
                                    .relative()
                                    .child(gpui::canvas(move |b, _, _| nx.set(f32::from(b.origin.x + b.size.width / 2.)), |_, _, _, _| {}).absolute().size_full())
                                    .on_click(cx.listener(move |v, _, _, cx| {
                                        // the preview goes with the click (it stayed up into the next episode)
                                        v.next_hover = false;
                                        v.next_since = None;
                                        if may("episode", None, cx) {
                                            pb.update(cx, |p, cx| p.play_next(cx))
                                        }
                                        cx.notify();
                                    }))
                                    .on_hover(cx.listener(|v, on: &bool, _, cx| {
                                        v.next_hover = *on;
                                        v.next_since = on.then(Instant::now);
                                        cx.notify();
                                    }))
                            }))
                            // volume: a slider that pops out above the button
                            .child(
                                div()
                                    .id("pl-vol")
                                    .relative()
                                    .occlude()
                                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                    .on_hover(cx.listener(|v, on: &bool, _, cx| {
                                        v.vol_open = *on;
                                        if *on {
                                            v.vol_since = Some(Instant::now());
                                        } else {
                                            v.vol_target = None;
                                        }
                                        cx.notify();
                                    }))
                                    .child(
                                        bar_btn("pl-mute", icon(if st.muted || st.volume <= 0. { "mute" } else { "volume" }, 23.), self.vol_pinned)
                                            .on_click(cx.listener(|v, _, _, cx| {
                                                cx.stop_propagation();
                                                v.vol_pinned = !v.vol_pinned;
                                                if v.vol_pinned {
                                                    v.menu = None;
                                                    v.reactions.update(cx, |r, cx| {
                                                        r.open = false;
                                                        cx.notify();
                                                    });
                                                }
                                                v.vol_since = Some(Instant::now());
                                                cx.notify();
                                            })),
                                    )
                                    .when(self.vol_open || self.vol_pinned, |d| {
                                        let pb = pb4.clone();
                                        let muted = st.muted;
                                        let va = self.vol_since.map_or(1., |t| {
                                            let x = (t.elapsed().as_secs_f32() / 0.24).min(1.);
                                            if x < 1. {
                                                window.request_animation_frame();
                                            }
                                            1. - (1. - x).powi(4)
                                        });
                                        const SH: f32 = 130.;
                                        d.child(
                                            // (a bridge under it, so moving up from the button keeps it open)
                                            div()
                                                .absolute()
                                                .left(px(-8.))
                                                .bottom(px(44.))
                                                .w(px(60.))
                                                .pb(px(12. - (1. - va) * 8.))
                                                .opacity(va)
                                                .child(
                                                    glass(div())
                                                        .id("pl-vol-pop")
                                                        .occlude()
                                                        .rounded(px(18.))
                                                        .py(px(12.))
                                                        .flex()
                                                        .flex_col()
                                                        .items_center()
                                                        .gap(px(10.))
                                                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(format!("{}", vol_now.round() as i64)))
                                                        .child(
                                                            div()
                                                                .id("pl-vol-slide")
                                                                .w(px(44.))
                                                                .h(px(SH))
                                                                .flex()
                                                                .justify_center()
                                                                .cursor_pointer()
                                                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|v, e: &gpui::MouseDownEvent, _, cx| {
                                                                    cx.stop_propagation();
                                                                    v.set_volume_at(f32::from(e.position.y), cx);
                                                                }))
                                                                .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _, cx| {
                                                                    if e.pressed_button == Some(gpui::MouseButton::Left) {
                                                                        v.set_volume_at(f32::from(e.position.y), cx);
                                                                    }
                                                                }))
                                                                .child(
                                                                    div()
                                                                        .relative()
                                                                        .w(px(6.))
                                                                        .h_full()
                                                                        .rounded_full()
                                                                        .bg(theme::white(0.18))
                                                                        .child(gpui::canvas(move |b, _, _| vol_x.set(f32::from(b.origin.y + b.size.height)), |_, _, _, _| {}).absolute().size_full())
                                                                        .child(div().absolute().bottom_0().left_0().w_full().rounded_full().bg(theme::text()).h(px(SH * vol_frac)))
                                                                        .child(div().absolute().left(px(-5.)).bottom(px(SH * vol_frac - 8.)).size(px(16.)).rounded_full().bg(theme::text()).shadow(vec![gpui::BoxShadow { color: theme::black(0.4), offset: gpui::point(px(0.), px(2.)), blur_radius: px(6.), spread_radius: px(0.), inset: false }])),
                                                                ),
                                                        )
                                                        // night mode: evens the loud and the quiet out
                                                        .child({
                                                            let on = night_on();
                                                            div()
                                                                .id("pl-night")
                                                                .size(px(36.))
                                                                .rounded_full()
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .cursor_pointer()
                                                                .when(on, |d| d.bg(theme::text()))
                                                                .when(!on, |d| d.bg(theme::white(0.08)).hover(|s| s.bg(theme::white(0.16))))
                                                                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                                                .on_click(cx.listener(|v, _, _, cx| {
                                                                    cx.stop_propagation();
                                                                    let on = !night_on();
                                                                    let _ = couch::state::get().core.db.set("gpui_night", &on);
                                                                    v.pb.read(cx).cmd("night", json!({ "value": on }));
                                                                    v.vol_hud = Some(Instant::now());
                                                                    let msg = if on { "night mode on: explosions quieter, voices clearer" } else { "night mode off" };
                                                                    cx.defer(move |cx| crate::Nav::toast(msg.into(), cx));
                                                                    cx.notify();
                                                                }))
                                                                .child(if on { icon_fill("moon", 18.).text_color(gpui::rgb(0x000000)) } else { icon("moon", 18.) })
                                                        })
                                                        // headphone surround: only for sound with more than two channels
                                                        .when(surround, |d| {
                                                            let on = headphones_on();
                                                            d.child(
                                                                div()
                                                                    .id("pl-hp")
                                                                    .size(px(36.))
                                                                    .rounded_full()
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .cursor_pointer()
                                                                    .when(on, |d| d.bg(theme::text()))
                                                                    .when(!on, |d| d.bg(theme::white(0.08)).hover(|s| s.bg(theme::white(0.16))))
                                                                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                                                    .on_click(cx.listener(move |v, _, _, cx| {
                                                                        cx.stop_propagation();
                                                                        let on = !headphones_on();
                                                                        let _ = couch::state::get().core.db.set("gpui_headphones", &on);
                                                                        v.vol_hud = Some(Instant::now());
                                                                        cx.spawn(async move |_, cx| {
                                                                            let r = crate::api::call::<serde_json::Value>("player_cmd", json!({ "cmd": "headphones", "args": { "value": on } })).await;
                                                                            let msg = match (on, r) {
                                                                                (false, _) => "headphone surround off".to_string(),
                                                                                (true, Ok(v)) if v["atmos"] == true => if v["atmosTrack"] == true || v["objects"] == true {
                                                                                    "Atmos on headphones: every sound around you, and above".to_string()
                                                                                } else {
                                                                                    "headphone surround on (TrueHD: Atmos heights too, if it has them)".to_string()
                                                                                },
                                                                                (true, Ok(v)) if v["on"] == true => format!("headphone surround on: {} speakers around your head", v["speakers"].as_i64().unwrap_or(0)),
                                                                                (true, Ok(_)) => "this one's stereo, nothing to place around you".to_string(),
                                                                                (true, Err(e)) => format!("headphone surround didn't start: {e}"),
                                                                            };
                                                                            let _ = cx.update(|cx| crate::Nav::toast(msg, cx));
                                                                        })
                                                                        .detach();
                                                                        cx.notify();
                                                                    }))
                                                                    .child(div().text_size(px(17.)).text_color(if on { gpui::rgb(0x000000) } else { gpui::rgb(0xffffff) }).child("🎧")),
                                                            )
                                                        })
,
                                                ),
                                        )
                                    }),
                            )
                            .when(super::reactions::room(cx).is_some() && self.reactions.read(cx).people.len() > 1 && !narrow2, |d| {
                                let r = self.reactions.clone();
                                d.child(bar_btn("pl-react", crate::ui::emoji::emoji("😂", 24.), false).on_click(cx.listener(move |v, _, _, cx| {
                                    cx.stop_propagation();
                                    v.menu = None;
                                    v.vol_pinned = false;
                                    r.update(cx, |r, cx| r.toggle(cx));
                                })))
                            })
                            .when(hosting, |d| {
                                d.child(bar_btn("pl-couch", icon("couch", 23.), menu_on == Some(Menu::Couch)).on_click(cx.listener(|v, _, _, cx| {
                                    cx.stop_propagation();
                                    v.toggle_menu(Menu::Couch, cx);
                                })))
                            })
                            .child(bar_btn("pl-audio", icon("subtitles", 23.), matches!(menu_on, Some(Menu::Audio | Menu::Subs))).on_click(cx.listener(|v, _, _, cx| {
                                cx.stop_propagation();
                                v.toggle_menu(Menu::Audio, cx);
                            })))
                            .when(is_series && !narrow2 && !guest, |d| {
                                d.child(bar_btn("pl-eps", icon("stack", 23.), menu_on == Some(Menu::Episodes)).on_click(cx.listener(|v, _, _, cx| {
                                    cx.stop_propagation();
                                    v.toggle_menu(Menu::Episodes, cx);
                                })))
                            })
                            .when(has_streams && !narrow1, |d| {
                                d.child(bar_btn("pl-src", icon("link", 21.), menu_on == Some(Menu::Sources)).on_click(cx.listener(|v, _, _, cx| {
                                    cx.stop_propagation();
                                    v.toggle_menu(Menu::Sources, cx);
                                })))
                            })
                            .child(
                                bar_btn(
                                    "pl-speed",
                                    if (snap(st.speed) - 1.).abs() < 0.01 { icon("sliders", 22.).into_any_element() } else { div().text_size(px(14.)).font_weight(FontWeight::BLACK).child(speed_label(st.speed)).into_any_element() },
                                    menu_on == Some(Menu::Speed),
                                )
                                .on_click(cx.listener(|v, _, _, cx| {
                                    cx.stop_propagation();
                                    v.toggle_menu(Menu::Speed, cx);
                                })),
                            )
                            .when(!narrow1, |d| d.child(bar_btn("pl-pip", icon("pip", 22.), false).on_click(|_, window, cx| crate::pip::toggle_in(window, cx))))
                            .child(bar_btn("pl-full", icon(if window.is_fullscreen() { "exitFullscreen" } else { "fullscreen" }, 22.), false).on_click(|_, window, _| window.toggle_fullscreen())),
                    ),
            )
            // over everything else: the open menu, and what's next
            .children(menu)
            .children(next_card)
            .children(together_overlay(cx))
    }
}

/// A scrolling list with soft shadows at the edges that have more to see:
/// at the bottom while there's more below, at the top once there's more above.
fn faded(list: impl IntoElement, h: &gpui::ScrollHandle) -> gpui::Div {
    let off = -f32::from(h.offset().y);
    let max = f32::from(h.max_offset().y);
    let bg = gpui::rgba(0x151114f2);
    let clear = gpui::Rgba { a: 0., ..bg };
    div()
        .relative()
        .min_h_0()
        .child(list)
        .when(off > 1., |d| d.child(div().absolute().top_0().left_0().w_full().h(px(34.)).rounded_t(px(12.)).bg(linear_gradient(180., linear_color_stop(bg, 0.), linear_color_stop(clear, 1.)))))
        .when(max - off > 1., |d| d.child(div().absolute().bottom_0().left_0().w_full().h(px(34.)).rounded_b(px(12.)).bg(linear_gradient(180., linear_color_stop(clear, 0.), linear_color_stop(bg, 1.)))))
}

/// a guest's spot where the play buttons would be: whose remote it is
fn couch_chip(cx: &mut Context<PlayerView>) -> gpui::AnyElement {
    let t = crate::together::get(cx);
    let host = t.read(cx).following.as_ref().map(|f| f.1.clone()).unwrap_or_default();
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(40.))
        .px(px(14.))
        .rounded_full()
        .bg(theme::white(0.08))
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::white(0.8))
        .child(icon("couch", 18.))
        .child(format!("{host} has the remote"))
        .into_any_element()
}

/// watch together, over the film: the host left (choose), or the host is
/// waiting on someone's connection
fn together_overlay(cx: &mut Context<PlayerView>) -> Option<gpui::AnyElement> {
    let t = crate::together::get(cx);
    let tr = t.read(cx);
    if let Some(name) = tr.orphan.clone() {
        let (t1, t2) = (t.clone(), t.clone());
        let btn = |id: &'static str, label: &'static str, main: bool| {
            div()
                .id(id)
                .h(px(48.))
                .px(px(22.))
                .rounded_full()
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .when(main, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)).hover(|s| s.opacity(0.88)))
                .when(!main, |d| d.bg(theme::white(0.12)).hover(|s| s.bg(theme::white(0.18))))
                .child(label)
        };
        return Some(
            div()
                .id("tg-orphan")
                .occlude()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(theme::black(0.55))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(icon("couch", 44.))
                .child(div().text_size(px(30.)).font_weight(FontWeight::BLACK).child(format!("{name} left the couch")))
                .child(div().text_size(px(16.)).text_color(theme::label2()).child("paused right where they stopped"))
                .child(
                    div()
                        .mt(px(14.))
                        .flex()
                        .gap(px(12.))
                        .child(btn("tg-alone", "keep watching on your own", true).on_click(move |_, _, cx| t1.update(cx, |t, cx| t.keep_alone(cx))))
                        .child(btn("tg-stop", "stop", false).on_click(move |_, _, cx| t2.update(cx, |t, cx| t.stop_too(cx)))),
                )
                .into_any_element(),
        );
    }
    if let Some((who, _)) = tr.held.clone() {
        return Some(
            div()
                .absolute()
                .top(px(28.))
                .left_0()
                .w_full()
                .flex()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .h(px(44.))
                        .px(px(18.))
                        .rounded_full()
                        .bg(gpui::rgba(0x151114e6))
                        .border_1()
                        .border_color(theme::white(0.1))
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(div().size(px(10.)).rounded_full().bg(gpui::rgb(0xffc83d)))
                        .child(format!("waiting for {who} to catch up… press play to carry on")),
                )
                .into_any_element(),
        );
    }
    None
}

/// the speed you picked: watch-together nudges mpv a few % either side to keep
/// you in step with the host, which shouldn't show up as "1.0821318393x"
fn snap(v: f64) -> f64 {
    [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0].into_iter().find(|p| (p - v).abs() < 0.1).unwrap_or((v * 100.).round() / 100.)
}

fn speed_label(v: f64) -> String {
    let s = snap(v);
    format!("{}x", if s.fract() == 0. { format!("{s:.0}") } else { format!("{s}") })
}

/// a tiny picture of what each picture mode does
fn aspect_icon(k: &str, on: bool) -> impl IntoElement {
    let c: gpui::Hsla = if on { theme::text().into() } else { theme::white(0.6) };
    let frame = div().relative().w(px(40.)).h(px(26.)).rounded(px(4.)).border_1().border_color(c).overflow_hidden();
    match k {
        // the whole picture, bars above and below
        "fit" => frame.child(div().absolute().left_0().top(px(6.)).w_full().h(px(12.)).bg(c)),
        // fills it, the sides cut
        "fill" => frame.child(div().absolute().left(px(-8.)).top_0().w(px(54.)).h_full().bg(c)),
        // the bars cut off, nothing else
        "crop" => frame
            .child(div().absolute().left_0().top(px(3.)).w_full().h(px(18.)).bg(c))
            .child(div().absolute().left(px(3.)).top(px(1.)).w(px(32.)).h(px(1.)).bg(theme::black(0.6)))
            .child(div().absolute().left(px(3.)).bottom(px(1.)).w(px(32.)).h(px(1.)).bg(theme::black(0.6))),
        // pulled to the edges
        _ => frame.p(px(3.)).child(div().size_full().rounded(px(2.)).bg(c)),
    }
}

/// A subtitle with its asides set apart: "[steven]" (who's speaking, or a
/// sound) quieter and in small letters, "(whispering)" quieter and slanted,
/// A subtitle line as the settings say: font, size (scaled to the window),
/// weight, colour, and a box / glass / outline / shadow / nothing behind it.
/// (Settings uses this too, for its preview.)
pub fn sub_block(text: &str, vw: f32, ss: &couch::settings::Subtitles) -> gpui::AnyElement {
    let size = (vw * 0.0158).clamp(13., 30.) * ss.size.clamp(0.5, 2.);
    let font: &'static str = match ss.font.as_str() {
        "Satoshi" => "Satoshi",
        "Segoe UI" => "Segoe UI",
        "Georgia" => "Georgia",
        "Consolas" => "Consolas",
        "Arial" => "Arial",
        _ => "Figtree",
    };
    let weight = FontWeight(ss.weight.clamp(300, 900) as f32);
    let color = parse_hex(&ss.color).unwrap_or(gpui::hsla(0., 0., 1., 1.)).opacity(ss.text_opacity.clamp(0.2, 1.));
    let lh = size * 1.33;
    let text_el = |c: gpui::Hsla, styled: bool| {
        div()
            .w_full()
            .text_center()
            .font_family(font)
            .text_size(px(size))
            .font_weight(weight)
            .line_height(px(lh))
            .text_color(c)
            .child(styled_sub_in(text, if styled { None } else { Some(c) }))
    };
    let op = ss.bg_opacity.clamp(0., 1.);
    let base = div().relative().max_w(px((vw * 0.8).min(1200.))).text_center();
    match ss.background.as_str() {
        // the letters drawn over a dark copy of themselves, a hair out each way
        "outline" | "shadow" => {
            let k = (size / 18.).clamp(1., 2.4);
            let mut d = base.px(px(10.));
            if ss.background == "outline" {
                let dark = gpui::hsla(0., 0., 0., 0.95f32.max(op));
                for (dx, dy) in [(-k, 0.), (k, 0.), (0., -k), (0., k), (-k * 0.7, -k * 0.7), (k * 0.7, -k * 0.7), (-k * 0.7, k * 0.7), (k * 0.7, k * 0.7)] {
                    d = d.child(div().absolute().left(px(10. + dx)).right(px(10. - dx)).top(px(dy)).child(text_el(dark, false)));
                }
            } else {
                // a soft, spread-out shadow: faint copies on rings around the
                // letters (a little lower than them), adding up to a blur
                let faint = gpui::hsla(0., 0., 0., 0.09 + op * 0.06);
                for (ring, r) in [1.0f32, 2.2, 3.6].into_iter().enumerate() {
                    let n = 8 + ring * 4;
                    for a in 0..n {
                        let ang = a as f32 / n as f32 * std::f32::consts::TAU;
                        let (dx, dy) = (ang.cos() * r * k, ang.sin() * r * k + k * 0.9);
                        d = d.child(div().absolute().left(px(10. + dx)).right(px(10. - dx)).top(px(dy)).child(text_el(faint, false)));
                    }
                }
            }
            d.child(div().relative().child(text_el(color, true))).into_any_element()
        }
        "none" => base.px(px(10.)).child(text_el(color, true)).into_any_element(),
        // frosted: softer corners, a faint rim
        "glass" => base
            .px(px(18.))
            .py(px(6.))
            .rounded(px(14.))
            .bg(gpui::hsla(0.9, 0.1, 0.07, op.max(0.15)))
            .border_1()
            .border_color(theme::white(0.08))
            .child(text_el(color, true))
            .into_any_element(),
        _ => base.px(px(16.)).py(px(6.)).rounded(px(8.)).bg(theme::black(op)).child(text_el(color, true)).into_any_element(),
    }
}

fn parse_hex(h: &str) -> Option<gpui::Hsla> {
    let h = h.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(gpui::rgb(v).into())
}

/// ♪ lyrics slanted.
fn styled_sub(text: &str) -> gpui::StyledText {
    styled_sub_in(text, None)
}

/// (`ink`: one colour for everything, for the dark copies behind an outline)
fn styled_sub_in(text: &str, ink: Option<gpui::Hsla>) -> gpui::StyledText {
    let mut marks: Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> = vec![];
    let aside = gpui::HighlightStyle { color: Some(ink.unwrap_or(gpui::hsla(0., 0., 0.78, 1.))), font_weight: Some(FontWeight::MEDIUM), ..Default::default() };
    let slant = gpui::HighlightStyle { color: Some(ink.unwrap_or(gpui::hsla(0., 0., 0.78, 1.))), font_style: Some(gpui::FontStyle::Italic), font_weight: Some(FontWeight::MEDIUM), ..Default::default() };
    for (open, close, style) in [('[', ']', aside), ('(', ')', slant)] {
        let mut from = None;
        for (i, c) in text.char_indices() {
            if c == open {
                from = Some(i);
            } else if c == close {
                if let Some(a) = from.take() {
                    marks.push((a..i + c.len_utf8(), style));
                }
            }
        }
    }
    // a sung line, ♪ to ♪ (or the rest of the line)
    for line in text.split('\n') {
        if line.contains('♪') || line.contains('♫') {
            let start = line.as_ptr() as usize - text.as_ptr() as usize;
            marks.push((start..start + line.len(), gpui::HighlightStyle { font_style: Some(gpui::FontStyle::Italic), ..Default::default() }));
        }
    }
    marks.sort_by_key(|m| m.0.start);
    // overlapping marks: keep the first
    let mut clean: Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> = vec![];
    for m in marks {
        if clean.last().map_or(true, |l| m.0.start >= l.0.end) {
            clean.push(m);
        }
    }
    gpui::StyledText::new(text.to_string()).with_highlights(clean)
}
