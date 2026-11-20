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
    /// where its header goes: the whole list as a page
    pub spec: Option<views_catalog::Spec>,
}

pub struct Home {
    scope: &'static str,
    /// the billboard's clip: (owner, url)
    pub clip: Option<(String, String)>,
    /// scrolled away from the billboard: its clip waits (Some = when we came back to the top)
    away: Option<Option<std::time::Instant>>,
    /// the clip picked up again here: fade it back in from this moment
    resumed: Option<std::time::Instant>,
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
    /// nights in a row (theirs, and the ones you watched together)
    streaks: Vec<crate::views::crewx::Streak>,
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
        let me = Self { scope, away: None, resumed: None, clip: None, hero: None, rows: vec![], services: vec![], browse: vec![], genres: (None, vec![]), live: vec![], feed: vec![], streaks: vec![], scroll: ScrollHandle::new() };
        if scope == "all" {
            cx.spawn(async move |this, cx| {
                let mut n = 0u32;
                loop {
                let crew = api::call::<Value>("crew_list", json!({})).await.unwrap_or_default();
                let feed = api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/activity", "body": Value::Null })).await.unwrap_or_default();
                let all: Vec<Value> = feed["feed"].as_array().cloned().unwrap_or_default();
                let ok = this.update(cx, |v, cx| {
                    v.live = crew["members"].as_array().into_iter().flatten().filter(|m| m["watching"]["title"].is_string()).cloned().collect();
                    v.feed = all.iter().take(24).cloned().collect();
                    cx.notify();
                });
                if ok.is_err() {
                    break;
                }
                // streaks and the crew's picks: now, then every 10 minutes
                if n % 10 == 0 {
                    let x = crate::views::crewx::load(&all).await;
                    let _ = this.update(cx, |v, cx| {
                        v.streaks = x.streaks;
                        v.rows.retain(|r| r.key != "crewpicks");
                        if !x.picks.is_empty() {
                            let at = v.rows.iter().take_while(|r| r.key == "cw" || r.key == "follow").count();
                            let spec = views_catalog::Spec { title: x.picks_title.clone(), fixed: Some(x.picks.clone()), ..Default::default() };
                            v.rows.insert(at, Row { key: "crewpicks".into(), title: x.picks_title, items: x.picks, extra: vec![], spec: Some(spec) });
                        }
                        cx.notify();
                    });
                }
                n += 1;
                crate::timer::after(std::time::Duration::from_secs(60)).await;
                }
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
                        if v.scope == "all" {
                            crate::ui::ambient::set_home(&pick, cx);
                            // its logo's colours (settings' bar samples use them)
                            if let Some(src) = pick.logo.clone() {
                                cx.spawn(async move |_, _| {
                                    let t = couch::rt::spawn(async move {
                                        let core = couch::state::get().core.clone();
                                        let (b, _) = couch::images::load(&core, &crate::api::sized(&src, "w500")).await.ok()?;
                                        crate::views::player::tint_of(&b)
                                    })
                                    .await
                                    .ok()
                                    .flatten();
                                    if let Some(t) = t {
                                        crate::views::player::set_last_tint(t);
                                    }
                                })
                                .detach();
                            }
                        }
                        crate::views::detail::prefetch(&pick, cx);
                        cx.notify();
                    });
                    // a couple of seconds in, the billboard plays the title's
                    // trailer, once (a clip only when there's no trailer)
                    crate::timer::after(std::time::Duration::from_millis(2000)).await;
                    let year = pick.year();
                    let trailer = api::call::<Option<String>>("trailer_get", json!({ "kind": pick.kind, "id": pick.id })).await.ok().flatten().filter(|k| !k.is_empty()).map(|k| format!("https://www.youtube.com/watch?v={k}"));
                    let url = match (std::env::var("COUCH_TEST_CLIP"), trailer) {
                        (Ok(f), _) => Some(f),
                        (_, Some(t)) => Some(t),
                        _ => api::call::<Option<String>>("clip_url", json!({ "id": pick.id, "title": pick.name, "year": year })).await.ok().flatten(),
                    };
                    if let Some(url) = url {
                        let owner = format!("hero:{scope}:{}", pick.id);
                        let _ = this.update(cx, |v, cx| {
                            v.clip = Some((owner.clone(), url.clone()));
                            let vid = crate::ui::video::get(cx);
                            vid.update(cx, |x, cx| x.play_once(&owner, &url, cx));
                        });
                    }
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            // continue watching first (Home only)
            if scope == "all" {
                if let Some((metas, extra)) = cw_row().await {
                    let _ = this.update(cx, |v, cx| v.set_cw(metas, extra, cx));
                }
            }
            // new from the actors, directors and studios you follow
            if scope == "all" {
                if let Ok(list) = api::call::<Vec<Meta>>("follow_feed", json!({})).await {
                    if !list.is_empty() {
                        let _ = this.update(cx, |v, cx| {
                            let at = v.rows.iter().position(|r| r.key == "cw").map_or(0, |i| i + 1);
                            v.rows.insert(at, Row { key: "follow".into(), title: "new from who you follow".into(), spec: Some(views_catalog::Spec { title: "new from who you follow".into(), fixed: Some(list.clone()), ..Default::default() }), items: list, extra: vec![] });
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
                    let spec = views_catalog::Spec { title: title.clone(), catalog: cat, extra, ..Default::default() };
                    Row { key, title, items, extra: vec![], spec: Some(spec) }
                })
                .collect();
            let rows = futures::future::join_all(tasks).await;
            let _ = this.update(cx, |v, cx| {
                v.rows.extend(rows.into_iter().filter(|r| !r.items.is_empty()));
                cx.notify();
            });
            // because you watched …: your last two finishes, and what's like them
            if scope == "all" {
                let byw = because_you_watched().await;
                let _ = this.update(cx, |v, cx| {
                    // these replace the engine's "more like …" rows (the same picks, twice otherwise)
                    if !byw.is_empty() {
                        v.rows.retain(|r| !r.key.contains("/couch-morelike/"));
                    }
                    // a little way down: after the first couple of regular rows
                    let lead = v.rows.iter().take_while(|r| matches!(r.key.as_str(), "cw" | "follow" | "crewpicks")).count();
                    for (k, r) in byw.into_iter().enumerate() {
                        let at = (lead + 2 + k * 3).min(v.rows.len());
                        v.rows.insert(at, r);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
        me
    }
}

impl Render for Home {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // taken off "continue watching" from the right-click menu: gone right away
        let gone = super::menu::dismissed();
        if !gone.is_empty() {
            if let Some(r) = self.rows.iter_mut().find(|r| r.key == "cw") {
                let keep: Vec<bool> = r.items.iter().map(|m| !gone.contains(&m.id)).collect();
                if keep.iter().any(|k| !k) {
                    let mut k = keep.iter();
                    r.items.retain(|_| *k.next().unwrap_or(&true));
                    let mut k = keep.iter();
                    r.extra.retain(|_| *k.next().unwrap_or(&true));
                    if let Some(spec) = r.spec.as_mut() {
                        spec.fixed = Some(r.items.clone());
                    }
                }
            }
            self.rows.retain(|r| r.key != "cw" || !r.items.is_empty());
        }
        let vh = window.viewport_size().height;
        let mut page = div().id(SharedString::from(format!("home-{}", self.scope))).size_full().overflow_y_scroll().track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, self.scope));
        match &self.hero {
            Some(m) => {
                let (a, b, c) = (m.clone(), m.clone(), m.clone());
                let vid = crate::ui::video::get(cx);
                // scrolled past the billboard: the clip waits and the backdrop and logo
                // show; back at the top, a second's beat before it plays on
                let gone = -f32::from(self.scroll.offset().y) > f32::from(vh) * 0.45;
                if gone {
                    self.away = Some(None);
                } else if let Some(back) = self.away {
                    match back {
                        None => self.away = Some(Some(std::time::Instant::now())),
                        Some(t) if t.elapsed().as_secs_f32() >= 1. => {
                            self.away = None;
                            self.resumed = Some(std::time::Instant::now());
                        }
                        Some(_) => {}
                    }
                    if self.away.is_some() {
                        // check again in a moment
                        cx.spawn(async move |this, cx| {
                            crate::timer::after(std::time::Duration::from_millis(250)).await;
                            let _ = this.update(cx, |_, cx| cx.notify());
                        })
                        .detach();
                    }
                }
                let waiting = self.away.is_some();
                let resumed = self.resumed;
                // the clip: resume it if we came back to this page
                let clip = match &self.clip {
                    Some((owner, url)) => {
                        if vid.read(cx).owner.is_empty() {
                            let (o, u) = (owner.clone(), url.clone());
                            vid.update(cx, |x, cx| x.play(&o, &u, cx));
                        }
                        let owner = owner.clone();
                        vid.update(cx, |x, _| {
                            x.set_paused(&owner, waiting);
                            let f = x.frame(&owner, window)?;
                            if waiting {
                                return None;
                            }
                            let a = x.since.map_or(0., |t| (t.elapsed().as_secs_f32() / 0.8).min(1.));
                            let b = resumed.map_or(1., |t| (t.elapsed().as_secs_f32() / 0.8).min(1.));
                            Some((f, a.min(b)))
                        })
                    }
                    None => None,
                };
                if clip.as_ref().map_or(false, |c| c.1 < 1.) {
                    window.request_animation_frame();
                }
                page = page.child(hero::hero(
                    m,
                    // a taller billboard: more picture above the title
                    vh * 0.94,
                    clip,
                    Box::new(move |_, _, cx| Nav::play(&a, cx)),
                    Box::new(move |_, _, cx| Nav::detail(&b, cx)),
                    Box::new(move |_, _, cx| Nav::add(&c, cx)),
                ));
            }
            None => page = page.child(div().w_full().h(vh * 0.86)),
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
            // one card per title; everyone watching it, as faces on the picture
            let mut groups: Vec<(String, Vec<&Value>)> = vec![];
            for mb in &self.live {
                let id = mb["watching"]["metaId"].as_str().unwrap_or("").to_string();
                match groups.iter_mut().find(|g| g.0 == id) {
                    Some(g) => g.1.push(mb),
                    None => groups.push((id, vec![mb])),
                }
            }
            let cards = groups.into_iter().enumerate().map(|(i, (_, who))| {
                let mb = who[0];
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
                let (se, _ep_title) = crate::ui::split_episode(w["episode"].as_str().unwrap_or(""));
                let people: Vec<(Option<String>, String)> = who.iter().map(|p| (p["avatar"].as_str().map(String::from), p["name"].as_str().unwrap_or("someone").to_string())).collect();
                let key = format!("live-{i}");
                let paused = mb["paused"].as_bool() == Some(true);
                let (host, host_name) = (mb["id"].as_str().unwrap_or("").to_string(), mb["name"].as_str().unwrap_or("them").to_string());
                let mm = m.clone();
                // I've asked to join this one: a yellow ring until they answer
                let pending = crate::together::get(cx).read(cx).asking.as_ref().map_or(false, |a| a.1 == host);
                div()
                    .id(SharedString::from(format!("live-wrap-{i}")))
                    .group(SharedString::from(format!("live-g-{i}")))
                    .relative()
                    .flex_none()
                    // a click on it: the title's page (join is the button)
                    .child(card::card_with(SharedString::from(key.clone()), &m, frac, None, None, move |_, _, cx| Nav::detail(&mm, cx)))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w(px(card::W))
                            .h(px(card::H))
                            // paused over there: a small pause mark (playing needs no mark)
                            .when(paused, |d| d.child(div().absolute().top(px(10.)).left(px(10.)).size(px(28.)).rounded_full().bg(theme::black(0.6)).flex().items_center().justify_center().child(crate::ui::icon_fill("pause", 14.))))
                            .children(se.map(|t| div().absolute().top(px(10.)).right(px(10.)).child(crate::ui::ep_badge(t))))
                            .child(div().absolute().bottom(px(10.)).right(px(10.)).child(crate::ui::faces(&key, people, 30.)))
                            .when(pending, |d| d.child(div().absolute().top_0().left_0().size_full().rounded(px(12.)).border_2().border_color(gpui::rgb(0xffc83d))))
                            // pointing at it: join them (asked already: always shown, cancels)
                            .child(
                                div().absolute().size_full().flex().items_center().justify_center().when(!pending, |d| d.opacity(0.).group_hover(SharedString::from(format!("live-g-{i}")), |s| s.opacity(1.))).child(
                                    div()
                                        .id(SharedString::from(format!("live-join-{i}")))
                                        .occlude()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .px(px(18.))
                                        .h(px(40.))
                                        .rounded(px(12.))
                                        .bg(theme::text())
                                        .text_color(gpui::rgb(0x000000))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .cursor_pointer()
                                        .hover(|s| s.opacity(0.88))
                                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                        .when(pending, |d| d.bg(gpui::rgb(0xffc83d)))
                                        .on_click(move |_, _, cx| {
                                            cx.stop_propagation();
                                            let (h, n) = (host.clone(), host_name.clone());
                                            crate::together::get(cx).update(cx, |t, cx| if pending { t.cancel_ask(cx) } else { t.ask_to_join(h, n, cx) });
                                        })
                                        .child(crate::ui::icon("couch", 18.).text_color(gpui::rgb(0x000000)))
                                        .child(if pending { "asked · cancel" } else { "join" }),
                                ),
                            ),
                    )
            });
            // on the couch leads the page, above continue watching
            below = below.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div().px(px(theme::GUTTER)).flex().child(
                            div()
                                .id("live-head")
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.8))
                                .on_click(|_, _, cx| Nav::go(crate::Route::Couch, cx))
                                .child(div().text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).child("on the couch"))
                                .child(crate::ui::icon("chevronWide", 20.).text_color(theme::label3())),
                        ),
                    )
                    .child(crate::ui::rows::scroller("row-live", div().id("row-live").overflow_x_scroll().flex().gap(px(10.)).px(px(theme::GUTTER)).children(cards)))
                    .into_any_element(),
            );
        }
        // the crew's activity: reviews, ratings, finished
        if !self.feed.is_empty() {
            // one card per title, everyone who rated or reviewed it together;
            // a review gets a big quote card over the art, a bare rating a poster
            // with the scores on it, so the row isn't the same box over and over
            let mut groups: Vec<(String, Vec<&Value>)> = vec![];
            for a in &self.feed {
                let id = a["title"]["id"].as_str().unwrap_or("").to_string();
                match groups.iter_mut().find(|g| g.0 == id) {
                    Some(g) => g.1.push(a),
                    None => groups.push((id, vec![a])),
                }
            }
            // who's on a roll first, then what they've been up to
            let items: Vec<gpui::AnyElement> = groups.into_iter().take(16).enumerate().map(|(i, (_, acts))| feed_card(i, &acts)).collect();
            crew_rows.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().px(px(theme::GUTTER)).text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).child("the crew lately"))
                    .child(crate::ui::rows::scroller("row-feed", div().id("row-feed").overflow_x_scroll().flex().gap(px(10.)).px(px(theme::GUTTER)).children(items)))
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
            // only rows on (or near) the screen are drawn
            let key = format!("{}:{}", self.scope, r.key);
            below = below.child(crate::ui::lazy::section(key, &self.scroll, vh, || {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(row_head(&r.key, &r.title, r.spec.clone()))
                    .child(crate::ui::rows::scroller(SharedString::from(format!("row-{}", r.key)), div()
                            .id(SharedString::from(format!("row-{}", r.key)))
                            .overflow_x_scroll()
                            .flex()
                            .gap(px(10.))
                            .px(px(theme::GUTTER))
                            .children(r.items.iter().enumerate().map(|(i, m)| {
                                let mm = m.clone();
                                let (p, sub) = r.extra.get(i).cloned().unwrap_or((None, None));
                                if r.key == "cw" {
                                    // its page, with Resume ready
                                    let clip = if m.id == card_owner { card_clip.clone() } else { None };
                                    card::card_with(SharedString::from(format!("c-{ri}-{i}")), m, p, sub, Some(clip), move |_, _, cx| Nav::detail(&mm, cx))
                                } else {
                                    let clip = if m.id == card_owner { card_clip.clone() } else { None };
                                    card::card_preview(SharedString::from(format!("c-{ri}-{i}")), m, clip, move |_, _, cx| Nav::detail(&mm, cx))
                                }
                            })),))
            }));
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
                let spec = views_catalog::Spec { title: name.clone(), service: None, catalog: c.clone(), genre: None, ..Default::default() };
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
                    .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label2()).child(label))
                    .child(div().text_size(px(18.)).font_weight(FontWeight::BLACK).child(name))
            });
            let genres = self.genres.1.iter().enumerate().map(|(i, gname)| {
                let spec = views_catalog::Spec { title: self.genres.0.as_ref().and_then(|c| c["name"].as_str()).unwrap_or("films").to_string(), service: None, catalog: self.genres.0.clone().unwrap_or_default(), genre: Some(gname.clone()), ..Default::default() };
                div()
                    .id(("genre", i))
                    .flex_none()
                    .px(px(18.))
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .bg(gpui::rgb(tints[(i + 3) % tints.len()]))
                    .font_weight(FontWeight::SEMIBOLD)
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
                    .child(div().px(px(theme::GUTTER)).text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).child("browse"))
                    .child(crate::ui::rows::scroller("browse-tiles", div()
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
                            .children(tiles),))
                    .child(crate::ui::rows::scroller("browse-genres", div().id("browse-genres").overflow_x_scroll().flex().gap(px(8.)).px(px(theme::GUTTER)).children(genres))),
            );
        }
        // streaming services
        if !self.services.is_empty() {
            below = below.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().px(px(theme::GUTTER)).text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).child("services"))
                    .child(crate::ui::rows::scroller("services", div().id("services").overflow_x_scroll().flex().gap(px(10.)).px(px(theme::GUTTER)).children(self.services.iter().map(|(id, name, logo)| {
                        let spec = views_catalog::Spec { title: name.clone(), service: Some((*id, logo.clone())), catalog: Value::Null, genre: None, ..Default::default() };
                        div()
                            .id(SharedString::from(format!("svc-{id}")))
                            .flex_none()
                            .w(px(180.))
                            .h(px(96.))
                            .rounded(px(16.))
                            .bg(theme::white(0.06))
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
                    })))),
            );
        }
        let _ = cx;
        page.child(below)
    }
}

