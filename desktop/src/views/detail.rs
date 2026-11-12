//! A title's page: the billboard-style top (logo, facts, Play/Resume,
//! My List, Mark watched, the blurb), the episodes by season, the cast,
//! and more like it.
use crate::api::{self, sized, Meta};
use crate::playback;
use crate::theme;
use crate::ui::{card, icon, icon_fill};
use crate::Nav;
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, Context, FontWeight, ObjectFit, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

pub struct Detail {
    pub base: Meta,
    full: Option<Value>,
    extra: Option<Value>,
    item: Option<Value>,
    season: Option<i64>,
    rating: Option<u8>,
    hover_star: u8,
    favs: Option<Vec<Value>>,
    /// the trailer's YouTube key (None: not looked up, Some(""): there isn't one)
    trailer: Option<String>,
    scroll: ScrollHandle,
}

impl Detail {
    pub fn new(base: Meta, cx: &mut Context<Self>) -> Self {
        let (kind, id) = (base.kind.clone(), base.id.clone());
        cx.spawn(async move |this, cx| {
            let (full, extra) = futures::join!(
                api::call::<Value>("meta_get", json!({ "kind": kind, "id": id, "hint": Value::Null })),
                api::call::<Value>("meta_enrich", json!({ "kind": kind, "id": id })),
            );
            let lid = full.as_ref().ok().and_then(|f| f["id"].as_str().map(|s| s.to_string())).unwrap_or(id.clone());
            let item = api::call::<Value>("library_item", json!({ "id": lid })).await.ok();
            let favs = api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/favs", "body": Value::Null })).await.ok().and_then(|v| v["favs"].as_array().cloned());
            let rating = api::call::<Value>("simkl_item_status", json!({ "kind": kind, "id": lid })).await.ok().and_then(|v| v["rating"].as_u64()).map(|r| r as u8);
            let _ = this.update(cx, |d, cx| {
                d.full = full.ok();
                d.extra = extra.ok();
                d.item = item.filter(|v| !v.is_null());
                d.rating = rating;
                d.favs = favs;
                // open on the season you're in
                if let Some(f) = &d.full {
                    let at = d.item.as_ref().and_then(|i| i["state"]["season"].as_i64());
                    let seasons = seasons(f);
                    d.season = at.filter(|s| seasons.contains(s)).or_else(|| seasons.iter().copied().find(|s| *s > 0)).or(seasons.first().copied());
                }
                cx.notify();
            });
        })
        .detach();
        cx.observe(&crate::ui::video::get(cx), |_, _, cx| cx.notify()).detach();
        let (k2, i2) = (base.kind.clone(), base.id.clone());
        cx.spawn(async move |this, cx| {
            let key = api::call::<Option<String>>("trailer_get", json!({ "kind": k2, "id": i2 })).await.ok().flatten().unwrap_or_default();
            let _ = this.update(cx, |d, cx| {
                d.trailer = Some(key);
                cx.notify();
            });
        })
        .detach();
        Self { base, full: None, extra: None, item: None, season: None, rating: None, hover_star: 0, favs: None, trailer: None, scroll: ScrollHandle::new() }
    }

    fn refresh_item(&mut self, cx: &mut Context<Self>) {
        let id = self.lid();
        cx.spawn(async move |this, cx| {
            let item = api::call::<Value>("library_item", json!({ "id": id })).await.ok();
            let _ = this.update(cx, |d, cx| {
                d.item = item.filter(|v| !v.is_null());
                cx.notify();
            });
        })
        .detach();
    }

    fn lid(&self) -> String {
        self.full.as_ref().and_then(|f| f["id"].as_str().map(|s| s.to_string())).unwrap_or(self.base.id.clone())
    }

    fn meta(&self) -> Meta {
        let mut m = self.full.as_ref().and_then(|f| serde_json::from_value::<Meta>(f.clone()).ok()).unwrap_or(self.base.clone());
        // the art the page opened with stays put
        m.background = self.base.background.clone().or(m.background).or(self.extra.as_ref().and_then(|e| e["backdrop"].as_str().map(|s| s.to_string())));
        m.logo = self.base.logo.clone().or(m.logo).or(self.extra.as_ref().and_then(|e| e["logo"].as_str().map(|s| s.to_string())));
        m
    }

