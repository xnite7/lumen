//! Search: type, and every addon's results land as they come, shows and
//! films apart. Empty: your recent searches.
use crate::api::{self, Meta};
use crate::host;
use crate::theme;
use crate::ui::{card, icon, input::{Changed, Submitted, TextInput}};
use crate::Nav;
use futures::StreamExt;
use gpui::{div, prelude::*, px, Context, Entity, FontWeight, ScrollHandle, SharedString, Task, Window};
use serde_json::{json, Value};
use std::time::Duration;

pub struct Search {
    pub input: Entity<TextInput>,
    results: Vec<Meta>,
    recents: Vec<String>,
    searching: bool,
    pending: Option<Task<()>>,
    req: u64,
    scroll: ScrollHandle,
}

impl Search {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new("shows, movies, people…", cx));
        cx.subscribe(&input, |s, _, _: &Changed, cx| s.changed(cx)).detach();
        cx.subscribe(&input, |s, _, _: &Submitted, cx| s.run(cx)).detach();
        let me = Self { input, results: vec![], recents: vec![], searching: false, pending: None, req: 0, scroll: ScrollHandle::new() };
        cx.spawn(async move |this, cx| {
            let r = api::call::<Vec<String>>("recents_get", json!({})).await.unwrap_or_default();
            let _ = this.update(cx, |s, cx| {
                s.recents = r;
                cx.notify();
            });
        })
        .detach();
        me
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        // wait for a pause in typing
        self.pending = Some(cx.spawn(async move |this, cx| {
            crate::timer::after(Duration::from_millis(320)).await;
            let _ = this.update(cx, |s, cx| s.run(cx));
        }));
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let q = self.input.read(cx).content.trim().to_string();
        self.req += 1;
        let my = self.req;
        self.results.clear();
        if q.len() < 2 {
            self.searching = false;
            cx.notify();
            return;
        }
        self.searching = true;
        cx.notify();
        let req_id = format!("s{my}-{}", std::process::id());
        let mut rx = host::listen("search");
        let rid = req_id.clone();
        cx.spawn(async move |this, cx| {
            let _ = api::call::<Value>("search_start", json!({ "query": q, "reqId": rid })).await;
            while let Some(c) = rx.next().await {
                if c["reqId"].as_str() != Some(req_id.as_str()) {
                    continue;
                }
                let done = c["finished"].as_bool() == Some(true);
                let metas: Vec<Meta> = c["metas"].as_array().into_iter().flatten().filter_map(|m| serde_json::from_value(m.clone()).ok()).collect();
                let alive = this
                    .update(cx, |s, cx| {
                        if s.req != my {
                            return false;
                        }
                        for m in metas {
                            if !m.name.is_empty() && !s.results.iter().any(|x| x.id == m.id) {
                                s.results.push(m);
                            }
                        }
                        if done {
                            s.searching = false;
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if done || !alive {
                    break;
                }
            }
        })
        .detach();
    }
}

impl Render for Search {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let vw = f32::from(window.viewport_size().width);
        let cols = (((vw - g * 2.) + 14.) / (176. + 14.)).floor().max(2.);
        let w = ((vw - g * 2.) - (cols - 1.) * 14.) / cols;
        let q = self.input.read(cx).content.to_string();
        let mut page = div().id("search").size_full().overflow_y_scroll().track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "search")).bg(theme::bg()).pt(px(100.)).px(px(g)).flex().flex_col().gap(px(26.));
        page = page.child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(14.))
                .h(px(64.))
                .px(px(20.))
                .rounded(px(18.))
                .bg(theme::white(0.08))
                .text_size(px(24.))
                .font_weight(FontWeight::MEDIUM)
                .child(icon("search", 26.).text_color(theme::label2()))
                .child(self.input.clone()),
        );
        if q.trim().len() < 2 {
            if !self.recents.is_empty() {
                page = page
                    .child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child("recent"))
                    .child(div().flex().flex_wrap().gap(px(10.)).children(self.recents.iter().enumerate().map(|(i, r)| {
                        let r2 = r.clone();
                        div()
                            .id(SharedString::from(format!("rec-{i}")))
                            .px(px(16.))
                            .h(px(38.))
                            .flex()
                            .items_center()
                            .rounded_full()
                            .bg(theme::white(0.1))
                            .text_size(px(15.))
                            .font_weight(FontWeight::MEDIUM)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::white(0.18)))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                let t = r2.clone();
                                s.input.update(cx, |i, cx| i.set_text(&t, cx));
                                s.run(cx);
                            }))
                            .child(r.clone())
                    })));
            }
            return page;
        }
        for (label, kind) in [("shows", "series"), ("movies", "movie"), ("more", "")] {
            let list: Vec<&Meta> = self.results.iter().filter(|m| if kind.is_empty() { m.kind != "series" && m.kind != "movie" } else { m.kind == kind }).collect();
            if list.is_empty() {
                continue;
            }
            page = page.child(div().text_size(px(22.)).font_weight(FontWeight::BLACK).child(label)).child(div().flex().flex_wrap().gap(px(14.)).children(
                list.into_iter().enumerate().map(|(i, m)| {
                    let mm = m.clone();
                    card::poster(SharedString::from(format!("sr-{kind}-{i}")), m, w, move |_, _, cx| Nav::detail(&mm, cx))
                }),
            ));
        }
        if self.results.is_empty() {
            page = page.child(div().text_size(px(16.)).text_color(theme::label2()).child(if self.searching { "looking…" } else { "nothing for that. try another spelling." }));
        }
        page.child(div().h(px(60.)))
    }
}