/// A row's title; with somewhere to go it opens the whole list, and a little
/// › slides in beside it on hover.
fn row_head(key: &str, title: &str, spec: Option<views_catalog::Spec>) -> gpui::AnyElement {
    let t = div().text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::text()).child(title.to_string());
    let Some(spec) = spec else {
        return div().px(px(theme::GUTTER)).child(t).into_any_element();
    };
    let group = SharedString::from(format!("rh-{key}"));
    div()
        .px(px(theme::GUTTER))
        .flex()
        .child(
            div()
                .id(SharedString::from(format!("rhead-{key}")))
                .group(group.clone())
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .on_click({
                    let following = key == "follow";
                    move |_, _, cx| Nav::go(if following { crate::Route::Following } else { crate::Route::Catalog(spec.clone()) }, cx)
                })
                .child(t)
                .child(
                    div()
                        .opacity(0.)
                        .ml(px(-6.))
                        .text_color(theme::label2())
                        .group_hover(group, |s| s.opacity(1.).ml(px(0.)))
                        .child(crate::ui::icon("chevronWide", 20.)),
                ),
        )
        .into_any_element()
}

/// a crew member's face (soft circle), or their initial
fn face(a: &Value, size: f32) -> gpui::AnyElement {
    let name = a["name"].as_str().unwrap_or("?");
    match a["avatar"].as_str() {
        Some(u) => gpui::img(crate::net::face(u, size)).size(px(size)).flex_none().into_any_element(),
        None => div().size(px(size)).flex_none().rounded_full().bg(theme::neon()).flex().items_center().justify_center().text_size(px(size * 0.45)).font_weight(FontWeight::BLACK).child(name.chars().next().unwrap_or('?').to_uppercase().to_string()).into_any_element(),
    }
}

