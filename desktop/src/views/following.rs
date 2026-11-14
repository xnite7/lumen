//! Following: the people and studios you follow, and what's new from them.
use crate::api::{self, Meta};
use crate::theme;
use crate::ui::{card, icon};
use crate::Nav;
use gpui::{div, img, prelude::*, px, AnyElement, Context, FontWeight, ObjectFit, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub struct Following {
    list: Vec<Value>,
    /// (title, who it's from)
    feed: Vec<(Meta, String)>,
    loaded: bool,
    /// studio logos, made white when they're too dark for couch's black
    logos: HashMap<u64, Arc<gpui::Image>>,
    scroll: ScrollHandle,
}

impl Following {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let me = Self { list: vec![], feed: vec![], loaded: false, logos: HashMap::new(), scroll: ScrollHandle::new() };
        Self::load(cx);
        me
    }

    pub fn load(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let list = api::call::<Vec<Value>>("follows_list", json!({})).await.unwrap_or_default();
            let studios: Vec<(u64, String)> = list
                .iter()
                .filter(|f| f["kind"] == "studio")
                .filter_map(|f| Some((f["id"].as_u64()?, f["img"].as_str()?.to_string())))
                .filter(|(_, u)| !u.ends_with(".svg"))
                .collect();
            let _ = this.update(cx, |s, cx| {
                s.list = list;
                s.loaded = true;
                cx.notify();
            });
            for (id, u) in studios {
                let pic = couch::rt::spawn(async move {
                    let core = couch::state::get().core.clone();
                    couch::images::load(&core, &u).await.ok().and_then(|(b, _)| crate::ui::readable_logo(&b))
                })
                .await
                .ok()
                .flatten();
                if let Some(p) = pic {
                    let _ = this.update(cx, |s, cx| {
                        s.logos.insert(id, p);
                        cx.notify();
                    });
                }
            }
            let feed = api::call::<Vec<Value>>("follow_feed", json!({})).await.unwrap_or_default();
            let feed: Vec<(Meta, String)> = feed
                .into_iter()
                .filter_map(|v| {
                    let who = v["because"].as_str().unwrap_or("").to_string();
                    serde_json::from_value::<Meta>(v).ok().map(|m| (m, who))
                })
                .collect();
            let _ = this.update(cx, |s, cx| {
                s.feed = feed;
                cx.notify();
            });
        })
        .detach();
    }

    fn unfollow(&mut self, kind: String, id: u64, name: String, cx: &mut Context<Self>) {
        self.list.retain(|f| !(f["kind"] == kind.as_str() && f["id"].as_u64() == Some(id)));
        self.feed.retain(|(_, who)| who != &name);
        cx.notify();
        cx.spawn(async move |_, cx| {
            let _ = api::call::<Value>("follow_toggle", json!({ "kind": kind, "id": id, "name": name, "img": Value::Null })).await;
            let _ = cx.update(|cx| Nav::toast(format!("unfollowed {name}"), cx));
        })
        .detach();
    }
}

fn section(title: &str, n: usize) -> gpui::Div {
    div()
        .flex_none()
        .mt(px(14.))
        .flex()
        .items_baseline()
        .gap(px(10.))
        .child(div().text_size(px(22.)).font_weight(FontWeight::BLACK).child(title.to_string()))
        .child(div().text_size(px(15.)).text_color(theme::label3()).child(n.to_string()))
}

