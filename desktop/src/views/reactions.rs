//! Teleparty-style reactions while watching together: tap an emoji and it
//! floats up the side of everyone's screen with your name on it.
use crate::{api, theme, ui::emoji};
use gpui::{div, prelude::*, px, Context, FontWeight, Window};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::{Duration, Instant};

pub const EMOJI: [&str; 10] = ["😂", "😭", "😱", "🔥", "❤️", "👀", "💀", "🍿", "👏", "🤯"];
const LIFE: f32 = 3.6;

struct Fly {
    emoji: String,
    name: String,
    x: f32,
    born: Instant,
    mine: bool,
}

pub struct Reactions {
    pub open: bool,
    flying: Vec<Fly>,
    since: u64,
    seen: HashSet<u64>,
    last_busy: Instant,
    opened_at: Instant,
}

/// the room we're in: the host we follow, or us when hosting
pub fn room(cx: &gpui::App) -> Option<String> {
    let t = crate::together::get(cx);
    let t = t.read(cx);
    t.following.as_ref().map(|f| f.0.clone()).or(t.hosting.clone())
}

impl Reactions {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            let mut last_room: Option<String> = None;
            loop {
                let Ok((r, me, busy)) = this.update(cx, |v, cx| {
                    let me = crate::parties::get(cx).read(cx).me.clone();
                    (room(cx), me, v.last_busy.elapsed() < Duration::from_secs(15))
                }) else {
                    break;
                };
                if r != last_room {
                    last_room = r.clone();
                    let _ = this.update(cx, |v, _| {
                        v.since = 0;
                        v.seen.clear();
                    });
                }
                if let Some(r) = r {
                    let since = this.update(cx, |v, _| v.since).unwrap_or(0);
                    let res = api::call::<Value>("crew_social", json!({ "method": "GET", "path": format!("/v1/react/{r}?since={since}"), "body": Value::Null })).await;
                    if let Ok(res) = res {
                        let list = res["reactions"].as_array().cloned().unwrap_or_default();
                        let _ = this.update(cx, |v, cx| {
                            let mut fresh = vec![];
                            for x in &list {
                                let n = x["n"].as_u64().unwrap_or(0);
                                v.since = v.since.max(n);
                                if v.seen.insert(n) && x["who"].as_str() != Some(me.as_str()) {
                                    fresh.push(x.clone());
                                }
                            }
                            // a batch replays with the gaps it was sent with
                            let first = fresh.iter().filter_map(|x| x["at"].as_f64()).fold(f64::MAX, f64::min);
                            for x in fresh {
                                v.last_busy = Instant::now();
                                let delay = (x["at"].as_f64().unwrap_or(first) - first).clamp(0., 1500.) as u64;
                                let (e, n) = (x["emoji"].as_str().unwrap_or("🔥").to_string(), x["name"].as_str().unwrap_or("").to_string());
                                cx.spawn(async move |this, cx| {
                                    gpui::Timer::after(Duration::from_millis(delay)).await;
                                    let _ = this.update(cx, |v, cx| v.launch(e, n, false, cx));
                                })
                                .detach();
                            }
                        });
                    }
                }
                gpui::Timer::after(Duration::from_millis(if busy { 350 } else { 900 })).await;
            }
        })
        .detach();
        Self { open: false, flying: vec![], since: 0, seen: HashSet::new(), last_busy: Instant::now() - Duration::from_secs(60), opened_at: Instant::now() }
    }

    fn launch(&mut self, emoji: String, name: String, mine: bool, cx: &mut Context<Self>) {
        let x = rand::random::<f32>() * 60.;
        self.flying.push(Fly { emoji, name, x, born: Instant::now(), mine });
        if self.flying.len() > 24 {
            self.flying.remove(0);
        }
        cx.notify();
    }

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        self.opened_at = Instant::now();
        cx.notify();
    }

    fn send(&mut self, e: &'static str, cx: &mut Context<Self>) {
        let Some(r) = room(cx) else { return };
        self.launch(e.to_string(), "you".into(), true, cx);
        self.opened_at = Instant::now();
        self.last_busy = Instant::now();
        cx.spawn(async move |_, _| {
            let _ = api::call::<Value>("crew_social", json!({ "method": "POST", "path": format!("/v1/react/{r}"), "body": { "emoji": e } })).await;
        })
        .detach();
    }
}

impl Render for Reactions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.flying.retain(|f| f.born.elapsed().as_secs_f32() < LIFE);
        // the tray folds itself away after a few quiet seconds
        if self.open && self.opened_at.elapsed() > Duration::from_secs(5) {
            self.open = false;
        }
        if !self.flying.is_empty() {
            window.request_animation_frame();
        } else if self.open {
            cx.spawn(async move |this, cx| {
                gpui::Timer::after(Duration::from_millis(500)).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        let h = f32::from(window.viewport_size().height);
        let flies = self.flying.iter().map(|f| {
            let t = f.born.elapsed().as_secs_f32() / LIFE;
            // rises with a little ease, sways, fades at the end
            let rise = 1. - (1. - t).powf(1.6);
            let y = 150. + rise * (h * 0.62);
            let sway = (t * 9.).sin() * 10.;
            let op = if t < 0.08 { t / 0.08 } else if t > 0.7 { ((1. - t) / 0.3).max(0.) } else { 1. };
            let scale = if t < 0.12 { 0.6 + t / 0.12 * 0.5 } else { 1.1 - (t - 0.12).min(0.2) * 0.5 };
            div()
                .absolute()
                .right(px(40. + f.x + sway))
                .bottom(px(y))
                .opacity(op)
                .flex()
                .flex_col()
                .items_center()
                .gap(px(2.))
                .child(emoji::emoji(&f.emoji, 46. * scale))
                .child(
                    div()
                        .px(px(8.))
                        .py(px(2.))
                        .rounded_full()
                        .bg(if f.mine { gpui::Hsla::from(theme::neon()) } else { theme::black(0.6).into() })
                        .text_size(px(11.))
                        .font_weight(FontWeight::BOLD)
                        .child(f.name.clone()),
                )
        });
        let tray = self.open.then(|| {
            div()
                .absolute()
                .right(px(theme::GUTTER))
                .bottom(px(110.))
                .flex()
                .gap(px(4.))
                .p(px(8.))
                .rounded_full()
                .bg(theme::black(0.72))
                .border_1()
                .border_color(theme::white(0.12))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .children(EMOJI.iter().enumerate().map(|(i, e)| {
                    div()
                        .id(("react", i))
                        .size(px(48.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::white(0.14)))
                        .on_click(cx.listener(move |v, _, _, cx| {
                            cx.stop_propagation();
                            v.send(e, cx)
                        }))
                        .child(emoji::emoji(e, 34.))
                }))
        });
        div().absolute().top_0().left_0().size_full().children(flies).children(tray)
    }
}
