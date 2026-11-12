//! The smart bar: ask for a feeling, not a title ("something cozy for a
//! rainy night"). Picks come with a line on why, from your own taste.
use crate::api::{self, sized, Meta};
use crate::theme;
use crate::ui::{icon, input::{Submitted, TextInput}};
use crate::Nav;
use gpui::{div, img, prelude::*, px, Context, Entity, FontWeight, ObjectFit, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

pub struct Vibe {
    pub input: Entity<TextInput>,
    chips: Vec<(String, String)>,
    busy: bool,
    asked: String,
    read: String,
    picks: Vec<(Meta, String)>,
    error: String,
    /// what you asked before, newest first (kept in couch's database)
    history: Vec<Value>,
    ready: bool,
    scroll: ScrollHandle,
}

const KEY: &str = "gpui_vibe_history";

impl Vibe {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new("a heist movie that's actually smart…", cx));
        cx.subscribe(&input, |v, i, _: &Submitted, cx| {
            let q = i.read(cx).content.trim().to_string();
            v.ask(q, cx);
        })
        .detach();
        let history = couch::state::get().core.db.get::<Vec<Value>>(KEY).unwrap_or_default();
        cx.spawn(async move |this, cx| {
            let ready = api::call::<Value>("secrets_status", json!({})).await.ok().and_then(|s| s["smartBar"].as_bool()).unwrap_or(false);
            let chips = if ready { api::call::<Value>("ai_prompts", json!({})).await.unwrap_or_default() } else { Value::Null };
            let _ = this.update(cx, |v, cx| {
                v.ready = ready;
                v.chips = chips["prompts"].as_array().into_iter().flatten().map(|c| (c["label"].as_str().unwrap_or("").to_string(), c["q"].as_str().unwrap_or("").to_string())).collect();
                cx.notify();
            });
        })
        .detach();
        Self { input, chips: vec![], busy: false, asked: String::new(), read: String::new(), picks: vec![], error: String::new(), history, ready: false, scroll: ScrollHandle::new() }
    }

    fn show(&mut self, q: &str, r: &Value) {
        self.asked = q.to_string();
        self.read = r["read"].as_str().unwrap_or("").to_string();
        self.picks = r["picks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| serde_json::from_value::<Meta>(p.clone()).ok().filter(|m| !m.id.is_empty()).map(|m| (m, p["why"].as_str().unwrap_or("").to_string())))
            .collect();
    }

    pub fn ask(&mut self, q: String, cx: &mut Context<Self>) {
        if q.is_empty() || self.busy {
            return;
        }
        self.input.update(cx, |i, cx| i.set_text(&q, cx));
        self.busy = true;
        self.error.clear();
        self.asked = q.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("ai_vibe", json!({ "query": q })).await;
            let _ = this.update(cx, |v, cx| {
                v.busy = false;
                match r {
                    Ok(r) => {
                        v.show(&q, &r);
                        v.history.retain(|h| h["q"].as_str().map_or(true, |x| !x.eq_ignore_ascii_case(&q)));
                        v.history.insert(0, json!({ "q": q, "result": r }));
                        v.history.truncate(30);
                        let _ = couch::state::get().core.db.set(KEY, &v.history);
                    }
                    Err(e) => v.error = e,
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for Vibe {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let mut page = div()
            .id("vibe")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "vibe"))
            .bg(theme::bg())
            .pt(px(100.))
            .px(px(g))
            .pb(px(80.))
            .flex()
            .flex_col()
            .gap(px(18.))
            .child(div().flex().flex_col().child(div().text_size(px(40.)).font_weight(FontWeight::BLACK).child("smart bar")).child(div().text_size(px(15.)).text_color(theme::label2()).child("ask for a feeling, not a title.")))
            .child(
                div()
                    .max_w(px(820.))
                    .h(px(60.))
                    .px(px(20.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .rounded_full()
                    .bg(if self.busy { theme::white(0.14) } else { theme::white(0.08) })
                    .child(icon("sparkles", 22.))
                    .child(div().flex_1().text_size(px(18.)).child(self.input.clone())),
            );
        if !self.ready {
            return page.child(div().text_color(theme::label2()).child("the smart bar isn't set up on this computer yet (it needs an AI key)."));
        }
        if self.asked.is_empty() && !self.chips.is_empty() {
            page = page.child(div().flex().flex_wrap().gap(px(8.)).max_w(px(900.)).children(self.chips.iter().enumerate().map(|(i, (l, q))| {
                let q = q.clone();
                div()
                    .id(("vchip", i))
                    .px(px(16.))
                    .h(px(38.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .bg(theme::white(0.08))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::white(0.14)))
                    .on_click(cx.listener(move |v, _, _, cx| v.ask(q.clone(), cx)))
                    .child(l.clone())
            })));
        }
        if self.busy {
            page = page.child(div().text_color(theme::label2()).child("thinking…"));
        }
        if !self.error.is_empty() {
            page = page.child(div().text_color(gpui::rgb(0xff8a8a)).child(self.error.clone()));
        }
        if !self.busy && !self.read.is_empty() {
            page = page.child(div().max_w(px(820.)).text_size(px(18.)).italic().text_color(theme::label2()).child(format!("“{}”", self.read)));
        }
        if !self.busy && !self.picks.is_empty() {
            page = page.child(div().flex().flex_col().gap(px(12.)).max_w(px(900.)).children(self.picks.iter().enumerate().map(|(i, (m, why))| {
                let mm = m.clone();
                div()
                    .id(SharedString::from(format!("vp-{i}")))
                    .flex()
                    .gap(px(16.))
                    .p(px(12.))
                    .rounded(px(16.))
                    .bg(theme::white(0.04))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::white(0.08)))
                    .on_click(move |_, _, cx| Nav::detail(&mm, cx))
                    .child(div().w(px(84.)).h(px(126.)).flex_none().rounded(px(10.)).overflow_hidden().bg(theme::surface2()).children(m.poster.as_ref().map(|u| img(sized(u, "w342")).size_full().object_fit(ObjectFit::Cover))))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(div().flex().items_baseline().gap(px(10.)).child(div().text_size(px(19.)).font_weight(FontWeight::BLACK).child(m.name.clone())).children(m.year().map(|y| div().text_color(theme::label3()).child(y))))
                            .child(div().text_size(px(15.)).line_height(px(21.)).text_color(theme::white(0.82)).child(why.clone())),
                    )
            })));
        }
        // asked before: reopen without a new request
        let past: Vec<Value> = self.history.iter().filter(|h| h["q"].as_str() != Some(self.asked.as_str())).take(12).cloned().collect();
        if !past.is_empty() && !self.busy {
            page = page.child(div().mt(px(10.)).text_size(px(18.)).font_weight(FontWeight::BLACK).child("asked before")).child(div().flex().flex_wrap().gap(px(8.)).max_w(px(900.)).children(past.into_iter().enumerate().map(|(i, h)| {
                let q = h["q"].as_str().unwrap_or("").to_string();
                let r = h["result"].clone();
                let label = q.clone();
                div()
                    .id(("vpast", i))
                    .px(px(14.))
                    .h(px(34.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .text_size(px(14.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::white(0.08)))
                    .on_click(cx.listener(move |v, _, _, cx| {
                        v.show(&q, &r);
                        v.input.update(cx, |i, cx| i.set_text(&q, cx));
                        cx.notify();
                    }))
                    .child(label)
            })));
        }
        page
    }
}