impl Render for Following {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let people: Vec<Value> = self.list.iter().filter(|f| f["kind"] == "person").cloned().collect();
        let studios: Vec<Value> = self.list.iter().filter(|f| f["kind"] == "studio").cloned().collect();
        let mut page = div()
            .id("following")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "following"))
            .pt(px(100.))
            .pb(px(80.))
            .flex()
            .flex_col()
            .gap(px(16.));
        let count = [
            (people.len(), "person", "people"),
            (studios.len(), "studio", "studios"),
        ]
        .into_iter()
        .filter(|c| c.0 > 0)
        .map(|(n, one, many)| format!("{n} {}", if n == 1 { one } else { many }))
        .collect::<Vec<_>>()
        .join(" · ");
        page = page.child(
            div()
                .flex_none()
                .px(px(g))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(div().text_size(px(40.)).font_weight(FontWeight::BLACK).child("following"))
                .child(div().text_size(px(16.)).text_color(theme::label2()).child(if self.list.is_empty() { "their new films and shows land on Home".to_string() } else { format!("{count} · their new films and shows land on Home") })),
        );

        // nobody yet: say how
        if self.loaded && self.list.is_empty() {
            return page.child(
                div()
                    .flex_none()
                    .mx(px(g))
                    .mt(px(30.))
                    .max_w(px(560.))
                    .p(px(28.))
                    .rounded(px(22.))
                    .bg(theme::white(0.045))
                    .border_1()
                    .border_color(theme::white(0.06))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(icon("heart", 30.))
                    .child(div().text_size(px(22.)).font_weight(FontWeight::BLACK).child("nobody yet"))
                    .child(div().text_size(px(16.)).line_height(px(23.)).text_color(theme::label2()).child("open an actor, a director or a studio from any title and press follow. when they have something new, it's waiting for you on Home."))
                    .child(
                        div()
                            .id("fol-find")
                            .mt(px(8.))
                            .px(px(18.))
                            .h(px(40.))
                            .flex()
                            .items_center()
                            .rounded_full()
                            .bg(theme::text())
                            .text_color(gpui::rgb(0x000000))
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.88))
                            .on_click(|_, _, cx| Nav::go(crate::Route::Search, cx))
                            .child("find someone"),
                    ),
            );
        }

        // what's new from them
        if !self.feed.is_empty() {
            let w = 150.;
            let items: Vec<AnyElement> = self
                .feed
                .iter()
                .take(30)
                .enumerate()
                .map(|(i, (m, who))| {
                    let mm = m.clone();
                    div()
                        .flex_none()
                        .w(px(w))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(card::poster(SharedString::from(format!("fol-new-{i}")), m, w, move |_, _, cx| Nav::detail(&mm, cx)))
                        .child(div().text_size(px(12.)).text_color(theme::label3()).truncate().child(who.clone()))
                        .into_any_element()
                })
                .collect();
            page = page
                .child(div().px(px(g)).child(section("new from them", self.feed.len().min(30))))
                .child(crate::ui::rows::scroller("row-fol-new", div().id("row-fol-new").flex_none().overflow_x_scroll().flex().gap(px(14.)).px(px(g)).children(items)));
        }

        // people: round faces
        if !people.is_empty() {
            let cells = people.iter().enumerate().map(|(i, f)| {
                let id = f["id"].as_u64().unwrap_or(0);
                let name = f["name"].as_str().unwrap_or("").to_string();
                let pic = f["img"].as_str().map(|u| crate::net::face(&crate::api::sized(u, "w342"), 132.));
                let group = SharedString::from(format!("fol-p-{i}"));
                let (n1, n2) = (name.clone(), name.clone());
                div()
                    .id(("fol-person", i))
                    .group(group.clone())
                    .w(px(148.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(10.))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| Nav::go(crate::Route::Person(n1.clone(), Some(id)), cx))
                    .child(
                        div()
                            .relative()
                            .size(px(132.))
                            .child(match pic {
                                Some(u) => {
                                    let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();
                                    img(u)
                                        .size(px(132.))
                                        .with_fallback(move || div().size(px(132.)).rounded_full().bg(theme::white(0.08)).flex().items_center().justify_center().text_size(px(46.)).font_weight(FontWeight::BLACK).child(initial.clone()).into_any_element())
                                        .into_any_element()
                                }
                                None => div().size(px(132.)).rounded_full().bg(theme::white(0.08)).flex().items_center().justify_center().text_size(px(46.)).font_weight(FontWeight::BLACK).child(name.chars().next().unwrap_or('?').to_uppercase().to_string()).into_any_element(),
                            })
                            // unfollow, while you point at them
                            .child(
                                div()
                                    .id(("fol-pu", i))
                                    .absolute()
                                    .top(px(0.))
                                    .right(px(0.))
                                    .size(px(34.))
                                    .rounded_full()
                                    .bg(gpui::rgba(0x151114e6))
                                    .border_1()
                                    .border_color(theme::white(0.14))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .opacity(0.)
                                    .group_hover(group.clone(), |s| s.opacity(1.))
                                    .hover(|s| s.bg(gpui::rgb(0x8a1f2c)))
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        cx.stop_propagation();
                                        s.unfollow("person".into(), id, n2.clone(), cx);
                                    }))
                                    .child(icon("close", 14.)),
                            ),
                    )
                    .child(div().w_full().text_center().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).line_clamp(2).text_color(theme::label2()).group_hover(group, |s| s.text_color(theme::text())).child(name))
            });
            page = page.child(div().px(px(g)).child(section("people", people.len()))).child(div().flex_none().px(px(g)).flex().flex_wrap().gap_x(px(18.)).gap_y(px(26.)).children(cells));
        }

        // studios: their logos on glass
        if !studios.is_empty() {
            let cells = studios.iter().enumerate().map(|(i, f)| {
                let id = f["id"].as_u64().unwrap_or(0);
                let name = f["name"].as_str().unwrap_or("").to_string();
                let group = SharedString::from(format!("fol-s-{i}"));
                let (n1, n2) = (name.clone(), name.clone());
                let logo = self.logos.get(&id).cloned();
                div()
                    .id(("fol-studio", i))
                    .group(group.clone())
                    .relative()
                    .w(px(230.))
                    .h(px(120.))
                    .rounded(px(18.))
                    .bg(theme::white(0.045))
                    .border_1()
                    .border_color(theme::white(0.06))
                    .hover(|s| s.bg(theme::white(0.09)).border_color(theme::white(0.14)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(move |_, _, cx| Nav::go(crate::Route::Studio(id, n1.clone()), cx))
                    .child(match logo {
                        Some(l) => img(l).max_w(px(150.)).max_h(px(60.)).object_fit(ObjectFit::Contain).into_any_element(),
                        None => div().px(px(16.)).text_center().text_size(px(18.)).font_weight(FontWeight::BLACK).line_clamp(2).child(name).into_any_element(),
                    })
                    .child(
                        div()
                            .id(("fol-su", i))
                            .absolute()
                            .top(px(8.))
                            .right(px(8.))
                            .size(px(30.))
                            .rounded_full()
                            .bg(gpui::rgba(0x151114e6))
                            .border_1()
                            .border_color(theme::white(0.14))
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(0.)
                            .group_hover(group, |s| s.opacity(1.))
                            .hover(|s| s.bg(gpui::rgb(0x8a1f2c)))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                cx.stop_propagation();
                                s.unfollow("studio".into(), id, n2.clone(), cx);
                            }))
                            .child(icon("close", 13.)),
                    )
            });
            page = page.child(div().px(px(g)).child(section("studios", studios.len()))).child(div().flex_none().px(px(g)).flex().flex_wrap().gap(px(14.)).children(cells));
        }
        page
    }
}
