//! Teleparty-style reactions while watching together: tap an emoji and it
//! floats up the side of everyone's screen with your name on it.
use crate::{api, theme, ui::emoji};
use crate::ui::input::{Submitted, TextInput};
use gpui::{div, prelude::*, px, Context, Focusable, FontWeight, Window};
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
    /// who's on this couch right now (from the room)
    pub people: Vec<Value>,
    /// your own row of emojis (Settings keeps it; + adds, right-click removes)
    mine: Vec<String>,
    adding: Option<gpui::Entity<TextInput>>,
    flying: Vec<Fly>,
    since: u64,
    seen: HashSet<u64>,
    last_busy: Instant,
    opened_at: Instant,
    /// couch chat: lines on screen (name, text, mine, when)
    chat: Vec<(String, String, bool, Instant)>,
    /// the chat box, while typing
    typing: Option<gpui::Entity<TextInput>>,
}

/// how long a chat line stays on screen
const CHAT_LIFE: f32 = 9.;

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
                    let _ = this.update(cx, |v, cx| {
                        v.since = 0;
                        v.seen.clear();
                        v.people.clear();
                        cx.notify();
                    });
                }
                if let Some(r) = r {
                    let since = this.update(cx, |v, _| v.since).unwrap_or(0);
                    let res = api::call::<Value>("crew_social", json!({ "method": "GET", "path": format!("/v1/react/{r}?since={since}"), "body": Value::Null })).await;
                    if let Ok(res) = res {
                        let list = res["reactions"].as_array().cloned().unwrap_or_default();
                        let people = res["people"].as_array().cloned().unwrap_or_default();
                        let _ = this.update(cx, |v, cx| {
                            if v.people != people {
                                v.people = people;
                                cx.notify();
                            }
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
                                let text = x["text"].as_str().map(String::from);
                                cx.spawn(async move |this, cx| {
                                    crate::timer::after(Duration::from_millis(delay)).await;
                                    let _ = this.update(cx, |v, cx| match text {
                                        Some(t) => v.say(n, t, false, cx),
                                        None => v.launch(e, n, false, cx),
                                    });
                                })
                                .detach();
                            }
                        });
                    }
                }
                crate::timer::after(Duration::from_millis(if busy { 350 } else { 900 })).await;
            }
        })
        .detach();
        let mine = couch::state::get().core.db.get::<Vec<String>>("gpui_reactions").filter(|l| !l.is_empty()).unwrap_or_else(|| EMOJI.iter().map(|e| e.to_string()).collect());
        Self { open: false, people: vec![], mine, adding: None, flying: vec![], since: 0, seen: HashSet::new(), last_busy: Instant::now() - Duration::from_secs(60), opened_at: Instant::now(), chat: vec![], typing: None }
    }

    fn launch(&mut self, emoji: String, name: String, mine: bool, cx: &mut Context<Self>) {
        // anywhere along the bottom (not just one side)
        let x = 0.08 + rand::random::<f32>() * 0.84;
        self.flying.push(Fly { emoji, name, x, born: Instant::now(), mine });
        if self.flying.len() > 24 {
            self.flying.remove(0);
        }
        cx.notify();
    }

    fn say(&mut self, name: String, text: String, mine: bool, cx: &mut Context<Self>) {
        self.chat.push((name, text, mine, Instant::now()));
        if self.chat.len() > 6 {
            self.chat.remove(0);
        }
        cx.notify();
    }

    /// open the chat box (Enter sends, Esc / empty closes)
    pub fn start_typing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| TextInput::new("say something to the couch…", cx));
        cx.subscribe(&input, |v, i, _: &Submitted, cx| {
            let t = i.read(cx).content.trim().to_string();
            v.typing = None;
            if !t.is_empty() {
                if let Some(r) = room(cx) {
                    v.say("you".into(), t.clone(), true, cx);
                    v.last_busy = Instant::now();
                    cx.spawn(async move |_, _| {
                        let _ = api::call::<Value>("crew_social", json!({ "method": "POST", "path": format!("/v1/react/{r}"), "body": { "text": t } })).await;
                    })
                    .detach();
                }
            }
            cx.notify();
        })
        .detach();
        window.focus(&input.focus_handle(cx), cx);
        self.typing = Some(input);
        self.opened_at = Instant::now();
        cx.notify();
    }

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        self.opened_at = Instant::now();
        cx.notify();
    }

    fn save(&self) {
        let _ = couch::state::get().core.db.set("gpui_reactions", &self.mine);
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        let input = cx.new(|cx| TextInput::new("paste an emoji (win + .)", cx));
        cx.subscribe(&input, |v, i, _: &Submitted, cx| {
            let t = i.read(cx).content.trim().to_string();
            // keep the emoji characters only
            let e: String = t.chars().filter(|c| !c.is_alphanumeric() && !c.is_whitespace() && !c.is_ascii_punctuation()).collect();
            if !e.is_empty() && e.chars().count() <= 8 && !v.mine.contains(&e) {
                v.mine.push(e);
                if v.mine.len() > 16 {
                    v.mine.remove(0);
                }
                v.save();
            }
            v.adding = None;
            v.opened_at = Instant::now();
            cx.notify();
        })
        .detach();
        self.adding = Some(input);
        self.opened_at = Instant::now();
        cx.notify();
    }

    fn send(&mut self, e: String, cx: &mut Context<Self>) {
        let Some(r) = room(cx) else { return };
        self.launch(e.clone(), "you".into(), true, cx);
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
        self.chat.retain(|c| c.3.elapsed().as_secs_f32() < CHAT_LIFE);
        if !self.chat.is_empty() {
            window.request_animation_frame();
        }
        if self.open && self.adding.is_none() && self.typing.is_none() && self.opened_at.elapsed() > Duration::from_secs(5) {
            self.open = false;
        }
        if !self.flying.is_empty() {
            window.request_animation_frame();
        } else if self.open {
            cx.spawn(async move |this, cx| {
                crate::timer::after(Duration::from_millis(500)).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        let h = f32::from(window.viewport_size().height);
        let vw = f32::from(window.viewport_size().width);
        let flies = self.flying.iter().map(|f| {
            let t = f.born.elapsed().as_secs_f32() / LIFE;
            // rises with a little ease, sways, fades at the end
            let rise = 1. - (1. - t).powf(1.6);
            // from the bottom of the screen up to the top
            let y = 60. + rise * (h - 140.);
            let sway = (t * 9. + f.x * 20.).sin() * 14.;
            let op = if t < 0.08 { t / 0.08 } else if t > 0.7 { ((1. - t) / 0.3).max(0.) } else { 1. };
            let scale = if t < 0.12 { 0.6 + t / 0.12 * 0.5 } else { 1.1 - (t - 0.12).min(0.2) * 0.5 };
            div()
                .absolute()
                .left(px(f.x * vw + sway - 23.))
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
                        .font_weight(FontWeight::SEMIBOLD)
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
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .children(self.mine.clone().into_iter().enumerate().map(|(i, e)| {
                    let e2 = e.clone();
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
                            v.send(e.clone(), cx)
                        }))
                        // right-click: take it off your row
                        .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |v, _, _, cx| {
                            cx.stop_propagation();
                            if v.mine.len() > 1 {
                                v.mine.retain(|x| x != &e2);
                                v.save();
                                v.opened_at = Instant::now();
                                cx.notify();
                            }
                        }))
                        .child(emoji::emoji(&e2_view(&i, &self.mine), 34.))
                }))
                // couch chat
                .child(match self.typing.clone() {
                    Some(inp) => div().w(px(280.)).h(px(40.)).px(px(14.)).flex().items_center().rounded_full().bg(theme::white(0.1)).text_size(px(15.)).child(inp).into_any_element(),
                    None => div()
                        .id("react-chat")
                        .size(px(48.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .cursor_pointer()
                        .bg(theme::white(0.08))
                        .hover(|s| s.bg(theme::white(0.16)))
                        .on_click(cx.listener(|v, _, window, cx| {
                            cx.stop_propagation();
                            v.start_typing(window, cx);
                        }))
                        .child(emoji::emoji("💬", 26.))
                        .into_any_element(),
                })
                .child(match self.adding.clone() {
                    Some(inp) => div().w(px(200.)).h(px(40.)).px(px(12.)).flex().items_center().rounded_full().bg(theme::white(0.1)).text_size(px(15.)).child(inp).into_any_element(),
                    None => div()
                        .id("react-add")
                        .size(px(48.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .cursor_pointer()
                        .bg(theme::white(0.08))
                        .hover(|s| s.bg(theme::white(0.16)))
                        .on_click(cx.listener(|v, _, window, cx| {
                            cx.stop_propagation();
                            v.add(cx);
                            if let Some(i) = &v.adding {
                                window.focus(&i.focus_handle(cx), cx);
                            }
                        }))
                        .child(crate::ui::icon("plus", 20.))
                        .into_any_element(),
                })
        });
        // the chat: newest at the bottom, each line fading out after a while
        let chat = (!self.chat.is_empty()).then(|| {
            div()
                .absolute()
                .left(px(theme::GUTTER))
                .bottom(px(120.))
                .max_w(px(460.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .children(self.chat.iter().map(|(name, text, mine, at)| {
                    let t = at.elapsed().as_secs_f32();
                    let a = if t < 0.2 { t / 0.2 } else if t > CHAT_LIFE - 1. { (CHAT_LIFE - t).max(0.) } else { 1. };
                    div()
                        .opacity(a)
                        .flex()
                        .items_baseline()
                        .gap(px(8.))
                        .px(px(14.))
                        .py(px(8.))
                        .rounded(px(16.))
                        .bg(gpui::rgba(0x0c090be0))
                        .border_1()
                        .border_color(theme::white(0.08))
                        .child(div().flex_none().text_size(px(13.)).font_weight(FontWeight::BLACK).text_color(if *mine { gpui::Hsla::from(theme::neon()) } else { gpui::Hsla::from(theme::label2()) }).child(name.clone()))
                        .child(div().text_size(px(15.)).line_height(px(20.)).child(text.clone()))
                }))
        });
        div().absolute().top_0().left_0().size_full().children(flies).children(chat).children(tray)
    }
}

fn e2_view(i: &usize, mine: &[String]) -> String {
    mine.get(*i).cloned().unwrap_or_default()
}