    fn watched(&self) -> Vec<String> {
        self.item.as_ref().and_then(|i| serde_json::from_value(i["state"]["watchedIds"].clone()).ok()).unwrap_or_default()
    }

    fn progress(&self, video: Option<&str>) -> f64 {
        let Some(i) = &self.item else { return 0. };
        let st = &i["state"];
        let d = st["duration"].as_f64().unwrap_or(0.);
        if d <= 0. {
            return 0.;
        }
        if let Some(v) = video {
            if st["video_id"].as_str() != Some(v) {
                return 0.;
            }
        }
        (st["timeOffset"].as_f64().unwrap_or(0.) / d).min(1.)
    }

    /// What the big button does: resume, next unwatched episode, or start.
    fn primary(&self) -> (String, Option<Value>, String) {
        let Some(f) = &self.full else { return ("Play".into(), None, String::new()) };
        let eps: Vec<Value> = playback::ordered(f);
        if f["type"] != "series" || eps.is_empty() {
            return (if self.progress(None) > 0.02 { "Resume" } else { "Play" }.into(), None, String::new());
        }
        let label = |v: &Value| format!("S{} E{}", v["season"].as_i64().unwrap_or(0), v["episode"].as_i64().or(v["number"].as_i64()).unwrap_or(0));
        if let Some(vid) = self.item.as_ref().and_then(|i| i["state"]["video_id"].as_str()) {
            if self.progress(Some(vid)) > 0.02 {
                if let Some(v) = eps.iter().find(|x| x["id"].as_str() == Some(vid)) {
                    return ("Resume".into(), Some(v.clone()), label(v));
                }
            }
        }
        let watched = self.watched();
        let now = chrono_now();
        let real: Vec<&Value> = eps.iter().filter(|v| v["season"].as_i64().unwrap_or(0) > 0).collect();
        let next = real.iter().find(|v| !watched.iter().any(|w| Some(w.as_str()) == v["id"].as_str()) && released(v, &now)).copied();
        let v = next.or(real.first().copied()).or(eps.first()).cloned();
        let cont = next.is_some() && next.map(|n| n["id"].clone()) != real.first().map(|n| n["id"].clone());
        match v {
            Some(v) => (if cont { "Continue" } else { "Play" }.into(), Some(v.clone()), label(&v)),
            None => ("Play".into(), None, String::new()),
        }
    }
}

fn chrono_now() -> String {
    couch::integrations::tmdb::today_str()
}
fn released(v: &Value, today: &str) -> bool {
    v["released"].as_str().map_or(true, |r| &r[..r.len().min(10)] <= today)
}
fn seasons(f: &Value) -> Vec<i64> {
    let mut s: Vec<i64> = playback::ordered(f).iter().map(|v| v["season"].as_i64().unwrap_or(0)).collect();
    s.dedup();
    let mut u: Vec<i64> = vec![];
    for x in s {
        if !u.contains(&x) {
            u.push(x);
        }
    }
    u.sort_by_key(|x| if *x == 0 { 9999 } else { *x });
    u
}

