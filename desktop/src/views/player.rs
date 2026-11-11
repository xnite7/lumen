//! The player: mpv draws the picture underneath (its own child window);
//! this view is the see-through layer on top with couch's controls.
use crate::api::sized;
use crate::playback::{self, Playback};
use crate::theme;
use crate::ui::{icon, icon_fill};
use gpui::{
    div, img, linear_color_stop, linear_gradient, prelude::*, px, Context, Entity, FontWeight, MouseMoveEvent, ObjectFit, Pixels, Window,
};
use serde_json::json;
use std::time::{Duration, Instant};

pub struct PlayerView {
    pb: Entity<Playback>,
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
                let show = paused || v.last_move.elapsed() < Duration::from_millis(2800) || v.pb.read(cx).loading;
                if show != v.shown {
                    v.shown = show;
                    cx.notify();
                }
            }) else {
                break;
            };
        })
        .detach();
        Self { pb, last_move: Instant::now(), shown: true, hover_x: None }
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
            .on_click(cx.listener(|v, e: &gpui::ClickEvent, _, cx| {
                // a click on the picture: play / pause (double click: full screen)
                if e.click_count() == 1 {
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
                            .child(round_btn("pl-full", icon("fullscreen", 26.), 50.).on_click(|_, window, _| window.toggle_fullscreen())),
                    ),
            )
    }
}
