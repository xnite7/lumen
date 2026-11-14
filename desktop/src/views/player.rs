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
}

pub struct PlayerView {
    pb: Entity<Playback>,
    menu: Option<Menu>,
    /// seek-bar stills, every 10 s of the film
    stills: std::collections::HashMap<i64, Option<std::sync::Arc<gpui::Image>>>,
    vol_hud: Option<Instant>,
    last_move: Instant,
    shown: bool,
    hover_x: Option<f32>,
    reactions: Entity<super::reactions::Reactions>,
    /// pointing at "next": show what it is
    next_hover: bool,
    vol_acc: f64,
    vol_target: Option<f64>,
    /// dragging the scrub bar (hover_x is where)
    drag: bool,
    track_w: f32,
    /// the volume slider slides out of its button while you point at it
    vol_open: bool,
    vol_x: std::rc::Rc<std::cell::Cell<f32>>,
    aspect: &'static str,
    /// the season the episodes menu is showing (None: the one playing)
    ep_season: Option<i64>,
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
                let show = paused || v.menu.is_some() || v.last_move.elapsed() < Duration::from_millis(2800) || v.pb.read(cx).loading;
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
        let reactions = cx.new(super::reactions::Reactions::new);
        cx.observe(&reactions, |_, _, cx| cx.notify()).detach();
        Self { pb, menu: None, stills: Default::default(), vol_hud: None, last_move: Instant::now(), shown: true, hover_x: None, reactions, next_hover: false, vol_acc: 0., vol_target: None, drag: false, track_w: 1., vol_open: false, vol_x: Default::default(), aspect: "fit", ep_season: None }
    }

    pub fn poke(&mut self, cx: &mut Context<Self>) {
        self.last_move = Instant::now();
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

fn next_pill(pb: Entity<Playback>, label: String) -> gpui::Stateful<gpui::Div> {
    pill("pl-upnext", format!("next episode · {label}")).on_click(move |_, _, cx| {
        cx.stop_propagation();
        if may("episode", None, cx) {
            pb.update(cx, |p, cx| p.play_next(cx))
        }
    })
}

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
        .hover(|s| s.bg(theme::white(0.14)))
        .child(child)
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
        let vol = (((x - self.vol_x.get()) / 96.).clamp(0., 1.) as f64 * max).round();
        self.vol_target = Some(vol);
        self.pb.read(cx).cmd("volume", json!({ "value": vol }));
        cx.notify();
    }

    fn want_still(&mut self, t: f64, cx: &mut Context<Self>) -> Option<std::sync::Arc<gpui::Image>> {
        let k = (t / 10.).floor() as i64;
        if let Some(v) = self.stills.get(&k) {
            return v.clone();
        }
        self.stills.insert(k, None);
        cx.spawn(async move |this, cx| {
            let r = api_still(k as f64 * 10. + 5.).await;
            let _ = this.update(cx, |v, cx| {
                v.stills.insert(k, r);
                cx.notify();
            });
        })
        .detach();
        None
    }

    fn menu_panel(&self, m: Menu, cx: &mut Context<Self>) -> gpui::AnyElement {
        let p = self.pb.read(cx);
        let row_f = |id: SharedString, label: String, on: bool, flag: Option<String>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(14.))
                .h(px(42.))
                .rounded(px(10.))
                .cursor_pointer()
                .text_size(px(16.))
                .font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                .hover(|s| s.bg(theme::white(0.1)))
                .child(div().w(px(18.)).children(on.then(|| icon("check", 18.))))
                .children(flag.map(|u| img(u).h(px(18.)).flex_none().object_fit(ObjectFit::Contain)))
                .child(label)
        };
        let row = |id: SharedString, label: String, on: bool| row_f(id, label, on, None);

        let (title, rows): (&str, Vec<gpui::AnyElement>) = match m {
            Menu::Audio | Menu::Subs => {
                let kind = if m == Menu::Audio { "audio" } else { "sub" };
                let list: Vec<serde_json::Value> = p.tracks.iter().filter(|t| t["type"] == kind).cloned().collect();
                let mut rows: Vec<gpui::AnyElement> = vec![];
                if m == Menu::Subs {
                    let off = !list.iter().any(|t| t["selected"].as_bool() == Some(true));
                    let pb = self.pb.clone();
                    rows.push(row("sub-off".into(), "Off".into(), off).on_click(move |_, _, cx| pb.read(cx).cmd("sid", json!({ "value": "no" }))).into_any_element());
                }
                for (i, (t, label)) in named(&list).into_iter().enumerate() {
                    let on = t["selected"].as_bool() == Some(true);
                    let id = t["id"].as_i64().unwrap_or(0);
                    let pb = self.pb.clone();
                    let cmd = if m == Menu::Audio { "aid" } else { "sid" };
                    let flag = crate::ui::badges::flag(t["lang"].as_str().unwrap_or(""));
                    rows.push(row_f(SharedString::from(format!("{kind}-{i}")), label, on, flag).on_click(move |_, _, cx| pb.read(cx).cmd(cmd, json!({ "value": id }))).into_any_element());
                }
                // subtitles early or late? nudge them
                if m == Menu::Subs && list.iter().any(|t| t["selected"].as_bool() == Some(true)) {
                    let d = p.state.sub_delay;
                    let nudge = |id: &'static str, label: &'static str, by: f64| {
                        let pb = self.pb.clone();
                        div()
                            .id(id)
                            .px(px(12.))
                            .h(px(30.))
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
                    rows.push(
                        div()
                            .mt(px(6.))
                            .px(px(14.))
                            .py(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().flex_1().text_size(px(13.)).text_color(theme::label3()).child(format!("out of sync? {}{:.1} s", if d > 0. { "+" } else { "" }, d)))
                            .child(nudge("sd-minus", "−0.1", -0.1))
                            .child(nudge("sd-plus", "+0.1", 0.1))
                            .into_any_element(),
                    );
                }
                (if m == Menu::Audio { "audio" } else { "subtitles" }, rows)
            }
            Menu::Episodes => {
                let Some(f) = p.full.clone() else { return div().into_any_element() };
                let playing = playback::ordered(&f).into_iter().find(|v| v["id"].as_str() == Some(p.video_id.as_str())).and_then(|v| v["season"].as_i64()).unwrap_or(1);
                let cur = self.ep_season.unwrap_or(playing);
                let mut seasons: Vec<i64> = playback::ordered(&f).into_iter().filter_map(|v| v["season"].as_i64()).collect();
                seasons.sort();
                seasons.dedup();
                let meta = p.meta.clone();
                let picker = (seasons.len() > 1).then(|| {
                    div()
                        .id("ep-seasons")
                        .px(px(8.))
                        .pb(px(8.))
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .children(seasons.iter().map(|n| {
                            let n = *n;
                            div()
                                .id(("ep-s", n as usize))
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
                        .into_any_element()
                });
                let rows: Vec<gpui::AnyElement> = picker.into_iter().chain(playback::ordered(&f)
                    .into_iter()
                    .filter(|v| v["season"].as_i64() == Some(cur))
                    .map(|v| {
                        let id = v["id"].as_str().unwrap_or("").to_string();
                        let on = id == p.video_id;
                        let label = format!("{}. {}", v["episode"].as_i64().or(v["number"].as_i64()).unwrap_or(0), playback::episode_name(&v));
                        let m = meta.clone();
                        row(SharedString::from(format!("ep-{id}")), label, on)
                            .on_click(move |_, _, cx| {
                                if let Some(m) = m.clone() {
                                    crate::Nav::play_episode(&m, id.clone(), cx);
                                }
                            })
                            .into_any_element()
                    }))
                    .collect();
                ("episodes", rows)
            }
            Menu::Speed => {
                let cur = p.state.speed;
                let rows = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0]
                    .into_iter()
                    .map(|v| {
                        let pb = self.pb.clone();
                        row(SharedString::from(format!("sp-{v}")), if v == 1.0 { "normal".into() } else { format!("{v}x") }, (snap(cur) - v).abs() < 0.01)
                            .on_click(move |_, _, cx| {
                                if may("speed", None, cx) {
                                    pb.read(cx).cmd("speed", json!({ "value": v }))
                                }
                            })
                            .into_any_element()
                    })
                    .collect();
                let mut rows: Vec<gpui::AnyElement> = rows;
                rows.push(div().px(px(14.)).pt(px(12.)).pb(px(4.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child("sleep timer").into_any_element());
                let sleep = p.sleep;
                for (label, v) in [("off", 0i64), ("end of this episode", -1), ("15 min", 15), ("30 min", 30), ("60 min", 60)] {
                    let pb = self.pb.clone();
                    let on = match (sleep, v) {
                        (None, 0) => true,
                        (Some(playback::Sleep::Episode), -1) => true,
                        (Some(playback::Sleep::At(_, m)), x) => m == x,
                        _ => false,
                    };
                    rows.push(
                        row(SharedString::from(format!("sl-{v}")), label.into(), on)
                            .on_click(move |_, _, cx| pb.update(cx, |p, cx| p.set_sleep(v, cx)))
                            .into_any_element(),
                    );
                }
                rows.push(div().px(px(14.)).pt(px(12.)).pb(px(4.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child("picture").into_any_element());
                for (label, v) in [("fit", "fit"), ("fill the screen", "fill"), ("stretch", "stretch")] {
                    rows.push(
                        row(SharedString::from(format!("as-{v}")), label.into(), self.aspect == v)
                            .on_click(cx.listener(move |me, _, _, cx| {
                                me.aspect = v;
                                me.pb.read(cx).cmd("aspect", json!({ "value": v }));
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
                ("speed", rows)
            }
            Menu::Sources => {
                // every stream found, best first; the one playing is ticked
                let cur = p.stream.as_ref().map(playback::stream_key).unwrap_or_default();
                let rows = p
                    .all
                    .iter()
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
                            .px(px(14.))
                            .py(px(8.))
                            .rounded(px(10.))
                            .cursor_pointer()
                            .hover(|d| d.bg(theme::white(0.1)))
                            .child(div().w(px(18.)).flex_none().children(on.then(|| icon("check", 18.))))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(div().text_size(px(15.)).font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM }).child(bits.join(" · ")))
                                    .child(div().text_size(px(12.)).text_color(theme::label3()).truncate().child(from)),
                            )
                            .on_click(move |_, _, cx| {
                                if !on {
                                    pb.update(cx, |p, cx| p.pick(st.clone(), cx));
                                }
                            })
                            .into_any_element()
                    })
                    .collect();
                ("streams", rows)
            }
        };
        div()
            .id("pl-menu")
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .absolute()
            .right(px(theme::GUTTER))
            .bottom(px(100.))
            .w(px(320.))
            .max_h(px(460.))
            .overflow_y_scroll()
            .p(px(8.))
            .rounded(px(22.))
            .bg(gpui::rgba(0x151114f0))
            .border_1()
            .border_color(theme::white(0.10))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(div().px(px(14.)).py(px(8.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child(title))
            .children(rows)
            .into_any_element()
    }
}

impl Render for PlayerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pb.read(cx);
        let vw = window.viewport_size().width;
        let g = theme::GUTTER;
        let bar_w: Pixels = vw - px(g * 2.);
        let st = p.state.clone();
        let frac = if st.duration > 0. { (st.pos / st.duration).clamp(0., 1.) as f32 } else { 0. };
        let ahead = if st.duration > 0. { ((st.pos + st.cache_ahead) / st.duration).clamp(0., 1.) as f32 } else { 0. };
        let title = p.meta.as_ref().map(|m| m.name.clone()).unwrap_or_default();
        let ep = p.episode_title.clone();
        let label = p.full.as_ref().and_then(|f| {
            let v = playback::ordered(f).into_iter().find(|v| v["id"].as_str() == Some(p.video_id.as_str()))?;
            Some(format!("S{} E{}", v["season"].as_i64()?, v["episode"].as_i64().or(v["number"].as_i64())?))
        });
        let shown = self.shown || p.loading || !p.error.is_empty();
        let art = p.meta.as_ref().and_then(|m| m.background.clone().or(m.poster.clone())).map(|u| sized(&u, "original"));
        let logo = p.meta.as_ref().and_then(|m| m.logo.clone()).map(|u| crate::net::logo(&sized(&u, "large"), 480.));
        let starting = p.loading && st.pos < 0.5;

        let mut root = div()
            .id("player")
            .size_full()
            .relative()
            .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _, cx| {
                if v.drag {
                    if e.pressed_button == Some(gpui::MouseButton::Left) {
                        v.hover_x = Some(f32::from(e.position.x) - theme::GUTTER);
                        cx.notify();
                    } else {
                        v.finish_drag(cx);
                    }
                }
                v.poke(cx)
            }))
            .on_mouse_up(gpui::MouseButton::Left, cx.listener(|v, _, _, cx| v.finish_drag(cx)))
            .on_click(cx.listener(|v, e: &gpui::ClickEvent, window, cx| {
                if just_moved() {
                    return;
                }
                if e.click_count() == 2 {
                    // double click: full screen (undo the first click's pause)
                    window.toggle_fullscreen();
                    let paused = v.pb.read(cx).state.paused;
                    if may(if paused { "play" } else { "pause" }, None, cx) {
                        v.pb.read(cx).cmd("pause", json!({ "value": !paused }));
                    }
                    return;
                }
                // a click on the picture: play / pause (double click: full screen)
                if v.menu.is_some() {
                    v.menu = None;
                    cx.notify();
                } else if e.click_count() == 1 {
                    let paused = v.pb.read(cx).state.paused;
                    if may(if paused { "play" } else { "pause" }, None, cx) {
                        v.pb.read(cx).cmd("pause", json!({ "value": !paused }));
                    }
                }
                v.poke(cx);
            }))
            .on_scroll_wheel(cx.listener(|v, e: &gpui::ScrollWheelEvent, w, cx| {
                // scroll = volume
                // 5% a notch, however fast you spin (fast spins arrive as bigger steps)
                let step = match e.delta {
                    gpui::ScrollDelta::Lines(l) => l.y / 3. * 5.,
                    gpui::ScrollDelta::Pixels(p) => f32::from(p.y) / 40. * 5.,
                };
                let _ = w;
                v.vol_acc += step as f64;
                if v.vol_acc.abs() < 1. {
                    return;
                }
                let change = v.vol_acc.trunc();
                v.vol_acc -= change;
                let p = v.pb.read(cx);
                let max = if couch::state::get().core.settings().playback.volume_boost { 130. } else { 100. };
                // build on the volume we last asked for (mpv's reply lags behind fast spins)
                let base = v.vol_target.filter(|_| v.vol_hud.map_or(false, |t| t.elapsed() < Duration::from_millis(800))).unwrap_or(p.state.volume);
                let vol = (base + change).clamp(0., max).round();
                v.vol_target = Some(vol);
                p.cmd("volume", json!({ "value": vol }));
                v.vol_hud = Some(Instant::now());
                cx.notify();
            }));

        // while it starts: the title's art under a dim, the logo, a line
        if starting || !p.error.is_empty() {
            root = root
                .bg(theme::bg())
                .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).opacity(0.35)))
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
                        .gap(px(18.))
                        .child(match logo.clone() {
                            Some(u) => img(u).max_w(px(420.)).max_h(px(140.)).object_fit(ObjectFit::Contain).into_any_element(),
                            None => div().text_size(px(48.)).font_weight(FontWeight::BLACK).child(title.clone()).into_any_element(),
                        })
                        .child(
                            div()
                                .text_size(px(16.))
                                .text_color(if p.error.is_empty() { theme::label2() } else { gpui::rgb(0xff8a8a) })
                                .child(if p.error.is_empty() { p.status.clone() } else { p.error.clone() }),
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

        // subtitles, drawn by couch
        if !p.sub.is_empty() {
            root = root.child(
                div().absolute().left_0().w_full().bottom(px(if shown { 160. } else { 64. })).flex().justify_center().child(
                    div()
                        .max_w(px(1100.))
                        .px(px(16.))
                        .py(px(6.))
                        .rounded(px(8.))
                        .bg(theme::black(0.55))
                        .text_center()
                        // subtitles in Figtree (open, easy to read at a glance)
                        .font_family("Figtree")
                        .text_size(px(30.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(px(40.))
                        .text_color(gpui::rgb(0xffffff))
                        .child(p.sub.clone()),
                ),
            );
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
        let root = root.child(self.reactions.clone()).children(denied_sign(cx, window));
        if !shown {
            return root;
        }

        // small window (picture in picture): just play/pause, ±10, a thin bar, and back to big
        if f32::from(vw) < 640. {
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
                .child(
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
                )
                .child(div().absolute().left(px(10.)).right(px(10.)).bottom(px(10.)).h(px(4.)).rounded_full().bg(theme::white(0.25)).child(div().h_full().rounded_full().w((vw - px(20.)) * frac).bg(theme::neon())));
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
        let near_end = st.duration > 300. && st.duration - st.pos < 45. && next.is_some();
        let has_streams = !p.all.is_empty();
        let menu = self.menu.map(|m| self.menu_panel(m, cx));
        let root = root.children(menu);
        // skip intro / recap; at the credits (or the last bit): next episode
        let skip = match &segment {
            Some((k, end)) if k == "intro" || k == "recap" => {
                let (end, pb) = (*end, self.pb.clone());
                Some(pill("pl-skip", if k == "intro" { "skip intro".into() } else { "skip recap".into() }).on_click(move |_, _, cx| {
                    if may("seek", Some(end), cx) {
                        pb.read(cx).cmd("seek", json!({ "to": end }));
                    }
                }))
            }
            Some((k, _)) if k == "credits" && next.is_some() => Some(next_pill(self.pb.clone(), next.clone().unwrap_or_default())),
            _ if near_end => Some(next_pill(self.pb.clone(), next.clone().unwrap_or_default())),
            _ => None,
        };
        let root = root.children(skip.map(|b| div().absolute().right(px(theme::GUTTER)).bottom(px(140.)).child(b)));
        // what's next, while you point at "next"
        let next_card = self.next_hover.then(|| self.pb.read(cx).next_video()).flatten().map(|n| {
            let imdb = crate::api::imdb_episode(n["id"].as_str().unwrap_or(""));
            let still = n["thumbnail"].as_str().map(String::from).or(imdb.clone());
            let over = n["overview"].as_str().or(n["description"].as_str()).unwrap_or("").to_string();
            div()
                .absolute()
                .right(px(theme::GUTTER))
                .bottom(px(100.))
                .w(px(340.))
                .rounded(px(16.))
                .overflow_hidden()
                .bg(theme::black(0.85))
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
        let root = root.children(next_card);
        let tw: f32 = f32::from(bar_w) - 92.;
        self.track_w = tw;
        let dragging = self.drag;
        let shown_frac = match (dragging, hover) {
            (true, Some(x)) => (x / tw.max(1.)).clamp(0., 1.),
            _ => frac,
        };
        let fat = dragging || hover.is_some();
        let segs: Vec<(f32, f32)> = if st.duration > 0. {
            self.pb
                .read(cx)
                .segments
                .as_object()
                .into_iter()
                .flat_map(|o| o.values())
                .flat_map(|l| l.as_array().into_iter().flatten())
                .map(|sg| {
                    let a = sg["start"].as_f64().unwrap_or(0.);
                    let b = sg["end"].as_f64().unwrap_or(st.duration);
                    ((a / st.duration) as f32, ((b - a).max(0.) / st.duration) as f32)
                })
                .collect()
        } else {
            vec![]
        };
        let menu_on = self.menu;
        let vol_now = self.vol_target.filter(|_| self.vol_open).unwrap_or(st.volume);
        let vol_max = if couch::state::get().core.settings().playback.volume_boost { 130. } else { 100. };
        let vol_frac = (vol_now / vol_max).clamp(0., 1.) as f32;
        let vol_x = self.vol_x.clone();
        let now_line = [label.clone().unwrap_or_default(), ep.clone()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        let top_logo = logo.clone();
        let top_title = title.clone();
        root
            // top: back, the title's logo
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h(px(150.))
                    .bg(linear_gradient(180., linear_color_stop(theme::black(0.65), 0.), linear_color_stop(theme::black(0.0), 1.)))
                    .child(
                        div()
                            .absolute()
                            .left(px(g))
                            .top(px(28.))
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
                            }),
                    ),
            )
            // what's playing, as badges (4K, Dolby Vision, Atmos...)
            .child({
                let p = self.pb.read(cx);
                let height = p.tracks.iter().find(|t| t["type"] == "video" && t["selected"] == true).and_then(|t| t["height"].as_i64()).unwrap_or(0);
                let codec = p.tracks.iter().find(|t| t["type"] == "audio" && t["selected"] == true).and_then(|t| t["codec"].as_str()).unwrap_or("").to_string();
                let c = p.stream.as_ref().map(|s| s["_couch"].clone()).unwrap_or_default();
                div()
                    .absolute()
                    .top(px(40.))
                    .right(px(g))
                    .flex()
                    .gap(px(8.))
                    .children(crate::ui::badges::format(&c, height, &codec).into_iter().map(|b| img(b.src).h(px(24.)).flex_none().object_fit(ObjectFit::Contain)))
            })
            // bottom: the scrub bar, then one glass bar with everything on it
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .h(px(210.))
                    .bg(linear_gradient(180., linear_color_stop(theme::black(0.0), 0.), linear_color_stop(theme::black(0.72), 1.)))
                    .child(
                        div()
                            .id("pl-scrub")
                            .absolute()
                            .left(px(g))
                            .bottom(px(98.))
                            .w(px(tw))
                            .h(px(26.))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |v, e: &gpui::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                v.drag = true;
                                v.hover_x = Some(f32::from(e.position.x) - g);
                                cx.notify();
                            }))
                            .on_mouse_move(cx.listener(move |v, e: &MouseMoveEvent, _, cx| {
                                v.hover_x = Some(f32::from(e.position.x) - g);
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
                                    .children(segs.into_iter().map(|(a, w)| div().absolute().top_0().h_full().left(px(tw * a)).w(px((tw * w).max(2.))).rounded_full().bg(theme::white(0.45))))
                                    .child(div().absolute().top_0().left_0().h_full().w(px(tw * shown_frac)).rounded_full().bg(theme::neon()))
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
                            ),
                    )
                    // time left, at the end of the bar
                    .child(
                        div()
                            .absolute()
                            .right(px(g))
                            .bottom(px(98.))
                            .w(px(80.))
                            .h(px(26.))
                            .flex()
                            .items_center()
                            .justify_end()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::label2())
                            .child(if st.duration > 0. { format!("-{}", clock(st.duration - st.pos)) } else { String::new() }),
                    )
                    .child(
                        glass(div())
                            .absolute()
                            .left(px(g - 6.))
                            .right(px(g - 6.))
                            .bottom(px(24.))
                            .h(px(62.))
                            .px(px(9.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .child(bar_btn("pl-play", if paused { icon_fill("play", 28.) } else { icon_fill("pause", 28.) }, false).on_click(
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
                            }))
                            // what's on, in words
                            .child(
                                div()
                                    .ml(px(12.))
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .justify_center()
                                    .child(div().text_size(px(15.)).font_weight(FontWeight::BLACK).truncate().child(title))
                                    .children((!now_line.is_empty()).then(|| div().text_size(px(13.)).text_color(theme::label2()).truncate().child(now_line))),
                            )
                            .children(next.map(|_| {
                                let pb = self.pb.clone();
                                bar_btn("pl-next", icon("next", 22.), false)
                                    .on_click(move |_, _, cx| {
                                        if may("episode", None, cx) {
                                            pb.update(cx, |p, cx| p.play_next(cx))
                                        }
                                    })
                                    .on_hover(cx.listener(|v, on: &bool, _, cx| {
                                        v.next_hover = *on;
                                        cx.notify();
                                    }))
                            }))
                            // volume: the slider slides out while you're on it
                            .child(
                                div()
                                    .id("pl-vol")
                                    .occlude()
                                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                    .flex()
                                    .items_center()
                                    .rounded_full()
                                    .when(self.vol_open, |d| d.bg(theme::white(0.08)).pr(px(14.)))
                                    .on_hover(cx.listener(|v, on: &bool, _, cx| {
                                        v.vol_open = *on;
                                        if !*on {
                                            v.vol_target = None;
                                        }
                                        cx.notify();
                                    }))
                                    .child(
                                        bar_btn("pl-mute", icon(if st.muted || st.volume <= 0. { "mute" } else { "volume" }, 23.), false)
                                            .on_click(move |_, _, cx| pb4.read(cx).cmd("mute", json!({}))),
                                    )
                                    .when(self.vol_open, |d| {
                                        d.child(
                                            div()
                                                .id("pl-vol-slide")
                                                .w(px(96.))
                                                .h(px(44.))
                                                .flex()
                                                .items_center()
                                                .cursor_pointer()
                                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|v, e: &gpui::MouseDownEvent, _, cx| {
                                                    cx.stop_propagation();
                                                    v.set_volume_at(f32::from(e.position.x), cx);
                                                }))
                                                .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _, cx| {
                                                    if e.pressed_button == Some(gpui::MouseButton::Left) {
                                                        v.set_volume_at(f32::from(e.position.x), cx);
                                                    }
                                                }))
                                                .child(
                                                    div()
                                                        .relative()
                                                        .w_full()
                                                        .h(px(4.))
                                                        .rounded_full()
                                                        .bg(theme::white(0.22))
                                                        .child(gpui::canvas(move |b, _, _| vol_x.set(f32::from(b.origin.x)), |_, _, _, _| {}).absolute().size_full())
                                                        .child(div().h_full().rounded_full().bg(theme::text()).w(px(96. * vol_frac)))
                                                        .child(div().absolute().top(px(-5.)).left(px(96. * vol_frac - 7.)).size(px(14.)).rounded_full().bg(theme::text())),
                                                ),
                                        )
                                    }),
                            )
                            .when(super::reactions::room(cx).is_some(), |d| {
                                let r = self.reactions.clone();
                                d.child(bar_btn("pl-react", crate::ui::emoji::emoji("😂", 24.), false).on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    r.update(cx, |r, cx| r.toggle(cx));
                                }))
                            })
                            .child(bar_btn("pl-subs", icon("subtitles", 23.), menu_on == Some(Menu::Subs)).on_click(cx.listener(|v, _, _, cx| {
                                cx.stop_propagation();
                                v.toggle_menu(Menu::Subs, cx);
                            })))
                            .child(bar_btn("pl-audio", icon("audio", 23.), menu_on == Some(Menu::Audio)).on_click(cx.listener(|v, _, _, cx| {
                                cx.stop_propagation();
                                v.toggle_menu(Menu::Audio, cx);
                            })))
                            .when(is_series, |d| {
                                d.child(bar_btn("pl-eps", icon("stack", 23.), menu_on == Some(Menu::Episodes)).on_click(cx.listener(|v, _, _, cx| {
                                    cx.stop_propagation();
                                    v.toggle_menu(Menu::Episodes, cx);
                                })))
                            })
                            .when(has_streams, |d| {
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
                            .child(bar_btn("pl-pip", icon("pip", 22.), false).on_click(|_, _, cx| crate::pip::toggle(cx)))
                            .child(bar_btn("pl-full", icon(if window.is_fullscreen() { "exitFullscreen" } else { "fullscreen" }, 22.), false).on_click(|_, window, _| window.toggle_fullscreen())),
                    ),
            )
    }
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
