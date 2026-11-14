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
    /// titles offered while you type (before the full search lands)
    suggest: Vec<Value>,
    suggest_for: String,
    /// "did you mean …" when a search finds nothing
    meant: Option<String>,
    last_q: String,
}

impl Search {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new("shows, movies, people…", cx));
        cx.subscribe(&input, |s, _, _: &Changed, cx| s.changed(cx)).detach();
        cx.subscribe(&input, |s, _, _: &Submitted, cx| s.run(cx)).detach();
        let me = Self { input, results: vec![], recents: vec![], searching: false, pending: None, req: 0, scroll: ScrollHandle::new(), suggest: vec![], suggest_for: String::new(), meant: None, last_q: String::new() };
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
        let q = self.input.read(cx).content.trim().to_string();
        if q == self.last_q {
            // set from a suggestion or a recent: that search is already running
            return;
        }
        self.meant = None;
        if q.len() < 2 {
            self.suggest.clear();
        }
        // wait for a pause in typing; suggestions come first (they're quick)
        self.pending = Some(cx.spawn(async move |this, cx| {
            crate::timer::after(Duration::from_millis(110)).await;
            if q.len() >= 2 {
                let list = api::call::<Vec<Value>>("search_suggest", json!({ "query": q.clone() })).await.unwrap_or_default();
                let _ = this.update(cx, |s, cx| {
                    s.suggest = list;
                    s.suggest_for = q.clone();
                    cx.notify();
                });
            }
            crate::timer::after(Duration::from_millis(260)).await;
            let _ = this.update(cx, |s, cx| s.run(cx));
        }));
    }

    fn clear_recents(&mut self, cx: &mut Context<Self>) {
        self.recents.clear();
        cx.notify();
        cx.spawn(async move |_, _| {
            let _ = api::call::<Value>("recents_clear", json!({})).await;
        })
        .detach();
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let q = self.input.read(cx).content.trim().to_string();
        self.last_q = q.clone();
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
                            if s.results.is_empty() {
                                s.ask_meant(cx);
                            }
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

impl Search {
    /// nothing found: maybe it was a typo
    fn ask_meant(&mut self, cx: &mut Context<Self>) {
        let q = self.input.read(cx).content.trim().to_string();
        let my = self.req;
        cx.spawn(async move |this, cx| {
            let m = api::call::<Option<String>>("did_you_mean", json!({ "query": q.clone() })).await.ok().flatten();
            let _ = this.update(cx, |s, cx| {
                if s.req == my {
                    s.meant = m.filter(|m| !m.trim().is_empty() && m.to_lowercase() != q.to_lowercase());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn pick(&mut self, text: String, cx: &mut Context<Self>) {
        self.suggest.clear();
        self.input.update(cx, |i, cx| i.set_text(&text, cx));
        self.pending = None;
        self.run(cx);
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
                .border_1()
                .border_color(theme::white(0.07))
                .text_size(px(24.))
                .font_weight(FontWeight::MEDIUM)
                .child(icon("search", 26.).text_color(theme::label2()))
                .child(self.input.clone()),
        );
        if q.trim().len() < 2 {
            if !self.recents.is_empty() {
                page = page
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(14.))
                            .child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child("recent"))
                            .child(
                                div()
                                    .id("rec-clear")
                                    .px(px(12.))
                                    .h(px(28.))
                                    .flex()
                                    .items_center()
                                    .rounded_full()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::label3())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme::white(0.1)).text_color(theme::text()))
                                    .on_click(cx.listener(|s, _, _, cx| s.clear_recents(cx)))
                                    .child("clear"),
                            ),
                    )
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
        // while the search is still out: what you probably mean, one click away
        let typed = q.trim().to_lowercase();
        if !self.suggest.is_empty() && self.suggest_for.to_lowercase() == typed && (self.searching || self.results.is_empty()) {
            page = page.child(div().flex_none().mt(px(-12.)).flex().flex_wrap().gap(px(8.)).children(self.suggest.iter().take(8).enumerate().map(|(i, sg)| {
                let name = sg["name"].as_str().unwrap_or("").to_string();
                let year = sg["year"].as_i64().map(|y| y.to_string()).unwrap_or_default();
                let n2 = name.clone();
                div()
                    .id(("sg", i))
                    .px(px(14.))
                    .h(px(38.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded_full()
                    .bg(theme::white(0.07))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::white(0.16)))
                    .text_size(px(15.))
                    .font_weight(FontWeight::MEDIUM)
                    .on_click(cx.listener(move |s, _, _, cx| s.pick(n2.clone(), cx)))
                    .child(icon("search", 14.).text_color(theme::label3()))
                    .child(name)
                    .children((!year.is_empty()).then(|| div().text_size(px(13.)).text_color(theme::label3()).child(year)))
            })));
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
            if let (false, Some(m)) = (self.searching, self.meant.clone()) {
                let m2 = m.clone();
                page = page.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(18.))
                        .child(div().text_color(theme::label2()).child("did you mean"))
                        .child(
                            div()
                                .id("meant")
                                .px(px(14.))
                                .h(px(36.))
                                .flex()
                                .items_center()
                                .rounded_full()
                                .bg(theme::white(0.1))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::neon())
                                .cursor_pointer()
                                .hover(|s| s.bg(theme::white(0.18)))
                                .on_click(cx.listener(move |s, _, _, cx| s.pick(m2.clone(), cx)))
                                .child(format!("{m}?")),
                        ),
                );
            }
        }
        page.child(div().h(px(60.)))
    }
}
