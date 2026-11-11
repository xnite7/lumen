//! Someone's page on the couch: what they're on right now, what they're in
//! the middle of, what they finished (with their score), their favourites,
//! their reviews and what's next on their list. Tinted with their colour.
use crate::api::{self, sized, Meta};
use crate::theme;
use crate::ui::card;
use crate::Nav;
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, Context, FontWeight, Hsla, ObjectFit, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

pub struct Member {
    pub id: String,
    data: Option<Value>,
    scroll: ScrollHandle,
}

fn metas(v: &Value) -> Vec<(Meta, Value)> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| serde_json::from_value::<Meta>(x.clone()).ok().map(|m| (m, x.clone())))
        .filter(|(m, _)| !m.id.is_empty())
        .collect()
}

fn hex(s: &str) -> Option<Hsla> {
    let s = s.trim_start_matches('#');
    u32::from_str_radix(s, 16).ok().filter(|_| s.len() == 6).map(|v| gpui::rgb(v).into())
}

impl Member {
    pub fn new(id: String, cx: &mut Context<Self>) -> Self {
        let i = id.clone();
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("crew_social", json!({ "method": "GET", "path": format!("/v1/profile/{i}"), "body": Value::Null })).await.ok();
            let _ = this.update(cx, |m, cx| {
                m.data = r;
                cx.notify();
            });
        })
        .detach();
        Self { id, data: None, scroll: ScrollHandle::new() }
    }
}

impl Render for Member {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let vw = f32::from(window.viewport_size().width);
        let cols = (((vw - g * 2.) + 14.) / (160. + 14.)).floor().max(2.);
        let w = ((vw - g * 2.) - (cols - 1.) * 14.) / cols;
        let d = self.data.clone().unwrap_or_default();
        let p = &d["profile"];
        let accent = d["style"]["accent"].as_str().and_then(hex).unwrap_or(theme::neon().into());
        let mut glow = accent;
        glow.a = 0.28;
        let name = d["name"].as_str().unwrap_or("someone").to_string();
        let stats: Vec<(i64, &str)> = [("movies", "films"), ("shows", "shows"), ("episodes", "episodes"), ("plan", "on the list")]
            .into_iter()
            .filter_map(|(k, l)| p["stats"][k].as_i64().filter(|n| *n > 0 || k != "episodes").map(|n| (n, l)))
            .collect();

