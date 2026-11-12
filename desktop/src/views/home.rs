//! Home: the billboard, then your rows (Continue watching, your lists,
//! couch's picks woven in), the same order the webview built.
use crate::api::{self, Meta};
use crate::theme;
use crate::ui::{card, hero};
use crate::Nav;
use crate::views::catalog as views_catalog;
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
    /// the billboard's clip: (owner, url)
    pub clip: Option<(String, String)>,
    hero: Option<Meta>,
    rows: Vec<Row>,
    /// streaming services: (TMDB provider id, name, logo)
    services: Vec<(i64, String, Option<String>)>,
    /// catalogs to browse, and the genres of one of them
    browse: Vec<Value>,
    genres: (Option<Value>, Vec<String>),
    /// the crew: who's watching now, and what they've been up to
    live: Vec<Value>,
    feed: Vec<Value>,
    scroll: ScrollHandle,
}

fn ago(at: f64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as f64;
    let s = ((now - at) / 1000.).max(1.) as i64;
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{}m", s / 60),
        3600..=86399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86400),
    }
}

impl Home {
    /// scope: "all" (Home), "series" (Shows) or "movie" (Movies)
    pub fn new(scope: &'static str, cx: &mut Context<Self>) -> Self {
        cx.observe(&crate::ui::video::get(cx), |_, _, cx| cx.notify()).detach();
        let me = Self { scope, clip: None, hero: None, rows: vec![], services: vec![], browse: vec![], genres: (None, vec![]), live: vec![], feed: vec![], scroll: ScrollHandle::new() };
        if scope == "all" {
            cx.spawn(async move |this, cx| loop {
                let crew = api::call::<Value>("crew_list", json!({})).await.unwrap_or_default();
                let feed = api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/activity", "body": Value::Null })).await.unwrap_or_default();
                let ok = this.update(cx, |v, cx| {
                    v.live = crew["members"].as_array().into_iter().flatten().filter(|m| m["watching"]["title"].is_string()).cloned().collect();
                    v.feed = feed["feed"].as_array().into_iter().flatten().take(24).cloned().collect();
                    cx.notify();
                });
                if ok.is_err() {
                    break;
                }
                gpui::Timer::after(std::time::Duration::from_secs(60)).await;
            })
            .detach();
        }
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
                        v.hero = Some(pick.clone());
                        cx.notify();
                    });
                    // a couple of seconds in, the billboard plays a clip (muted)
                    gpui::Timer::after(std::time::Duration::from_millis(2000)).await;
                    let year = pick.year();
                    let clip = match std::env::var("COUCH_TEST_CLIP") {
                        Ok(f) => Ok(Some(f)),
                        Err(_) => api::call::<Option<String>>("clip_url", json!({ "id": pick.id, "title": pick.name, "year": year })).await,
                    };
                    if let Ok(Some(url)) = clip {
                        let owner = format!("hero:{scope}:{}", pick.id);
                        let _ = this.update(cx, |v, cx| {
                            v.clip = Some((owner.clone(), url.clone()));
                            let vid = crate::ui::video::get(cx);
                            vid.update(cx, |x, cx| x.play(&owner, &url, cx));
                        });
                    }
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
            // new from the actors, directors and studios you follow
            if scope == "all" {
                if let Ok(list) = api::call::<Vec<Meta>>("follow_feed", json!({})).await {
                    if !list.is_empty() {
                        let _ = this.update(cx, |v, cx| {
                            let at = v.rows.iter().position(|r| r.key == "cw").map_or(0, |i| i + 1);
                            v.rows.insert(at, Row { key: "follow".into(), title: "new from who you follow".into(), items: list, extra: vec![] });
                            cx.notify();
                        });
                    }
                }
            }
            // your own lists, then couch's picks, woven together
            let catalogs = api::call::<Vec<Value>>("catalogs_list", json!({})).await.unwrap_or_default();
            if scope == "all" {
                let browse: Vec<Value> = catalogs
                    .iter()
                    .filter(|c| !c["extra"].as_array().into_iter().flatten().any(|e| e["isRequired"].as_bool() == Some(true) && e["options"].as_array().map_or(true, |o| o.is_empty())))
                    .filter(|c| c["adult"].as_bool() != Some(true))
                    .take(12)
                    .cloned()
                    .collect();
                let gsrc = catalogs.iter().find(|c| c["type"] == "movie" && c["extra"].as_array().into_iter().flatten().any(|e| e["name"] == "genre" && e["options"].as_array().map_or(false, |o| !o.is_empty()))).cloned();
                let genres: Vec<String> = gsrc
                    .as_ref()
                    .and_then(|c| c["extra"].as_array()?.iter().find(|e| e["name"] == "genre")?["options"].as_array().cloned())
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|g| g.as_str().map(String::from))
                    .take(18)
                    .collect();
                let services: Vec<(i64, String, Option<String>)> = api::call::<Vec<Value>>("streaming_services", json!({}))
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|s| Some((s["id"].as_i64()?, s["name"].as_str().unwrap_or("").to_string(), crate::ui::badges::service(s["id"].as_i64()?).or(s["logo"].as_str().map(String::from)))))
                    .collect();
                let _ = this.update(cx, |v, cx| {
                    v.browse = browse;
                    v.genres = (gsrc, genres);
                    v.services = services;
                    cx.notify();
                });
            }
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
                let vid = crate::ui::video::get(cx);
                // the clip: resume it if we came back to this page
                let clip = match &self.clip {
                    Some((owner, url)) => {
                        if vid.read(cx).owner.is_empty() {
                            let (o, u) = (owner.clone(), url.clone());
                            vid.update(cx, |x, cx| x.play(&o, &u, cx));
                        }
                        let owner = owner.clone();
                        vid.update(cx, |x, _| {
                            let f = x.frame(&owner, window)?;
                            let a = x.since.map_or(0., |t| (t.elapsed().as_secs_f32() / 0.8).min(1.));
                            Some((f, a))
                        })
                    }
                    None => None,
                };
                if clip.as_ref().map_or(false, |c| c.1 < 1.) {
                    window.request_animation_frame();
                }
                page = page.child(hero::hero(
                    m,
                    vh * 0.86,
                    clip,
                    Box::new(move |_, _, cx| Nav::play(&a, cx)),
                    Box::new(move |_, _, cx| Nav::detail(&b, cx)),
                    Box::new(move |_, _, cx| Nav::add(&c, cx)),
                ));
            }
            None => page = page.child(div().w_full().h(vh * 0.86).bg(theme::bg())),
        }
        // the card that's previewing (one at a time)
        let vid = crate::ui::video::get(cx);
        let owner = vid.read(cx).owner.clone();
        let card_clip = if owner.starts_with("card:") {
            let o = owner.clone();
            vid.update(cx, |x, _| {
                let f = x.frame(&o, window)?;
                let t = x.since.map_or(0., |t| t.elapsed().as_secs_f32());
                Some((f, (t / 0.5).min(1.), t))
            })
        } else {
            None
        };
        if card_clip.as_ref().map_or(false, |c| c.2 < 3.) {
            window.request_animation_frame();
        }
        let card_owner = owner.trim_start_matches("card:").to_string();
        let mut below = div().relative().mt(px(-120.)).flex().flex_col().gap(px(34.)).pb(px(80.));
        let mut crew_rows: Vec<gpui::AnyElement> = vec![];
        // on the couch: what the crew is watching right now
        if !self.live.is_empty() {
            let cards = self.live.iter().enumerate().map(|(i, mb)| {
                let w = &mb["watching"];
                let m = Meta {
                    id: w["metaId"].as_str().unwrap_or("").into(),
                    kind: w["type"].as_str().unwrap_or("").into(),
                    name: w["title"].as_str().unwrap_or("").into(),
                    poster: w["poster"].as_str().map(String::from),
                    background: w["background"].as_str().map(String::from),
                    logo: w["logo"].as_str().map(String::from),
                    ..Default::default()
                };
                let frac = match (mb["pos"].as_f64(), mb["duration"].as_f64()) {
                    (Some(p), Some(d)) if d > 0. => Some((p / d).clamp(0., 1.) as f32),
                    _ => None,
                };
                let who = mb["name"].as_str().unwrap_or("someone").to_string();
                let sub = [format!("{who} is watching"), w["episode"].as_str().unwrap_or("").to_string()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
                card::card_with(SharedString::from(format!("live-{i}")), &m, frac, Some(sub), None, |_, _, cx| Nav::go(crate::Route::Couch, cx))
            });
            crew_rows.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().px(px(theme::GUTTER)).flex().items_center().gap(px(10.)).child(div().text_size(px(21.)).font_weight(FontWeight::BOLD).child("on the couch")).child(div().size(px(8.)).rounded_full().bg(theme::neon())).child(div().text_size(px(14.)).text_color(theme::label2()).child(format!("{} watching", self.live.len()))))
                    .child(div().id("row-live").overflow_x_scroll().flex().gap(px(10.)).px(px(theme::GUTTER)).children(cards))
                    .into_any_element(),
            );
        }
        // the crew's activity: reviews, ratings, finished
        if !self.feed.is_empty() {
            let items = self.feed.iter().enumerate().map(|(i, a)| {
                let t = &a["title"];
                let m = Meta { id: t["id"].as_str().unwrap_or("").into(), kind: t["type"].as_str().unwrap_or("").into(), name: t["name"].as_str().unwrap_or("").into(), poster: t["poster"].as_str().map(String::from), background: t["background"].as_str().map(String::from), ..Default::default() };
                let who = a["name"].as_str().unwrap_or("someone").to_string();
                let wid = a["who"].as_str().unwrap_or("").to_string();
                let what = match a["kind"].as_str().unwrap_or("") {
                    "review" => "reviewed",
                    "rating" => "rated",
                    "finished" => "finished",
                    k => k,
                }
                .to_string();
                let rating = a["rating"].as_i64();
                let text = a["text"].as_str().unwrap_or("").to_string();
                let when = a["at"].as_f64().map(ago).unwrap_or_default();
                let mm = m.clone();
                div()
                    .id(("feed", i))
                    .flex_none()
                    .w(px(320.))
                    .p(px(12.))
                    .flex()
                    .gap(px(12.))
                    .rounded(px(16.))
                    .bg(theme::white(0.05))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::white(0.09)))
                    .on_click(move |_, _, cx| Nav::detail(&mm, cx))
                    .child(div().w(px(64.)).h(px(96.)).flex_none().rounded(px(8.)).overflow_hidden().bg(theme::surface2()).children(m.poster.as_ref().map(|u| gpui::img(crate::api::sized(u, "w342")).size_full().object_fit(gpui::ObjectFit::Cover))))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .text_size(px(13.))
                                    .child(div().id(("feed-who", i)).font_weight(FontWeight::BOLD).cursor_pointer().hover(|s| s.underline()).on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        Nav::go(crate::Route::Member(wid.clone()), cx)
                                    }).child(who))
                                    .child(div().text_color(theme::label2()).child(what))
                                    .child(div().text_color(theme::label3()).child(when)),
                            )
                            .child(div().text_size(px(15.)).font_weight(FontWeight::BLACK).line_clamp(1).text_ellipsis().child(m.name.clone()))
                            .children(rating.map(|r| div().text_size(px(13.)).font_weight(FontWeight::BLACK).text_color(theme::lamp()).child(format!("★ {r}/10"))))
                            .children((!text.is_empty()).then(|| div().text_size(px(13.)).line_height(px(18.)).text_color(theme::white(0.8)).line_clamp(2).child(crate::ui::emoji::text(&text, 13.)))),
                    )
            });
            crew_rows.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().px(px(theme::GUTTER)).text_size(px(21.)).font_weight(FontWeight::BOLD).child("the crew lately"))
                    .child(div().id("row-feed").overflow_x_scroll().flex().gap(px(10.)).px(px(theme::GUTTER)).children(items))
                    .into_any_element(),
            );
        }
        // after continue watching / who you follow
        let lead = self.rows.iter().take_while(|r| r.key == "cw" || r.key == "follow").count();
        for (ri, r) in self.rows.iter().enumerate() {
            if ri == lead {
                for c in crew_rows.drain(..) {
                    below = below.child(c);
                }
            }
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
                                    card::card_with(SharedString::from(format!("c-{ri}-{i}")), m, p, sub, None, move |_, _, cx| Nav::play(&mm, cx))
                                } else {
                                    let clip = if m.id == card_owner { card_clip.clone() } else { None };
                                    card::card_preview(SharedString::from(format!("c-{ri}-{i}")), m, clip, move |_, _, cx| Nav::detail(&mm, cx))
                                }
                            })),
                    ),
            );
        }
        for c in crew_rows.drain(..) {
            below = below.child(c);
        }
        // browse: every catalog as a tile, then genres
        if !self.browse.is_empty() {
            let tints = [0x3a1830, 0x2a1a24, 0x3b2416, 0x241626, 0x33152a, 0x2d1d1a, 0x1f1824, 0x3a2014, 0x281422, 0x1d1418];
            let tiles = self.browse.iter().enumerate().map(|(i, c)| {
                let label = match c["type"].as_str().unwrap_or("") {
                    "movie" => "films",
                    "series" => "shows",
                    "tv" => "channels",
                    other => other,
                }
                .to_string();
                let name = c["name"].as_str().unwrap_or("").to_lowercase();
                let spec = views_catalog::Spec { title: name.clone(), service: None, catalog: c.clone(), genre: None };
                div()
                    .id(("browse", i))
                    .flex_none()
                    .w(px(220.))
                    .h(px(110.))
                    .p(px(16.))
                    .rounded(px(16.))
                    .bg(gpui::rgb(tints[i % tints.len()]))
                    .flex()
                    .flex_col()
                    .justify_end()
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.85))
                    .on_click(move |_, _, cx| Nav::go(crate::Route::Catalog(spec.clone()), cx))
                    .child(div().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(theme::label2()).child(label))
                    .child(div().text_size(px(18.)).font_weight(FontWeight::BLACK).child(name))
            });
            let genres = self.genres.1.iter().enumerate().map(|(i, gname)| {
                let spec = views_catalog::Spec { title: self.genres.0.as_ref().and_then(|c| c["name"].as_str()).unwrap_or("films").to_string(), service: None, catalog: self.genres.0.clone().unwrap_or_default(), genre: Some(gname.clone()) };
                div()
                    .id(("genre", i))
                    .flex_none()
                    .px(px(18.))
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .bg(gpui::rgb(tints[(i + 3) % tints.len()]))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.85))
                    .on_click(move |_, _, cx| Nav::go(crate::Route::Catalog(spec.clone()), cx))
                    .child(gname.to_lowercase())
            });
            below = below.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().px(px(theme::GUTTER)).text_size(px(21.)).font_weight(FontWeight::BOLD).child("browse"))
                    .child(
                        div()
                            .id("browse-tiles")
                            .overflow_x_scroll()
                            .flex()
                            .gap(px(10.))
                            .px(px(theme::GUTTER))
                            .child(
                                div()
                                    .id("browse-vibe")
                                    .flex_none()
                                    .w(px(220.))
                                    .h(px(110.))
                                    .p(px(16.))
                                    .rounded(px(16.))
                                    .bg(gpui::linear_gradient(135., gpui::linear_color_stop(theme::neon(), 0.), gpui::linear_color_stop(gpui::rgb(0x5b2a86), 1.)))
                                    .flex()
                                    .flex_col()
                                    .justify_between()
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.88))
                                    .on_click(|_, _, cx| Nav::go(crate::Route::Vibe, cx))
                                    .child(crate::ui::icon("sparkles", 24.))
                                    .child(div().flex().flex_col().child(div().text_size(px(18.)).font_weight(FontWeight::BLACK).child("smart bar")).child(div().text_size(px(12.)).text_color(theme::white(0.8)).child("ask for a feeling"))),
                            )
                            .children(tiles),
                    )
                    .child(div().id("browse-genres").overflow_x_scroll().flex().gap(px(8.)).px(px(theme::GUTTER)).children(genres)),
            );
        }
        // streaming services
        if !self.services.is_empty() {
            below = below.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().px(px(theme::GUTTER)).text_size(px(21.)).font_weight(FontWeight::BOLD).child("services"))
                    .child(div().id("services").overflow_x_scroll().flex().gap(px(10.)).px(px(theme::GUTTER)).children(self.services.iter().map(|(id, name, logo)| {
                        let spec = views_catalog::Spec { title: name.clone(), service: Some((*id, logo.clone())), catalog: Value::Null, genre: None };
                        div()
                            .id(SharedString::from(format!("svc-{id}")))
                            .flex_none()
                            .w(px(180.))
                            .h(px(96.))
                            .rounded(px(16.))
                            .bg(theme::white(0.06))
                            .border_1()
                            .border_color(theme::white(0.08))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::white(0.12)))
                            .on_click(move |_, _, cx| Nav::go(crate::Route::Catalog(spec.clone()), cx))
                            .child(match logo {
                                Some(u) => gpui::img(u.clone()).max_w(px(130.)).max_h(px(52.)).object_fit(gpui::ObjectFit::Contain).into_any_element(),
                                None => div().font_weight(FontWeight::BLACK).child(name.clone()).into_any_element(),
                            })
                    }))),
            );
        }
        let _ = cx;
        page.child(below)
    }
}
