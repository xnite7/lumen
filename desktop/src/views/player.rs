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
}

impl PlayerView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let pb = playback::get(cx);
        cx.observe(&pb, |_, _, cx| cx.notify()).detach();
        // controls fade away after a few still seconds
        cx.spawn(async move |this, cx| loop {
            gpui::Timer::after(Duration::from_millis(400)).await;
            let Ok(()) = this.update(cx, |v, cx| {
                let paused = v.pb.read(cx).state.paused;
                let show = paused || v.menu.is_some() || v.last_move.elapsed() < Duration::from_millis(2800) || v.pb.read(cx).loading;
                if show != v.shown {
                    v.shown = show;
                    cx.notify();
                }
            }) else {
                break;
            };
        })
        .detach();
        let reactions = cx.new(super::reactions::Reactions::new);
        cx.observe(&reactions, |_, _, cx| cx.notify()).detach();
        Self { pb, menu: None, stills: Default::default(), vol_hud: None, last_move: Instant::now(), shown: true, hover_x: None, reactions, next_hover: false }
    }

    pub fn poke(&mut self, cx: &mut Context<Self>) {
        self.last_move = Instant::now();
        if !self.shown {
            self.shown = true;
            cx.notify();
        }
    }
}

/// Watching on someone's couch: pause/seek go to the host (when they've
/// shared the remote); otherwise they stay with the host. True = do it here too.
pub fn may(action: &str, pos: Option<f64>, cx: &mut gpui::App) -> bool {
    let t = crate::together::get(cx);
    if t.read(cx).following.is_none() {
        return true;
    }
    let sent = t.update(cx, |t, _| t.steer(action, pos));
    if !sent {
        crate::Nav::toast("the host has the remote".into(), cx);
    }
    sent
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
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .hover(|s| s.opacity(0.88))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(label)
}

fn next_pill(pb: Entity<Playback>, label: String) -> gpui::Stateful<gpui::Div> {
    pill("pl-upnext", format!("next episode · {label}")).on_click(move |_, _, cx| {
        cx.stop_propagation();
        pb.update(cx, |p, cx| p.play_next(cx))
    })
}