        let row = |title: &str, list: Vec<(Meta, Value)>, key: &str, landscape: bool| {
            let key = key.to_string();
            div()
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(12.))
                .mt(px(14.))
                .child(div().text_size(px(22.)).font_weight(FontWeight::BLACK).child(title.to_string()))
                .child(div().id(SharedString::from(format!("m-{key}"))).overflow_x_scroll().flex().gap(px(12.)).children(list.into_iter().enumerate().map(move |(i, (m, raw))| {
                    let mm = m.clone();
                    let el = if landscape {
                        card::card(SharedString::from(format!("m-{key}-{i}")), &m, move |_, _, cx| Nav::detail(&mm, cx))
                    } else {
                        card::poster(SharedString::from(format!("m-{key}-{i}")), &m, w, move |_, _, cx| Nav::detail(&mm, cx))
                    };
                    let rating = raw["rating"].as_i64();
                    let eps = raw["eps"].as_array().and_then(|e| Some((e.first()?.as_i64()?, e.get(1)?.as_i64()?)));
                    div()
                        .flex_none()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(el)
                        .children(rating.map(|r| div().text_size(px(14.)).font_weight(FontWeight::BLACK).text_color(theme::lamp()).child(format!("★ {r}/10"))))
                        .children(eps.filter(|e| e.1 > 0).map(|(a, b)| div().text_size(px(13.)).text_color(theme::label2()).child(format!("{a} of {b} episodes"))))
                })))
        };

        let mut page = div()
            .id("member")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .bg(theme::bg())
            .relative()
            // their colour, glowing in from the top
            .child(div().absolute().top_0().left_0().w_full().h(px(420.)).bg(linear_gradient(180., linear_color_stop(glow, 0.), linear_color_stop(theme::black(0.0), 1.))))
            .child(
                div()
                    .relative()
                    .pt(px(110.))
                    .px(px(g))
                    .flex()
                    .items_center()
                    .gap(px(24.))
                    .child(
                        div()
                            .size(px(128.))
                            .flex_none()
                            .rounded_full()
                            .overflow_hidden()
                            .border_4()
                            .border_color(accent)
                            .bg(theme::surface2())
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(48.))
                            .font_weight(FontWeight::BLACK)
                            .child(match d["avatar"].as_str() {
                                Some(u) => img(u.to_string()).size_full().object_fit(ObjectFit::Cover).into_any_element(),
                                None => div().child(name.chars().next().unwrap_or('?').to_uppercase().to_string()).into_any_element(),
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(div().text_size(px(48.)).font_weight(FontWeight::BLACK).child(name.clone()))
                            .child(div().flex().gap(px(22.)).children(stats.into_iter().map(|(n, l)| {
                                div().flex().items_baseline().gap(px(6.)).child(div().text_size(px(22.)).font_weight(FontWeight::BLACK).child(n.to_string())).child(div().text_size(px(14.)).text_color(theme::label2()).child(l))
                            }))),
                    ),
            );
        let mut body = div().relative().px(px(g)).pt(px(14.)).flex().flex_col().gap(px(14.));
        if self.data.is_none() {
            body = body.child(div().text_color(theme::label2()).child("loading…"));
        } else if p.is_null() {
            body = body.child(div().text_color(theme::label2()).child(format!("{name} hasn't shared their page yet.")));
        }
        let watching = metas(&p["watching"]);
        if !watching.is_empty() {
            body = body.child(row("in the middle of", watching, "watching", true));
        }
        let recent = metas(&p["recent"]);
        if !recent.is_empty() {
            body = body.child(row("finished", recent, "recent", false));
        }
        let favs = metas(&d["favs"]);
        if !favs.is_empty() {
            body = body.child(row("favourites", favs, "favs", false));
        }
        // reviews
        let reviews: Vec<&Value> = d["activity"].as_array().into_iter().flatten().filter(|a| a["text"].as_str().map_or(false, |t| !t.is_empty())).take(8).collect();
        if !reviews.is_empty() {
            body = body.child(div().mt(px(14.)).text_size(px(22.)).font_weight(FontWeight::BLACK).child("reviews")).child(div().flex().flex_col().gap(px(12.)).max_w(px(820.)).children(reviews.into_iter().map(|a| {
                let t = &a["title"];
                div()
                    .flex()
                    .gap(px(14.))
                    .p(px(14.))
                    .rounded(px(16.))
                    .bg(theme::white(0.05))
                    .child(div().w(px(70.)).h(px(105.)).flex_none().rounded(px(8.)).overflow_hidden().bg(theme::surface2()).children(t["poster"].as_str().map(|u| img(sized(u, "w342")).size_full().object_fit(ObjectFit::Cover))))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(div().flex().items_center().gap(px(10.)).child(div().text_size(px(16.)).font_weight(FontWeight::BOLD).child(t["name"].as_str().unwrap_or("").to_string())).children(a["rating"].as_i64().map(|r| div().text_size(px(14.)).font_weight(FontWeight::BLACK).text_color(theme::lamp()).child(format!("★ {r}/10")))))
                            .child(div().text_size(px(15.)).line_height(px(21.)).text_color(theme::white(0.85)).child(a["text"].as_str().unwrap_or("").to_string())),
                    )
            })));
        }
        let plan = metas(&p["plan"]);
        if !plan.is_empty() {
            body = body.child(row("up next on their list", plan, "plan", false));
        }
        page = page.child(body).child(div().h(px(80.)));
        page
    }
}