fn score_color(r: i64) -> gpui::Hsla {
    match r {
        8.. => theme::lamp().into(),
        5.. => theme::text().into(),
        _ => theme::neon().into(),
    }
}

fn feed_card(i: usize, acts: &[&Value]) -> gpui::AnyElement {
    let first = acts[0];
    let t = &first["title"];
    let m = Meta { id: t["id"].as_str().unwrap_or("").into(), kind: t["type"].as_str().unwrap_or("").into(), name: t["name"].as_str().unwrap_or("").into(), poster: t["poster"].as_str().map(String::from), background: t["background"].as_str().map(String::from), logo: t["logo"].as_str().map(String::from), ..Default::default() };
    // no logo saved with it: metahub's (TMDB's behind it) by the IMDb id
    let mut m = m;
    if m.logo.is_none() && m.id.starts_with("tt") {
        m.logo = Some(format!("https://images.metahub.space/logo/medium/{}/img", m.id.split(':').next().unwrap_or(&m.id)));
    }
    let when = acts.iter().filter_map(|a| a["at"].as_f64()).fold(0f64, f64::max);
    let when = if when > 0. { ago(when) } else { String::new() };
    let mm = m.clone();
    let pre = m.clone();
    let review = acts.iter().find(|a| a["text"].as_str().map_or(false, |t| !t.is_empty())).copied();
    let base = div().id(("feed", i)).flex_none().relative().rounded(px(16.)).overflow_hidden().bg(theme::surface2()).cursor_pointer().on_hover(move |on, _, cx| {
        if *on {
            crate::views::detail::prefetch(&pre, cx);
        }
    }).on_click(move |_, _, cx| Nav::detail(&mm, cx));
    // everyone else on this title: little faces with their scores
    let others = |skip: Option<&Value>| {
        div().flex().gap(px(10.)).children(acts.iter().filter(|a| skip.map_or(true, |s| !std::ptr::eq(**a, s))).take(4).map(|a| {
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .child(face(a, 22.))
                .children(a["rating"].as_i64().map(|r| div().text_size(px(13.)).font_weight(FontWeight::BLACK).text_color(score_color(r)).child(r.to_string())))
                .when(a["kind"].as_str() == Some("finished") && a["rating"].is_null(), |d| d.child(crate::ui::icon("check", 14.).text_color(theme::label2())))
        }))
    };
    match review {
        // a review: the words are the card
        Some(r) => {
            let art = m.background.clone().or(m.poster.clone()).map(|u| crate::net::fit(&crate::api::sized(&u, "w780"), 420.));
            let quote = r["text"].as_str().unwrap_or("").to_string();
            let who = r["name"].as_str().unwrap_or("someone").to_string();
            let wid = r["who"].as_str().unwrap_or("").to_string();
            base.w(px(420.))
                .h(px(236.))
                .children(art.map(|u| gpui::img(u).absolute().top_0().left_0().size_full().object_fit(gpui::ObjectFit::Cover).opacity(0.55)))
                .child(div().absolute().top_0().left_0().size_full().bg(gpui::linear_gradient(180., gpui::linear_color_stop(theme::black(0.35), 0.), gpui::linear_color_stop(theme::black(0.92), 0.75))))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .p(px(18.))
                        .flex()
                        .flex_col()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(match m.logo.clone() {
                                    Some(u) => {
                                        let name = m.name.clone();
                                        gpui::img(crate::net::logo(&crate::api::sized(&u, "w500"), 150.))
                                            .h(px(34.))
                                            .max_w(px(150.))
                                            .object_fit(gpui::ObjectFit::Contain)
                                            .with_fallback(move || div().w(px(150.)).text_size(px(15.)).font_weight(FontWeight::BLACK).truncate().child(name.clone()).into_any_element())
                                            .into_any_element()
                                    }
                                    None => div().text_size(px(15.)).font_weight(FontWeight::BLACK).max_w(px(260.)).truncate().child(m.name.clone()).into_any_element(),
                                })
                                .child(div().text_size(px(12.)).text_color(theme::label3()).child(when)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(12.))
                                .child(div().text_size(px(19.)).line_height(px(25.)).font_weight(FontWeight::SEMIBOLD).line_clamp(3).text_ellipsis().child(crate::ui::emoji::text(&format!("“{quote}”"), 19.)))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .id(("feed-who", i))
                                                .flex()
                                                .items_center()
                                                .gap(px(8.))
                                                .on_click(move |_, _, cx| {
                                                    cx.stop_propagation();
                                                    Nav::go(crate::Route::Member(wid.clone()), cx)
                                                })
                                                .child(face(r, 26.))
                                                .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).hover(|s| s.underline()).child(crate::ui::emoji::text(&who, 14.)))
                                                .children(r["rating"].as_i64().map(|n| div().text_size(px(14.)).font_weight(FontWeight::BLACK).text_color(score_color(n)).child(format!("★ {n}")))),
                                        )
                                        .child(others(Some(r))),
                                ),
                        ),
                )
                .into_any_element()
        }
        // just scores: the poster, with the crew's numbers over its foot
        None => {
            let poster = m.poster.clone().map(|u| crate::net::fit(&crate::api::sized(&u, "w342"), 156.));
            let single = acts.len() == 1;
            let a = first;
            let line = if single {
                match (a["kind"].as_str(), a["rating"].as_i64()) {
                    (_, Some(_)) => format!("{} rated it", a["name"].as_str().unwrap_or("someone")),
                    (Some("finished"), None) => format!("{} finished it", a["name"].as_str().unwrap_or("someone")),
                    _ => a["name"].as_str().unwrap_or("someone").to_string(),
                }
            } else {
                format!("{} of the crew", acts.len())
            };
            base.w(px(156.))
                .h(px(236.))
                .children(poster.map(|u| gpui::img(u).absolute().top_0().left_0().size_full().object_fit(gpui::ObjectFit::Cover)))
                .child(div().absolute().left_0().bottom_0().w_full().h(px(140.)).bg(gpui::linear_gradient(180., gpui::linear_color_stop(theme::black(0.0), 0.), gpui::linear_color_stop(theme::black(0.95), 0.8))))
                .child(
                    div()
                        .absolute()
                        .left(px(10.))
                        .right(px(10.))
                        .bottom(px(10.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .when(single, |d| {
                            d.child(
                                div()
                                    .flex()
                                    .items_end()
                                    .justify_between()
                                    .child(face(a, 26.))
                                    .children(a["rating"].as_i64().map(|r| div().flex().items_end().child(div().text_size(px(34.)).line_height(px(34.)).font_weight(FontWeight::BLACK).text_color(score_color(r)).child(r.to_string())).child(div().text_size(px(13.)).text_color(theme::label3()).mb(px(3.)).child("/10"))))
                                    .when(a["rating"].is_null(), |d| d.child(crate::ui::icon("check", 22.).text_color(theme::lamp()))),
                            )
                        })
                        .when(!single, |d| d.child(others(None)))
                        .child(div().flex().justify_between().text_size(px(11.)).text_color(theme::label2()).child(div().truncate().child(crate::ui::emoji::text(&line, 11.))).child(div().flex_none().text_color(theme::label3()).child(when))),
                )
                .into_any_element()
        }
    }
}