impl Detail {
    fn stars(&self, m: &Meta, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = if self.hover_star > 0 { self.hover_star } else { self.rating.unwrap_or(0) };
        div()
            .id("stars")
            .flex()
            .items_center()
            .gap(px(4.))
            .on_hover(cx.listener(|d, on: &bool, _, cx| {
                if !*on {
                    d.hover_star = 0;
                    cx.notify();
                }
            }))
            .child(div().mr(px(8.)).text_size(px(14.)).text_color(theme::label3()).child("your rating"))
            .children((1..=10u8).map(|n| {
                let (kind, id, name) = (m.kind.clone(), self.lid(), m.name.clone());
                let title = json!({ "id": m.id, "type": m.kind, "name": m.name, "poster": m.poster, "background": m.background });
                div()
                    .id(SharedString::from(format!("star-{n}")))
                    .cursor_pointer()
                    .on_hover(cx.listener(move |d, on: &bool, _, cx| {
                        if *on {
                            d.hover_star = n;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |d, _, _, cx| {
                        let v = if d.rating == Some(n) { None } else { Some(n) };
                        d.rating = v;
                        cx.notify();
                        let (kind, id, name, title) = (kind.clone(), id.clone(), name.clone(), title.clone());
                        cx.spawn(async move |_, cx| {
                            let r = api::call::<Value>("simkl_rate", json!({ "kind": kind, "id": id, "rating": v })).await;
                            if r.is_ok() && v.is_some() {
                                let _ = api::call::<Value>("crew_social", json!({ "method": "POST", "path": "/v1/activity", "body": { "kind": "rating", "title": title, "rating": v } })).await;
                            }
                            let _ = cx.update(|cx| Nav::toast(match (&r, v) {
                                (Err(e), _) => e.clone(),
                                (_, Some(n)) => format!("rated {name} {n}/10"),
                                _ => "rating removed".into(),
                            }, cx));
                        })
                        .detach();
                    }))
                    .child(if shown >= n { icon_fill("star", 20.).text_color(theme::lamp()) } else { icon("star", 20.).text_color(theme::label3()) })
            }))
    }
}

fn act(id: &'static str, ic: impl IntoElement, label: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_col()
        .items_center()
        .gap(px(6.))
        .w(px(84.))
        .cursor_pointer()
        .child(
            div()
                .size(px(48.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme::white(0.12))
                .border_1()
                .border_color(theme::white(0.1))
                .hover(|s| s.bg(theme::white(0.2)))
                .child(ic),
        )
        .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(theme::label2()).child(label.to_string()))
}

impl Render for Detail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vh = window.viewport_size().height;
        let m = self.meta();
        let g = theme::GUTTER;
        let art = m.background.clone().or(m.poster.clone()).map(|u| sized(&u, "original"));
        let logo = m.logo.clone().map(|u| sized(&u, "large"));
        let ex = self.extra.clone().unwrap_or(Value::Null);
        let full = self.full.clone();

        // Show • Thriller • 2017 • 2 Seasons • 16+
        let mut facts: Vec<String> = vec![if m.kind == "series" { "Show".into() } else { "Movie".into() }];
        if let Some(gn) = ex["genres"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str().map(|s| s.to_string()).or(v["name"].as_str().map(|s| s.to_string()))).or(m.genres.as_ref().and_then(|g| g.first().cloned())) {
            facts.push(gn);
        }
        if let Some(y) = m.year() {
            facts.push(y);
        }
        if let Some(f) = &full {
            if m.kind == "series" {
                let n = seasons(f).into_iter().filter(|s| *s > 0).count();
                if n > 0 {
                    facts.push(format!("{n} Season{}", if n > 1 { "s" } else { "" }));
                }
            } else if let Some(r) = f["runtime"].as_str() {
                facts.push(r.to_string());
            }
        }
        if let Some(c) = ex["certification"].as_str() {
            facts.push(c.to_string());
        }

        let (plabel, pvideo, psub) = self.primary();
        let flagged = self.item.as_ref().map_or(false, |i| i["state"]["flaggedWatched"].as_bool() == Some(true));
        let owned = self.item.as_ref().map_or(false, |i| i["removed"].as_bool() != Some(true) && i["temp"].as_bool() != Some(true));
        let name = m.name.clone();
        // rating only once you've actually watched it
        let seen_it = self.rating.is_some() || flagged || !self.watched().is_empty() || self.item.as_ref().map_or(false, |i| i["state"]["timesWatched"].as_i64().unwrap_or(0) > 0);

        // the trailer, playing over the art (with sound)
        let towner = format!("trailer:{}", self.base.id);
        let vid = crate::ui::video::get(cx);
        let trailer_on = vid.read(cx).owner == towner;
        let tframe = if trailer_on {
            let o = towner.clone();
            vid.update(cx, |x, _| {
                let f = x.frame(&o, window)?;
                let a = x.since.map_or(0., |t| (t.elapsed().as_secs_f32() / 0.6).min(1.));
                Some((f, a))
            })
        } else {
            None
        };
        if tframe.as_ref().map_or(false, |f| f.1 < 1.) {
            window.request_animation_frame();
        }
        let has_trailer = self.trailer.as_ref().map_or(false, |k| !k.is_empty());
        let hero = div()
            .relative()
            .w_full()
            .h(vh * 0.92)
            .flex_none()
            // the art, and the trailer over it once it's playing
            .children(match &tframe {
                Some((p, a)) => crate::ui::video::layers(art.clone(), p, *a, 0.),
                None => art.clone().map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).into_any_element()).into_iter().collect(),
            })
            .child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(90., linear_color_stop(theme::black(0.85), 0.), linear_color_stop(theme::black(0.0), 0.7))))
            .child(div().absolute().left_0().bottom_0().w_full().h(vh * 0.5).bg(linear_gradient(180., linear_color_stop(theme::black(0.0), 0.), linear_color_stop(theme::black(1.0), 1.))))
            .child(
                div()
                    .absolute()
                    .left(px(g))
                    .bottom(px(70.))
                    .w(px(640.))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(match logo {
                        Some(u) => img(u)
                            .h(px(160.))
                            .max_w(px(480.))
                            .object_fit(ObjectFit::Contain)
                            .with_fallback(move || div().text_size(px(60.)).font_weight(FontWeight::BLACK).child(name.clone()).into_any_element())
                            .into_any_element(),
                        None => div().text_size(px(60.)).line_height(px(62.)).font_weight(FontWeight::BLACK).child(m.name.clone()).into_any_element(),
                    })
                    .child(div().flex().gap(px(10.)).text_size(px(16.)).font_weight(FontWeight::MEDIUM).children(facts.into_iter().enumerate().flat_map(|(i, f)| {
                        let dot = (i > 0).then(|| div().text_color(theme::label3()).child("·").into_any_element());
                        dot.into_iter().chain(std::iter::once(div().child(f).into_any_element()))
                    })))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(18.))
                            .child(
                                div()
                                    .id("d-play")
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .h(px(54.))
                                    .px(px(28.))
                                    .rounded(px(14.))
                                    .bg(theme::text())
                                    .text_color(gpui::rgb(0x000000))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(18.))
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.88))
                                    .on_click({
                                        let m = m.clone();
                                        move |_, _, cx| match &pvideo {
                                            Some(v) => Nav::play_episode(&m, v["id"].as_str().unwrap_or("").to_string(), cx),
                                            None => Nav::play(&m, cx),
                                        }
                                    })
                                    .child(icon_fill("play", 20.).text_color(gpui::rgb(0x000000)))
                                    .child(plabel)
                                    .children((!psub.is_empty()).then(|| div().text_size(px(15.)).font_weight(FontWeight::MEDIUM).opacity(0.6).child(psub))),
                            )
                            .child(act("d-list", icon(if owned { "check" } else { "plus" }, 22.), "My List").on_click({
                                // My List, your couch lists, or a new one
                                let m = m.clone();
                                move |_, _, cx| Nav::add(&m, cx)
                            }))
                            .child({
                                let base = m.id.split(':').next().unwrap_or("").to_string();
                                let fav = self.favs.as_ref().map_or(false, |f| f.iter().any(|x| x["id"].as_str() == Some(base.as_str())));
                                let r = json!({ "id": base, "type": m.kind, "name": m.name, "poster": m.poster, "background": m.background });
                                act("d-fav", if fav { icon_fill("heart", 21.).text_color(theme::neon_soft()) } else { icon("heart", 21.) }, "Favourite").on_click(cx.listener(move |d, _, _, cx| {
                                    let mut list = d.favs.clone().unwrap_or_default();
                                    let id = r["id"].clone();
                                    let now = list.iter().any(|x| x["id"] == id);
                                    if now {
                                        list.retain(|x| x["id"] != id);
                                    } else {
                                        let mut e = r.clone();
                                        e["at"] = json!(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64);
                                        list.insert(0, e);
                                    }
                                    d.favs = Some(list.clone());
                                    cx.notify();
                                    cx.spawn(async move |_, cx| {
                                        let _ = api::call::<Value>("crew_social", json!({ "method": "PUT", "path": "/v1/favs", "body": { "favs": list } })).await;
                                        let _ = cx.update(|cx| Nav::toast(if now { "removed from favourites".into() } else { "added to favourites".into() }, cx));
                                    })
                                    .detach();
                                }))
                            })
                            .when(has_trailer, |d| {
                                let key = self.trailer.clone().unwrap_or_default();
                                let (o, v) = (towner.clone(), vid.clone());
                                d.child(act("d-trailer", icon(if trailer_on { "close" } else { "film" }, 22.), if trailer_on { "Stop trailer" } else { "Trailer" }).on_click(move |_, _, cx| {
                                    let o = o.clone();
                                    if trailer_on {
                                        v.update(cx, |x, cx| x.stop(&o, cx));
                                    } else {
                                        let url = format!("https://www.youtube.com/watch?v={key}");
                                        v.update(cx, |x, cx| x.play(&o, &url, cx));
                                    }
                                }))
                            })
                            .child(act("d-party", icon("couch", 22.), "Watch party").on_click({
                                let m = m.clone();
                                move |_, _, cx| Nav::plan_party(&m, cx)
                            }))
                            .child(act("d-seen", icon(if flagged { "checkCircle" } else { "eye" }, 22.), if flagged { "Watched" } else { "Mark watched" }).on_click(
                                cx.listener(move |d, _, _, cx| {
                                    let Some(f) = d.full.clone() else { return };
                                    let on = !flagged;
                                    let today = chrono_now();
                                    let all: Option<Vec<String>> = (f["type"] == "series").then(|| {
                                        playback::ordered(&f)
                                            .iter()
                                            .filter(|v| v["season"].as_i64().unwrap_or(0) > 0 && released(v, &today))
                                            .filter_map(|v| v["id"].as_str().map(|s| s.to_string()))
                                            .collect()
                                    });
                                    cx.spawn(async move |this, cx| {
                                        let _ = api::call::<Value>("library_set_watched", json!({ "meta": f, "videoId": Value::Null, "watched": on, "allVideos": all })).await;
                                        let _ = this.update(cx, |d, cx| d.refresh_item(cx));
                                    })
                                    .detach();
                                }),
                            )),
                    )
                    .children(ex["tagline"].as_str().map(|t| div().text_size(px(17.)).italic().text_color(theme::label2()).child(t.to_string())))
                    .children(m.description.clone().or(ex["overview"].as_str().map(|s| s.to_string())).map(|d| {
                        div().max_w(px(600.)).text_size(px(17.)).line_height(px(25.)).text_color(theme::white(0.88)).child(d)
                    }))
                    .when(seen_it, |d| d.child(self.stars(&m, cx))),
            );

        let mut page = div().id("detail").size_full().overflow_y_scroll().track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "detail")).child(hero);

        // ---------------------------------------------------------- episodes
        if let Some(f) = &full {
            let eps = playback::ordered(f);
            if !eps.is_empty() {
                let ss = seasons(f);
                let cur = self.season.unwrap_or(ss.first().copied().unwrap_or(1));
                let watched = self.watched();
                let today = chrono_now();
                let back_art = m.background.clone().map(|u| sized(&u, "w780"));
                page = page.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(14.))
                        .mt(px(10.))
                        .child(
                            div()
                                .px(px(g))
                                .flex()
                                .items_center()
                                .gap(px(16.))
                                .child(div().text_size(px(24.)).font_weight(FontWeight::BLACK).child("episodes"))
                                .child(div().id("seasons").flex().gap(px(8.)).overflow_x_scroll().children(ss.iter().map(|s| {
                                    let s = *s;
                                    let on = s == cur;
                                    div()
                                        .id(SharedString::from(format!("s{s}")))
                                        .px(px(14.))
                                        .h(px(34.))
                                        .flex()
                                        .items_center()
                                        .rounded_full()
                                        .text_size(px(15.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .cursor_pointer()
                                        .when(on, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
                                        .when(!on, |d| d.bg(theme::white(0.1)).hover(|s| s.bg(theme::white(0.18))))
                                        .on_click(cx.listener(move |d, _, _, cx| {
                                            d.season = Some(s);
                                            cx.notify();
                                        }))
                                        .child(if s == 0 { "Specials".to_string() } else { format!("Season {s}") })
                                }))),
                        )
                        .child(
                            div().id("eps").overflow_x_scroll().flex().gap(px(14.)).px(px(g)).children(
                                eps.iter().filter(|v| v["season"].as_i64().unwrap_or(0) == cur).map(|v| {
                                    let vid = v["id"].as_str().unwrap_or("").to_string();
                                    let n = v["episode"].as_i64().or(v["number"].as_i64()).unwrap_or(0);
                                    let seen = watched.contains(&vid);
                                    let future = !released(v, &today);
                                    let p = self.progress(Some(&vid));
                                    let imdb = api::imdb_episode(&vid);
                                    let thumb = v["thumbnail"].as_str().map(|s| s.to_string()).or(imdb.clone()).or(back_art.clone());
                                    let alt = imdb.or(back_art.clone());
                                    let mm = m.clone();
                                    let vid2 = vid.clone();
                                    let f2 = f.clone();
                                    div()
                                        .flex_none()
                                        .w(px(300.))
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.))
                                        .when(future, |d| d.opacity(0.45))
                                        .child(
                                            div()
                                                .id(SharedString::from(format!("ep-{vid}")))
                                                .relative()
                                                .w_full()
                                                .h(px(169.))
                                                .rounded(px(12.))
                                                .overflow_hidden()
                                                .bg(theme::surface2())
                                                .cursor_pointer()
                                                .border_1()
                                                .border_color(theme::white(0.))
                                                .hover(|s| s.border_color(theme::white(0.5)))
                                                .on_click(move |_, _, cx| {
                                                    if !future {
                                                        Nav::play_episode(&mm, vid.clone(), cx)
                                                    }
                                                })
                                                .children(thumb.map(|u| {
                                                    let alt = alt.clone().filter(|a| a != &u);
                                                    img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).with_fallback(move || match alt.clone() {
                                                        Some(a) => img(a).size_full().object_fit(ObjectFit::Cover).into_any_element(),
                                                        None => div().into_any_element(),
                                                    })
                                                }))
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .left(px(10.))
                                                        .bottom(px(8.))
                                                        .text_size(px(30.))
                                                        .font_weight(FontWeight::BLACK)
                                                        .text_color(theme::white(0.92))
                                                        .child(n.to_string()),
                                                )
                                                .when(p > 0.01, |d| {
                                                    d.child(div().absolute().left_0().bottom_0().w_full().h(px(4.)).bg(theme::white(0.25)).child(
                                                        div().h_full().w(px(300. * p as f32)).bg(theme::lamp()),
                                                    ))
                                                })
                                                // watched tick: always when watched, on hover otherwise
                                                .child(
                                                    div()
                                                        .id(SharedString::from(format!("tick-{vid2}")))
                                                        .absolute()
                                                        .top(px(8.))
                                                        .right(px(8.))
                                                        .size(px(28.))
                                                        .rounded_full()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .bg(gpui::rgb(0x000000))
                                                        .when(!seen, |d| d.opacity(0.).hover(|s| s.opacity(1.)))
                                                        .on_click(cx.listener(move |d, _, _, cx| {
                                                            cx.stop_propagation();
                                                            let (f, vid) = (f2.clone(), vid2.clone());
                                                            cx.spawn(async move |this, cx| {
                                                                let _ = api::call::<Value>("library_set_watched", json!({ "meta": f, "videoId": vid, "watched": !seen })).await;
                                                                let _ = this.update(cx, |d, cx| d.refresh_item(cx));
                                                            })
                                                            .detach();
                                                            let _ = d;
                                                        }))
                                                        .child(icon("check", 16.)),
                                                ),
                                        )
                                        .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).truncate().child(playback::episode_name(v)))
                                        .children(v["overview"].as_str().or(v["description"].as_str()).map(|o| {
                                            div().text_size(px(14.)).line_height(px(19.)).line_clamp(2).text_ellipsis().text_color(theme::label2()).child(o.to_string())
                                        }))
                                }),
                            ),
                        ),
                );
            }
        }

        // ----------------------------------------------------------- studios
        if let Some(cos) = ex["companies"].as_array().filter(|c| !c.is_empty()) {
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .mt(px(36.))
                    .child(div().px(px(g)).text_size(px(24.)).font_weight(FontWeight::BLACK).child("made by"))
                    .child(div().px(px(g)).flex().flex_wrap().gap(px(12.)).children(cos.iter().map(|c| {
                        let (cid, cname) = (c["id"].as_u64().unwrap_or(0), c["name"].as_str().unwrap_or("").to_string());
                        let logo = c["logo"].as_str().map(|s| s.to_string());
                        let n2 = cname.clone();
                        div()
                            .id(SharedString::from(format!("co-{cid}")))
                            .h(px(64.))
                            .px(px(18.))
                            .flex()
                            .items_center()
                            .rounded(px(14.))
                            .bg(if logo.is_some() { gpui::rgb(0xf4eaf0).into() } else { theme::white(0.08) })
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .on_click(move |_, _, cx| Nav::go(crate::Route::Studio(cid, n2.clone()), cx))
                            .child(match logo {
                                Some(u) => img(u).h(px(36.)).max_w(px(160.)).object_fit(ObjectFit::Contain).into_any_element(),
                                None => div().font_weight(FontWeight::SEMIBOLD).child(cname).into_any_element(),
                            })
                    }))),
            );
        }

        // -------------------------------------------------------------- cast
        if let Some(cast) = ex["cast"].as_array().filter(|c| !c.is_empty()) {
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .mt(px(36.))
                    .child(div().px(px(g)).text_size(px(24.)).font_weight(FontWeight::BLACK).child("cast"))
                    .child(div().id("cast").overflow_x_scroll().flex().gap(px(16.)).px(px(g)).children(cast.iter().take(20).map(|c| {
                        let photo = c["profile"].as_str().map(|s| s.to_string());
                        let nm = c["name"].as_str().unwrap_or("").to_string();
                        let (pn, pid) = (nm.clone(), c["id"].as_u64());
                        div()
                            .id(SharedString::from(format!("cast-{nm}")))
                            .cursor_pointer()
                            .on_click(move |_, _, cx| Nav::go(crate::Route::Person(pn.clone(), pid), cx))
                            .flex_none()
                            .w(px(110.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .size(px(96.))
                                    .rounded_full()
                                    .overflow_hidden()
                                    .bg(theme::surface2())
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(30.))
                                    .font_weight(FontWeight::BLACK)
                                    .text_color(theme::label3())
                                    .child(match photo {
                                        Some(u) => img(u).size_full().object_fit(ObjectFit::Cover).into_any_element(),
                                        None => div().child(nm.chars().next().unwrap_or('?').to_string()).into_any_element(),
                                    }),
                            )
                            .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_center().line_clamp(2).child(nm))
                            .children(c["character"].as_str().filter(|s| !s.is_empty()).map(|ch| {
                                div().text_size(px(12.)).text_center().line_clamp(1).text_ellipsis().text_color(theme::label3()).child(ch.to_string())
                            }))
                    }))),
            );
        }

        // -------------------------------------------------------- more like it
        if let Some(recs) = ex["recommendations"].as_array().filter(|c| !c.is_empty()) {
            let recs: Vec<Meta> = recs.iter().filter_map(|r| serde_json::from_value(r.clone()).ok()).collect();
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .mt(px(36.))
                    .child(div().px(px(g)).text_size(px(24.)).font_weight(FontWeight::BLACK).child("more like this"))
                    .child(div().id("recs").overflow_x_scroll().flex().gap(px(10.)).px(px(g)).children(recs.into_iter().enumerate().map(|(i, r)| {
                        let rr = r.clone();
                        card::card(SharedString::from(format!("rec-{i}")), &r, move |_, _, cx| open_rec(&rr, cx))
                    }))),
            );
        }
        page.child(div().h(px(90.)))
    }
}

/// TMDB recommendations carry tmdb: ids; find the real one first.
pub fn open_rec(r: &Meta, cx: &mut gpui::App) {
    if !r.id.starts_with("tmdb:") {
        return Nav::detail(r, cx);
    }
    let r = r.clone();
    cx.spawn(async move |cx| {
        let res = api::call::<Value>("tmdb_resolve", json!({ "kind": r.kind, "id": r.id })).await;
        let _ = cx.update(|cx| match res {
            Ok(v) => {
                let mut m = r.clone();
                m.id = v["id"].as_str().unwrap_or(&r.id).to_string();
                m.kind = v["type"].as_str().unwrap_or(&r.kind).to_string();
                Nav::detail(&m, cx)
            }
            Err(e) => Nav::toast(e, cx),
        });
    })
    .detach();
}
