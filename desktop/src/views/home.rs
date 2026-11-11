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
    /// Continue watching: (progress, "S2 E4 · title") per item
    pub extra: Vec<(Option<f32>, Option<String>)>,
}

pub struct Home {
    scope: &'static str,
    hero: Option<Meta>,
    rows: Vec<Row>,
    scroll: ScrollHandle,
}

impl Home {
    /// scope: "all" (Home), "series" (Shows) or "movie" (Movies)
    pub fn new(scope: &'static str, cx: &mut Context<Self>) -> Self {
        let me = Self { scope, hero: None, rows: vec![], scroll: ScrollHandle::new() };
        cx.spawn(async move |this, cx| {
            // the billboard: one title per visit, the next one each time
            if let Ok(list) = api::call::<Vec<Meta>>("hero_items", json!({})).await {
                let list: Vec<Meta> = list.into_iter().filter(|m| m.background.is_some() && (scope == "all" || m.kind == scope)).collect();
                if !list.is_empty() {
                    let st = couch::state::get();
                    let turn = st.core.db.get::<usize>("gpui_hero_turn").unwrap_or(0);
                    if scope == "all" {
                        let _ = st.core.db.set("gpui_hero_turn", &(turn + 1));
                    }
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
            // continue watching first (Home only)
            if scope != "all" {
            } else if let Ok(items) = api::call::<Vec<Value>>("continue_watching", json!({})).await {
                let mut metas: Vec<Meta> = vec![];
                let mut extra = vec![];
                for v in items {
                    let m = Meta {
                        id: v["_id"].as_str().or(v["id"].as_str()).unwrap_or("").into(),
                        kind: v["type"].as_str().unwrap_or("").into(),
                        name: v["name"].as_str().unwrap_or("").into(),
                        poster: v["poster"].as_str().map(|s| s.into()),
                        background: v["background"].as_str().map(|s| s.into()),
                        logo: v["logo"].as_str().map(|s| s.into()),
                        ..Default::default()
                    };
                    if m.id.is_empty() {
                        continue;
                    }
                    let st = &v["state"];
                    let d = st["duration"].as_f64().unwrap_or(0.);
                    let p = (d > 0.).then(|| (st["timeOffset"].as_f64().unwrap_or(0.) / d) as f32);
                    let vid = st["video_id"].as_str().unwrap_or("");
                    let parts: Vec<&str> = vid.split(':').collect();
                    let sub = (parts.len() == 3).then(|| format!("S{} E{}", parts[1], parts[2]));
                    metas.push(m);
                    extra.push((p, sub));
                }
                if !metas.is_empty() {
                    let _ = this.update(cx, |v, cx| {
                        v.rows.insert(0, Row { key: "cw".into(), title: "continue watching".into(), items: metas, extra });
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
                    let personal = c["builtin"].as_bool() == Some(true) && matches!(id, "simkl-new" | "simkl-plantowatch" | "motn-top" | "simkl-upnext" | "simkl-watching");
                    let browsable = !c["extra"].as_array().into_iter().flatten().any(|e| e["isRequired"].as_bool() == Some(true) && (e["name"] == "search" || e["options"].as_array().map_or(true, |o| o.is_empty())));
                    if scope == "all" {
                        personal && !matches!(id, "simkl-upnext" | "simkl-watching")
                    } else {
                        browsable && !personal && c["type"].as_str() == Some(scope)
                    }
                })
                .collect();
            let curated = api::call::<Vec<Value>>("home_rows", json!({ "scope": scope })).await.unwrap_or_default();
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
                    Row { key, title, items, extra: vec![] }
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
        let mut page = div().id(SharedString::from(format!("home-{}", self.scope))).size_full().overflow_y_scroll().track_scroll(&self.scroll).bg(theme::bg());
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
                                let (p, sub) = r.extra.get(i).cloned().unwrap_or((None, None));
                                if r.key == "cw" {
                                    // straight back in where you left off
                                    card::card_with(SharedString::from(format!("c-{ri}-{i}")), m, p, sub, move |_, _, cx| Nav::play(&mm, cx))
                                } else {
                                    card::card(SharedString::from(format!("c-{ri}-{i}")), m, move |_, _, cx| Nav::detail(&mm, cx))
                                }
                            })),
                    ),
            );
        }
        let _ = cx;
        page.child(below)
    }
}