/// "because you watched X": recommendations for your two latest finishes,
/// without anything you've already seen or started
async fn because_you_watched() -> Vec<Row> {
    let simkl = api::call::<Value>("my_lists", json!({})).await.unwrap_or_default();
    let (done, mut seen): (Vec<Meta>, std::collections::HashSet<String>) = if simkl["simkl"].as_bool() == Some(true) {
        let list = |k: &str| -> Vec<Meta> { simkl[k].as_array().into_iter().flatten().filter_map(|m| serde_json::from_value::<Meta>(m.clone()).ok()).filter(|m| !m.id.is_empty()).collect() };
        let done = list("completed");
        let seen = done.iter().chain(list("watching").iter()).map(|m| m.id.clone()).collect();
        (done, seen)
    } else {
        let mut items = api::call::<Vec<Value>>("library_list", json!({})).await.unwrap_or_default();
        items.sort_by(|a, b| b["state"]["lastWatched"].as_str().unwrap_or("").cmp(a["state"]["lastWatched"].as_str().unwrap_or("")));
        let seen = items.iter().filter_map(|i| i["_id"].as_str().map(String::from)).collect();
        let done = items
            .iter()
            .filter(|i| i["state"]["timesWatched"].as_i64().unwrap_or(0) > 0 || crate::api::flag(&i["state"]["flaggedWatched"]))
            .map(|i| Meta { id: i["_id"].as_str().unwrap_or("").into(), kind: i["type"].as_str().unwrap_or("").into(), name: i["name"].as_str().unwrap_or("").into(), ..Default::default() })
            .filter(|m| !m.id.is_empty())
            .collect();
        (done, seen)
    };
    // what you loved (a 10, or a favourite) and liked (7 to 9), newest first
    let lib = api::call::<Vec<Value>>("library_list", json!({})).await.unwrap_or_default();
    let known: std::collections::HashMap<String, Meta> = lib
        .iter()
        .filter_map(|i| {
            let id = i["_id"].as_str()?.to_string();
            Some((id.clone(), Meta { id, kind: i["type"].as_str().unwrap_or("").into(), name: i["name"].as_str().unwrap_or("").into(), ..Default::default() }))
        })
        .collect();
    let ratings = couch::commands::local_ratings(couch::state::get());
    let mut rated: Vec<(String, i64, u64)> = ratings
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| Some((k.clone(), v["rating"].as_i64()?, v["at"].as_u64().unwrap_or(0))))
        .collect();
    rated.sort_by(|a, b| b.2.cmp(&a.2));
    let favs: Vec<Meta> = api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/favs", "body": Value::Null }))
        .await
        .ok()
        .and_then(|v| v["favs"].as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|f| serde_json::from_value::<Meta>(f).ok())
        .filter(|m| !m.id.is_empty() && !m.name.is_empty())
        .collect();
    let loved: Vec<Meta> = rated.iter().filter(|r| r.1 >= 10).filter_map(|r| known.get(&r.0).cloned()).chain(favs).collect();
    let liked: Vec<Meta> = rated.iter().filter(|r| (7..=9).contains(&r.1)).filter_map(|r| known.get(&r.0).cloned()).collect();

    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut picks: Vec<(Meta, &'static str)> = vec![];
    for (list, why, n) in [(loved, "because you loved", 1), (liked, "because you liked", 1), (done, "because you watched", 2)] {
        let mut took = 0;
        for m in list {
            if took >= n || m.name.is_empty() || !used.insert(m.id.clone()) {
                continue;
            }
            picks.push((m, why));
            took += 1;
        }
    }
    let mut rows = vec![];
    for (m, why) in picks {
        let ex = api::call::<Value>("meta_enrich", json!({ "kind": m.kind, "id": m.id })).await.unwrap_or_default();
        let recs: Vec<Meta> = ex["recommendations"].as_array().into_iter().flatten().filter_map(|r| serde_json::from_value::<Meta>(r.clone()).ok()).take(18).collect();
        // TMDB's ids to the real ones (so they open, show logos, and can be checked against what you've seen)
        let jobs = recs.into_iter().map(|r| async move {
            if !r.id.starts_with("tmdb:") {
                return Some(r);
            }
            let v = api::call::<Value>("tmdb_resolve", json!({ "kind": r.kind, "id": r.id })).await.ok()?;
            let mut r = r;
            r.id = v["id"].as_str()?.to_string();
            r.kind = v["type"].as_str().unwrap_or(&r.kind).to_string();
            Some(r)
        });
        let items: Vec<Meta> = futures::future::join_all(jobs).await.into_iter().flatten().filter(|r| !seen.contains(&r.id)).collect();
        for r in &items {
            seen.insert(r.id.clone());
        }
        if items.len() >= 4 {
            let title = format!("{why} {}", m.name);
            let spec = views_catalog::Spec { title: title.clone(), fixed: Some(items.clone()), ..Default::default() };
            rows.push(Row { key: format!("byw-{}", m.id), title, items, extra: vec![], spec: Some(spec) });
        }
    }
    rows
}


