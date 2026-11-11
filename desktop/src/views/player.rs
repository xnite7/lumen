//! The player: mpv draws the picture underneath (its own child window);
//! this view is the see-through layer on top with couch's controls.
use crate::api::sized;
use crate::playback::{self, Playback};
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
}

pub struct PlayerView {
    pb: Entity<Playback>,
    menu: Option<Menu>,
    last_move: Instant,
    shown: bool,
    hover_x: Option<f32>,
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
        Self { pb, menu: None, last_move: Instant::now(), shown: true, hover_x: None }
    }

    pub fn poke(&mut self, cx: &mut Context<Self>) {
        self.last_move = Instant::now();
        if !self.shown {
            self.shown = true;
            cx.notify();
        }
    }
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
    fn menu_panel(&self, m: Menu, cx: &mut Context<Self>) -> gpui::AnyElement {
        let p = self.pb.read(cx);
        let row = |id: SharedString, label: String, on: bool| {
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
                .child(label)
        };
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
                    rows.push(row(SharedString::from(format!("{kind}-{i}")), label, on).on_click(move |_, _, cx| pb.read(cx).cmd(cmd, json!({ "value": id }))).into_any_element());
                }
                (if m == Menu::Audio { "audio" } else { "subtitles" }, rows)
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
                    v.pb.read(cx).cmd("pause", json!({ "value": !paused }));
                }
                v.poke(cx);
            }))
            .when(!shown, |d| d.cursor(gpui::CursorStyle::None));

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

        if !shown {
            return root;
        }

        let paused = st.paused;
        let pb = self.pb.clone();
        let pb2 = self.pb.clone();
        let pb3 = self.pb.clone();
        let pb4 = self.pb.clone();
        let hover = self.hover_x;
        let next = p.next_video().map(|n| format!("S{} E{}", n["season"].as_i64().unwrap_or(0), n["episode"].as_i64().or(n["number"].as_i64()).unwrap_or(0)));
        let menu = self.menu.map(|m| self.menu_panel(m, cx));
        let root = root.children(menu);
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
                                    pb.update(cx, |p, cx| p.close(cx));
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
                                if d > 0. {
                                    pb2.read(cx).cmd("seek", json!({ "to": (x / w).clamp(0., 1.) as f64 * d }));
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
                                        div()
                                            .absolute()
                                            .bottom(px(16.))
                                            .left(px(x - 34.))
                                            .w(px(68.))
                                            .py(px(4.))
                                            .rounded(px(8.))
                                            .bg(theme::black(0.8))
                                            .text_center()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::BOLD)
                                            .child(clock(t))
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
                                    let p = pb3.read(cx);
                                    p.cmd("pause", json!({ "value": !p.state.paused }));
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
                                    .child(icon("next", 20.))
                                    .child(format!("next · {}", n))
                            }))
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
                            .child(round_btn("pl-full", icon("fullscreen", 26.), 50.).on_click(|_, window, _| window.toggle_fullscreen())),
                    ),
            )
    }
}
