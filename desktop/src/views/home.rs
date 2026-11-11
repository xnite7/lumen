//! Home: the billboard, then your rows (Continue watching, your lists,
//! couch's picks woven in), the same order the webview built.
use crate::api::{self, Meta};
use crate::theme;
use crate::ui::{card, hero};
use crate::Nav;
use gpui::{div, prelude::*, px, Context, FontWeight, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

pub struct Row {
    pub key: String,
    pub title: String,
    pub items: Vec<Meta>,
}

pub struct Home {
    hero: Option<Meta>,
    rows: Vec<Row>,
    scroll: ScrollHandle,
}

impl Home {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let me = Self { hero: None, rows: vec![], scroll: ScrollHandle::new() };
        cx.spawn(async move |this, cx| {
            // the billboard: one title per visit, the next one each time
            if let Ok(list) = api::call::<Vec<Meta>>("hero_items", json!({})).await {
                let list: Vec<Meta> = list.into_iter().filter(|m| m.background.is_some()).collect();
                if !list.is_empty() {
                    let st = couch::state::get();
                    let turn = st.core.db.get::<usize>("gpui_hero_turn").unwrap_or(0);
                    let _ = st.core.db.set("gpui_hero_turn", &(turn + 1));
                    let pick = list[turn % list.len()].clone();
                    let _ = this.update(cx, |v, cx| {
                        v.hero = Some(pick);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            // continue watching first
            if let Ok(items) = api::call::<Vec<Value>>("continue_watching", json!({})).await {
                let metas: Vec<Meta> = items
                    .into_iter()
                    .filter_map(|v| serde_json::from_value::<Meta>(v.get("meta").cloned().unwrap_or(v)).ok())
                    .filter(|m| !m.id.is_empty())
                    .collect();
                if !metas.is_empty() {
                    let _ = this.update(cx, |v, cx| {
                        v.rows.insert(0, Row { key: "cw".into(), title: "continue watching".into(), items: metas });
                        cx.notify();
                    });
                }
            }
            // your own lists, then couch's picks, woven together
            let catalogs = api::call::<Vec<Value>>("catalogs_list", json!({})).await.unwrap_or_default();
            let personal: Vec<Value> = catalogs
                .into_iter()
                .filter(|c| {
                    let id = c["id"].as_str().unwrap_or("");
                    c["builtin"].as_bool() == Some(true)
                        && (matches!(id, "simkl-new" | "simkl-plantowatch" | "motn-top"))
                })
                .collect();
            let curated = api::call::<Vec<Value>>("home_rows", json!({ "scope": "all" })).await.unwrap_or_default();
            let mut specs: Vec<(Value, Vec<(String, String)>)> = vec![];
            let (mut a, mut b) = (personal.into_iter(), curated.into_iter());
            loop {
                let (x, y) = (a.next(), b.next());
                if x.is_none() && y.is_none() {
                    break;
                }
                if let Some(c) = x {
                    specs.push((c, vec![]));
                }
                if let Some(r) = y {
                    let extra: Vec<(String, String)> = serde_json::from_value(r["extra"].clone()).unwrap_or_default();
                    let cat = json!({
                        "key": format!("couch/{}/{}/{}", r["type"].as_str().unwrap_or(""), r["id"].as_str().unwrap_or(""), extra.first().map(|e| e.1.clone()).unwrap_or_default()),
                        "addonId": "com.midnite.couch.builtin",
                        "addonName": "couch",
                        "transportUrl": "couch://builtin/manifest.json",
                        "type": r["type"],
                        "id": r["id"],
                        "name": r["name"],
                        "extra": [{ "name": "skip" }],
                        "adult": false,
                        "builtin": true,
                    });
                    specs.push((cat, extra));
                }
            }
            // fetch them all at once; each lands where it belongs
            let tasks: Vec<_> = specs
                .into_iter()
                .map(|(cat, extra)| async move {
                    let key = cat["key"].as_str().unwrap_or("").to_string();
                    let title = cat["name"].as_str().unwrap_or("").to_lowercase();
                    let items = api::call::<Vec<Meta>>("catalog_fetch", json!({ "catalog": cat, "extra": extra })).await.unwrap_or_default();
                    Row { key, title, items }
                })
                .collect();
            let rows = futures::future::join_all(tasks).await;
            let _ = this.update(cx, |v, cx| {
                v.rows.extend(rows.into_iter().filter(|r| !r.items.is_empty()));
                cx.notify();
            });
        })
        .detach();
        me
    }
}

impl Render for Home {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vh = window.viewport_size().height;
        let mut page = div().id("home").size_full().overflow_y_scroll().track_scroll(&self.scroll).bg(theme::bg());
        match &self.hero {
            Some(m) => {
                let (a, b, c) = (m.clone(), m.clone(), m.clone());
                page = page.child(hero::hero(
                    m,
                    vh * 0.86,
                    Box::new(move |_, _, cx| Nav::play(&a, cx)),
                    Box::new(move |_, _, cx| Nav::detail(&b, cx)),
                    Box::new(move |_, _, cx| Nav::add(&c, cx)),
                ));
            }
            None => page = page.child(div().w_full().h(vh * 0.86).bg(theme::bg())),
        }
        let mut below = div().relative().mt(px(-120.)).flex().flex_col().gap(px(34.)).pb(px(80.));
        for (ri, r) in self.rows.iter().enumerate() {
            below = below.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().px(px(theme::GUTTER)).text_size(px(21.)).font_weight(FontWeight::BOLD).text_color(theme::text()).child(r.title.clone()))
                    .child(
                        div()
                            .id(SharedString::from(format!("row-{}", r.key)))
                            .overflow_x_scroll()
                            .flex()
                            .gap(px(10.))
                            .px(px(theme::GUTTER))
                            .children(r.items.iter().enumerate().map(|(i, m)| {
                                let mm = m.clone();
                                card::card(SharedString::from(format!("c-{ri}-{i}")), m, move |_, _, cx| Nav::detail(&mm, cx))
                            })),
                    ),
            );
        }
        let _ = cx;
        page.child(below)
    }
}