/// Continue watching, with "up next" added: for each show whose last episode
/// you finished (recently), the next one that's out and unwatched, placed by
/// when you watched. (Shows a half-watched episode is already in the row for.)
async fn with_next_up(items: Vec<Value>) -> Vec<Value> {
    let have: std::collections::HashSet<String> = items.iter().filter_map(|i| i["_id"].as_str().or(i["id"].as_str()).map(|x| x.split(':').next().unwrap_or(x).to_string())).collect();
    let lib = api::call::<Vec<Value>>("library_list", json!({})).await.unwrap_or_default();
    let th = couch::state::get().core.settings().playback.watched_threshold as f64 / 100.;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let cands: Vec<Value> = lib
        .into_iter()
        .filter(|i| i["type"] == "series" && i["removed"].as_bool() != Some(true) && i["state"]["dismissed"].as_bool() != Some(true))
        .filter(|i| i["_id"].as_str().map_or(false, |id| !have.contains(id)))
        .filter(|i| i["state"]["video_id"].as_str().map_or(false, |v| v.split(':').count() == 3))
        .filter(|i| {
            // finished the episode it was on (back at 0, or past the watched mark)
            let (d, t) = (i["state"]["duration"].as_f64().unwrap_or(0.), i["state"]["timeOffset"].as_f64().unwrap_or(0.));
            t <= 0. || (d > 0. && t >= d * th)
        })
        // watched in the last two months
        .filter(|i| now - couch::library::parse_iso(i["state"]["lastWatched"].as_str().unwrap_or("")) < 60 * 86_400_000)
        .collect();
    let today = couch::integrations::tmdb::today_str();
    let found = futures::future::join_all(cands.into_iter().take(12).map(|i| {
        let today = today.clone();
        async move {
            let id = i["_id"].as_str()?.to_string();
            let full = api::call::<Value>("meta_get", json!({ "kind": "series", "id": id, "hint": Value::Null })).await.ok()?;
            let eps = crate::playback::ordered(&full);
            let cur = i["state"]["video_id"].as_str()?;
            let at = eps.iter().position(|v| v["id"].as_str() == Some(cur))?;
            let seen: Vec<String> = i["state"]["watchedIds"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            let next = eps[at + 1..].iter().find(|v| {
                v["season"].as_i64().unwrap_or(0) > 0
                    && v["id"].as_str().map_or(false, |x| !seen.iter().any(|s| s == x))
                    && v["released"].as_str().filter(|r| !r.is_empty()).map_or(false, |r| &r[..r.len().min(10)] <= today.as_str())
            })?;
            let mut e = i.clone();
            // out in the last ten days: a new episode, first in the row
            let rel = next["released"].as_str().unwrap_or("");
            if !rel.is_empty() && &rel[..rel.len().min(10)] >= couch::integrations::tmdb::day_str(-10).as_str() {
                e["_new"] = json!(true);
            }
            e["state"]["video_id"] = next["id"].clone();
            e["state"]["timeOffset"] = json!(0);
            e["state"]["duration"] = json!(0);
            e["_next"] = json!(true);
            Some(e)
        }
    }))
    .await;
    let mut all: Vec<Value> = items.into_iter().chain(found.into_iter().flatten()).collect();
    // new episodes first, then what you watched last
    all.sort_by_key(|i| (i["_new"].as_bool() != Some(true), -couch::library::parse_iso(i["state"]["lastWatched"].as_str().unwrap_or(""))));
    // and a few from your list you haven't started, at the end
    let have: std::collections::HashSet<String> = all.iter().filter_map(|i| i["_id"].as_str().or(i["id"].as_str()).map(String::from)).collect();
    let lists = api::call::<Value>("my_lists", json!({})).await.unwrap_or_default();
    let plan: Vec<Value> = lists["plan"].as_array().cloned().unwrap_or_default();
    for m in plan.into_iter().filter(|m| m["id"].as_str().map_or(false, |id| !have.contains(id))).take(4) {
        let mut e = m.clone();
        e["_id"] = m["id"].clone();
        e["_plan"] = json!(true);
        all.push(e);
    }
    all
}

/// The continue-watching row: what you're in the middle of, plus up-next episodes.
async fn cw_row() -> Option<(Vec<Meta>, Vec<(Option<f32>, Option<String>)>)> {
    let items = api::call::<Vec<Value>>("continue_watching", json!({})).await.ok()?;
                // plus the next episode of shows you finished an episode of, in
                // the order you watched them
                let items = with_next_up(items).await;
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
                    let ep = if parts.len() == 3 { format!("S{} E{}", parts[1], parts[2]) } else { String::new() };
                    // what's left, like "2h 43m left"
                    let left = (d > 0.).then(|| ((d - st["timeOffset"].as_f64().unwrap_or(0.)).max(0.) / 60000.).round() as i64).map(|m| match m {
                        0 => "almost done".to_string(),
                        m if m >= 60 => format!("{}h {}m left", m / 60, m % 60),
                        m => format!("{m}m left"),
                    });
                    let left = if v["_new"].as_bool() == Some(true) {
                        Some("new episode".to_string())
                    } else if v["_next"].as_bool() == Some(true) {
                        Some("up next".to_string())
                    } else if v["_plan"].as_bool() == Some(true) {
                        Some("on your list".to_string())
                    } else {
                        left
                    };
                    let sub = (!ep.is_empty() || left.is_some()).then(|| format!("{ep}\t{}", left.unwrap_or_default()));
                    metas.push(m);
                    extra.push((p, sub));
                }
    Some((metas, extra))
}

impl Home {
    fn set_cw(&mut self, metas: Vec<Meta>, extra: Vec<(Option<f32>, Option<String>)>, cx: &mut Context<Self>) {
        self.rows.retain(|r| r.key != "cw");
        if !metas.is_empty() {
            let spec = views_catalog::Spec { title: "up next".into(), fixed: Some(metas.clone()), ..Default::default() };
            self.rows.insert(0, Row { key: "cw".into(), title: "up next".into(), items: metas, extra, spec: Some(spec) });
        }
        cx.notify();
    }
    /// Back on Home (after watching something): the row shows where you stopped.
    /// Once now, and again shortly (closing the player saves a moment later).
    pub fn reload_cw(&mut self, cx: &mut Context<Self>) {
        if self.scope != "all" {
            return;
        }
        cx.spawn(async move |this, cx| {
            for wait in [0u64, 1500, 4000] {
                crate::timer::after(std::time::Duration::from_millis(wait)).await;
                if let Some((metas, extra)) = cw_row().await {
                    if this.update(cx, |v, cx| v.set_cw(metas, extra, cx)).is_err() {
                        break;
                    }
                }
            }
        })
        .detach();
    }
}
