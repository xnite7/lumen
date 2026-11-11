//! A person: photo, the facts, the bio, and what they've been in.
use crate::api::{self, Meta};
use crate::theme;
use crate::ui::card;
use gpui::{div, img, prelude::*, px, Context, FontWeight, ObjectFit, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

pub struct Person {
    pub key: String,
    name: String,
    data: Option<Value>,
    scroll: ScrollHandle,
}

impl Person {
    pub fn new(name: String, tmdb_id: Option<u64>, cx: &mut Context<Self>) -> Self {
        let n = name.clone();
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("person_get", json!({ "name": n, "tmdbId": tmdb_id, "adult": false })).await.unwrap_or_default();
            let _ = this.update(cx, |p, cx| {
                p.data = Some(r);
                cx.notify();
            });
        })
        .detach();
        Self { key: format!("{name}{tmdb_id:?}"), name, data: None, scroll: ScrollHandle::new() }
    }
}

impl Render for Person {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let vw = f32::from(window.viewport_size().width);
        let cols = (((vw - g * 2.) + 14.) / (176. + 14.)).floor().max(2.);
        let w = ((vw - g * 2.) - (cols - 1.) * 14.) / cols;
        let d = self.data.clone().unwrap_or_default();
        let facts: Vec<String> = [d["knownFor"].as_str().map(|s| s.to_string()), d["birthday"].as_str().map(|b| format!("born {b}")), d["birthplace"].as_str().map(|s| s.to_string())]
            .into_iter()
            .flatten()
            .collect();
        let credits: Vec<Meta> = d["credits"].as_array().into_iter().flatten().filter_map(|c| serde_json::from_value(c.clone()).ok()).filter(|m: &Meta| m.poster.is_some()).collect();
        div()
            .id("person")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .bg(theme::bg())
            .pt(px(100.))
            .px(px(g))
            .flex()
            .flex_col()
            .gap(px(26.))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap(px(30.))
                    .child(
                        div()
                            .w(px(220.))
                            .h(px(330.))
                            .flex_none()
                            .rounded(px(18.))
                            .overflow_hidden()
                            .bg(theme::surface2())
                            .children(d["profile"].as_str().map(|u| img(u.to_string()).size_full().object_fit(ObjectFit::Cover))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .max_w(px(760.))
                            .child(div().text_size(px(48.)).font_weight(FontWeight::BLACK).child(d["name"].as_str().unwrap_or(&self.name).to_string()))
                            .child(div().text_size(px(15.)).text_color(theme::label2()).child(facts.join(" · ")))
                            .children(d["bio"].as_str().filter(|b| !b.is_empty()).map(|b| {
                                div().text_size(px(16.)).line_height(px(24.)).line_clamp(9).text_ellipsis().text_color(theme::white(0.85)).child(b.to_string())
                            })),
                    ),
            )
            .when(!credits.is_empty(), |p| {
                p.child(div().flex_none().text_size(px(24.)).font_weight(FontWeight::BLACK).child("known for")).child(div().flex_none().flex().flex_wrap().gap(px(14.)).children(
                    credits.into_iter().enumerate().map(|(i, m)| {
                        let mm = m.clone();
                        card::poster(SharedString::from(format!("pc-{i}")), &m, w, move |_, _, cx| crate::views::detail::open_rec(&mm, cx))
                    }),
                ))
            })
            .child(div().h(px(80.)))
    }
}