fn round_btn(id: &'static str, child: impl IntoElement, size: f32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
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
                .font_weight(if on { FontWeight::BOLD } else { FontWeight::MEDIUM })
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
                (if m == Menu::Audio { "audio" } else { "subtitles" }, rows)
            }
            Menu::Episodes => {
                let Some(f) = p.full.clone() else { return div().into_any_element() };
                let cur = playback::ordered(&f).into_iter().find(|v| v["id"].as_str() == Some(p.video_id.as_str())).and_then(|v| v["season"].as_i64()).unwrap_or(1);
                let meta = p.meta.clone();
                let rows = playback::ordered(&f)
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
                    })
                    .collect();
                ("episodes", rows)
            }
            Menu::Speed => {
                let cur = p.state.speed;
                let rows = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0]
                    .into_iter()
                    .map(|v| {
                        let pb = self.pb.clone();
                        row(SharedString::from(format!("sp-{v}")), if v == 1.0 { "normal".into() } else { format!("{v}x") }, (cur - v).abs() < 0.01)
                            .on_click(move |_, _, cx| pb.read(cx).cmd("speed", json!({ "value": v })))
                            .into_any_element()
                    })
                    .collect();
                let mut rows: Vec<gpui::AnyElement> = rows;
                rows.push(div().px(px(14.)).pt(px(12.)).pb(px(4.)).text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(theme::label3()).child("sleep timer").into_any_element());
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
                ("speed", rows)
            }
        };
        div()
            .id("pl-menu")
            .absolute()
            .right(px(theme::GUTTER))
            .bottom(px(96.))
            .w(px(300.))
            .max_h(px(420.))
            .overflow_y_scroll()
            .p(px(8.))
            .rounded(px(18.))
            .bg(gpui::rgba(0x0c090bf2))
            .border_1()
            .border_color(theme::white(0.1))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(div().px(px(14.)).py(px(8.)).text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(theme::label3()).child(title))
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
        let logo = p.meta.as_ref().and_then(|m| m.logo.clone()).map(|u| sized(&u, "large"));
        let starting = p.loading && st.pos < 0.5;

        let mut root = div()
            .id("player")
            .size_full()
            .relative()
            .on_mouse_move(cx.listener(|v, _: &MouseMoveEvent, _, cx| v.poke(cx)))
            .on_click(cx.listener(|v, e: &gpui::ClickEvent, window, cx| {
                if e.click_count() == 2 {
                    // double click: full screen (undo the first click's pause)
                    window.toggle_fullscreen();
                    let paused = v.pb.read(cx).state.paused;
                    v.pb.read(cx).cmd("pause", json!({ "value": !paused }));
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
            .when(!shown, |d| d.cursor(gpui::CursorStyle::None))
            .on_scroll_wheel(cx.listener(|v, e: &gpui::ScrollWheelEvent, w, cx| {
                // scroll = volume
                let dy = f32::from(e.delta.pixel_delta(w.line_height()).y);
                if dy.abs() < 0.5 {
                    return;
                }
                let p = v.pb.read(cx);
                let max = if couch::state::get().core.settings().playback.volume_boost { 130. } else { 100. };
                let vol = (p.state.volume + if dy > 0. { 5. } else { -5. }).clamp(0., max);
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
                        .child(match logo {
                            Some(u) => img(u).max_w(px(420.)).max_h(px(140.)).object_fit(ObjectFit::Contain).into_any_element(),
                            None => div().text_size(px(48.)).font_weight(FontWeight::BLACK).child(title.clone()).into_any_element(),
                        })
                        .child(
                            div()
                                .text_size(px(16.))
                                .text_color(if p.error.is_empty() { theme::label2() } else { gpui::rgb(0xff8a8a) })
                                .child(if p.error.is_empty() { p.status.clone() } else { p.error.clone() }),
                        ),
                );
        }

        // subtitles, drawn by couch
        if !p.sub.is_empty() {
            root = root.child(
                div().absolute().left_0().w_full().bottom(px(if shown { 150. } else { 64. })).flex().justify_center().child(
                    div()
                        .max_w(px(1100.))
                        .px(px(16.))
                        .py(px(6.))
                        .rounded(px(8.))
                        .bg(theme::black(0.55))
                        .text_center()
                        .text_size(px(30.))
                        .font_weight(FontWeight::BOLD)
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
                        .rounded(px(14.))
                        .bg(theme::black(0.7))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(icon(if st.volume <= 0. { "mute" } else { "volume" }, 22.))
                        .child(div().font_weight(FontWeight::BOLD).child(format!("{}%", st.volume.round() as i64))),
                ),
            );
            cx.spawn(async move |this, cx| {
                gpui::Timer::after(Duration::from_millis(1300)).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        let root = root.child(self.reactions.clone());
        if !shown {
            return root;
        }

        // small window (picture in picture): just play/pause, ±10, a thin bar, and back to big
        if f32::from(vw) < 640. {
            let paused = st.paused;
            let (pa, pb, pc) = (self.pb.clone(), self.pb.clone(), self.pb.clone());
            return root
                .child(div().absolute().top_0().left_0().size_full().bg(theme::black(0.35)))
                .child(
                    div().absolute().top(px(8.)).right(px(8.)).flex().gap(px(4.))
                        .child(round_btn("pip-big", icon("expand", 20.), 36.).on_click(|_, _, cx| crate::pip::toggle(cx)))
                        .child(round_btn("pip-close", icon("close", 18.), 36.).on_click(move |_, _, cx| {
                            crate::pip::toggle(cx);
                            pc.update(cx, |p, cx| p.close(cx));
                            crate::Nav::back(cx);
                        })),
                )
                .child(
                    div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().gap(px(18.))
                        .child(round_btn("pip-b10", icon("replay10", 26.), 46.).on_click(move |_, _, cx| pa.read(cx).cmd("seek", json!({ "to": -10, "relative": true }))))
                        .child(round_btn("pip-play", if paused { icon_fill("play", 34.) } else { icon_fill("pause", 34.) }, 60.).on_click({
                            let pb = self.pb.clone();
                            move |_, _, cx| {
                                let paused = pb.read(cx).state.paused;
                                if may(if paused { "play" } else { "pause" }, None, cx) {
                                    pb.read(cx).cmd("pause", json!({ "value": !paused }));
                                }
                            }
                        }))
                        .child(round_btn("pip-f10", icon("forward10", 26.), 46.).on_click(move |_, _, cx| pb.read(cx).cmd("seek", json!({ "to": 10, "relative": true })))),
                )
                .child(div().absolute().left(px(10.)).right(px(10.)).bottom(px(10.)).h(px(4.)).rounded_full().bg(theme::white(0.25)).child(div().h_full().rounded_full().w((vw - px(20.)) * frac).bg(theme::neon())));
        }

        let paused = st.paused;
        let pb = self.pb.clone();
        let pb2 = self.pb.clone();
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
        let root = root.children(skip.map(|b| div().absolute().right(px(theme::GUTTER)).bottom(px(130.)).child(b)));
        // what's next, while you point at "next"
        let next_card = self.next_hover.then(|| self.pb.read(cx).next_video()).flatten().map(|n| {
            let still = n["thumbnail"].as_str().map(String::from);
            let over = n["overview"].as_str().or(n["description"].as_str()).unwrap_or("").to_string();
            div()
                .absolute()
                .right(px(theme::GUTTER))
                .bottom(px(96.))
                .w(px(340.))
                .rounded(px(16.))
                .overflow_hidden()
                .bg(theme::black(0.85))
                .border_1()
                .border_color(theme::white(0.12))
                .flex()
                .flex_col()
                .children(still.map(|u| img(u).w_full().h(px(191.)).object_fit(ObjectFit::Cover)))
                .child(
                    div()
                        .p(px(14.))
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(div().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(theme::label3()).child(format!("NEXT · S{} E{}", n["season"].as_i64().unwrap_or(0), n["episode"].as_i64().or(n["number"].as_i64()).unwrap_or(0))))
                        .child(div().text_size(px(17.)).font_weight(FontWeight::BLACK).child(playback::episode_name(&n)))
                        .children((!over.is_empty()).then(|| div().text_size(px(13.)).line_height(px(18.)).text_color(theme::label2()).line_clamp(3).child(over))),
                )
        });
        let root = root.children(next_card);
        root
            // top: back, title
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h(px(130.))
                    .bg(linear_gradient(180., linear_color_stop(theme::black(0.7), 0.), linear_color_stop(theme::black(0.0), 1.)))
                    .child(
                        div()
                            .absolute()
                            .left(px(g - 10.))
                            .top(px(26.))
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(
                                round_btn("pl-back", icon("back", 26.), 48.).on_click(move |_, _, cx| {
                                    // back keeps the film playing in the corner
                                    let _ = &pb;
                                    crate::Nav::back(cx);
                                }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(div().text_size(px(22.)).font_weight(FontWeight::BLACK).child(title))
                                    .children((!ep.is_empty() || label.is_some()).then(|| {
                                        div()
                                            .text_size(px(15.))
                                            .text_color(theme::label2())
                                            .child([label.unwrap_or_default(), ep].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "))
                                    })),
                            ),
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
                    .top(px(34.))
                    .right(px(g))
                    .flex()
                    .gap(px(8.))
                    .children(crate::ui::badges::format(&c, height, &codec).into_iter().map(|b| img(b.src).h(px(26.)).flex_none().object_fit(ObjectFit::Contain)))
            })
            // bottom: the scrub bar and the buttons
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .h(px(170.))
                    .bg(linear_gradient(180., linear_color_stop(theme::black(0.0), 0.), linear_color_stop(theme::black(0.8), 1.)))
                    .child(
                        div()
                            .id("pl-scrub")
                            .absolute()
                            .left(px(g))
                            .bottom(px(86.))
                            .w(bar_w)
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .on_mouse_move(cx.listener(move |v, e: &MouseMoveEvent, _, cx| {
                                let x = f32::from(e.position.x) - g;
                                v.hover_x = Some(x);
                                cx.notify();
                            }))
                            .on_hover(cx.listener(|v, on: &bool, _, cx| {
                                if !*on {
                                    v.hover_x = None;
                                    cx.notify();
                                }
                            }))
                            .on_click(move |e, _, cx| {
                                let x = f32::from(e.position().x) - g;
                                let w = f32::from(bar_w).max(1.);
                                let d = pb2.read(cx).state.duration;
                                let to = (x / w).clamp(0., 1.) as f64 * d;
                                if d > 0. && may("seek", Some(to), cx) {
                                    pb2.read(cx).cmd("seek", json!({ "to": to }));
                                }
                            })
                            .child(
                                div()
                                    .relative()
                                    .w_full()
                                    .h(px(5.))
                                    .rounded_full()
                                    .bg(theme::white(0.22))
                                    .child(div().absolute().top_0().left_0().h_full().w(bar_w * ahead).rounded_full().bg(theme::white(0.35)))
                                    .child(div().absolute().top_0().left_0().h_full().w(bar_w * frac).rounded_full().bg(theme::neon()))
                                    .child(
                                        div()
                                            .absolute()
                                            .top(px(-5.5))
                                            .left(bar_w * frac - px(8.))
                                            .size(px(16.))
                                            .rounded_full()
                                            .bg(theme::text()),
                                    )
                                    .children(hover.filter(|_| st.duration > 0.).map(|x| {
                                        let w = f32::from(bar_w).max(1.);
                                        let t = (x / w).clamp(0., 1.) as f64 * st.duration;
                                        let still = still_img.clone();
                                        div()
                                            .absolute()
                                            .bottom(px(16.))
                                            .left(px((x - 100.).clamp(0., w - 200.)))
                                            .w(px(200.))
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .gap(px(6.))
                                            .children(still.map(|i| {
                                                img(i).w(px(200.)).h(px(112.)).rounded(px(10.)).border_2().border_color(theme::white(0.85)).object_fit(ObjectFit::Cover)
                                            }))
                                            .child(div().px(px(10.)).py(px(4.)).rounded(px(8.)).bg(theme::black(0.8)).text_size(px(14.)).font_weight(FontWeight::BOLD).child(clock(t)))
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(g - 10.))
                            .right(px(g - 10.))
                            .bottom(px(24.))
                            .h(px(56.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(round_btn("pl-play", if paused { icon_fill("play", 34.) } else { icon_fill("pause", 34.) }, 56.).on_click(
                                move |_, _, cx| {
                                    let paused = pb3.read(cx).state.paused;
                                    if may(if paused { "play" } else { "pause" }, None, cx) {
                                        pb3.read(cx).cmd("pause", json!({ "value": !paused }));
                                    }
                                },
                            ))
                            .child(round_btn("pl-back10", icon("replay10", 28.), 50.).on_click({
                                let pb = self.pb.clone();
                                move |_, _, cx| pb.read(cx).cmd("seek", json!({ "to": -10, "relative": true }))
                            }))
                            .child(round_btn("pl-fwd10", icon("forward10", 28.), 50.).on_click({
                                let pb = self.pb.clone();
                                move |_, _, cx| pb.read(cx).cmd("seek", json!({ "to": 10, "relative": true }))
                            }))
                            .child(
                                round_btn("pl-mute", icon(if st.muted || st.volume <= 0. { "mute" } else { "volume" }, 26.), 50.)
                                    .on_click(move |_, _, cx| pb4.read(cx).cmd("mute", json!({}))),
                            )
                            .child(
                                div()
                                    .ml(px(6.))
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("{} / {}", clock(st.pos), clock(st.duration))),
                            )
                            .child(div().flex_1())
                            .children(next.map(|n| {
                                let pb = self.pb.clone();
                                div()
                                    .id("pl-next")
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .h(px(44.))
                                    .px(px(16.))
                                    .rounded_full()
                                    .bg(theme::white(0.12))
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::BOLD)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme::white(0.2)))
                                    .on_click(move |_, _, cx| pb.update(cx, |p, cx| p.play_next(cx)))
                                    .on_hover(cx.listener(|v, on: &bool, _, cx| {
                                        v.next_hover = *on;
                                        cx.notify();
                                    }))
                                    .child(icon("next", 20.))
                                    .child(format!("next · {}", n))
                            }))
                            .when(is_series, |d| {
                                d.child(round_btn("pl-eps", icon("stack", 26.), 50.).on_click(cx.listener(|v, _, _, cx| {
                                    cx.stop_propagation();
                                    v.menu = if v.menu == Some(Menu::Episodes) { None } else { Some(Menu::Episodes) };
                                    cx.notify();
                                })))
                            })
                            .when(super::reactions::room(cx).is_some(), |d| {
                                let r = self.reactions.clone();
                                d.child(round_btn("pl-react", crate::ui::emoji::emoji("😂", 28.), 50.).on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    r.update(cx, |r, cx| r.toggle(cx));
                                }))
                            })
                            .child(round_btn("pl-subs", icon("subtitles", 26.), 50.).on_click(cx.listener(|v, _, _, cx| {
                                cx.stop_propagation();
                                v.menu = if v.menu == Some(Menu::Subs) { None } else { Some(Menu::Subs) };
                                cx.notify();
                            })))
                            .child(round_btn("pl-audio", icon("audio", 26.), 50.).on_click(cx.listener(|v, _, _, cx| {
                                cx.stop_propagation();
                                v.menu = if v.menu == Some(Menu::Audio) { None } else { Some(Menu::Audio) };
                                cx.notify();
                            })))
                            .child(
                                round_btn("pl-speed", div().text_size(px(15.)).font_weight(FontWeight::BLACK).child(format!("{}x", st.speed)), 50.).on_click(cx.listener(|v, _, _, cx| {
                                    cx.stop_propagation();
                                    v.menu = if v.menu == Some(Menu::Speed) { None } else { Some(Menu::Speed) };
                                    cx.notify();
                                })),
                            )
                            .child(round_btn("pl-pip", icon("pip", 26.), 50.).on_click(|_, _, cx| crate::pip::toggle(cx)))
                            .child(round_btn("pl-full", icon("fullscreen", 26.), 50.).on_click(|_, window, _| window.toggle_fullscreen())),
                    ),
            )
    }
}
