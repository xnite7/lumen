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
    /// where the rating bar is on screen (left, width), for the mouse
    rate_box: std::rc::Rc<std::cell::Cell<(f32, f32)>>,
    /// the library change this page has caught up with
    lib_seen: u64,
    favs: Option<Vec<Value>>,
    /// the trailer's YouTube key (None: not looked up, Some(""): there isn't one)
    trailer: Option<String>,
    /// the trailer button: the page steps aside for it (when, on/off)
    theater: Option<(std::time::Instant, bool)>,
    /// the main studio: id, name, its logo (made white when it's too dark to read)
    studio: Option<(u64, String, Option<std::sync::Arc<gpui::Image>>, Option<String>)>,
    /// what the crew wrote about this title
    reviews: Vec<Value>,
    /// the "write a review" sheet
    writing: bool,
    review_text: gpui::Entity<crate::ui::input::TextInput>,
    scroll: ScrollHandle,
    /// more like this, with real (IMDb) ids so they get logos, clips, your marks
    recs: Option<Vec<Meta>>,
    recs_asked: bool,
    /// the title logo's colour (favourite, rating)
    tint: Option<gpui::Hsla>,
    /// when the rest of the page arrived (it eases in rather than popping)
    shown: Option<std::time::Instant>,
}

type Studio = (u64, String, Option<std::sync::Arc<gpui::Image>>, Option<String>);

/// everything a title page shows, fetched together
#[derive(Clone)]
pub struct Loaded {
    full: Option<Value>,
    extra: Option<Value>,
    item: Option<Value>,
    favs: Option<Vec<Value>>,
    rating: Option<u8>,
    studio: Option<Studio>,
    reviews: Vec<Value>,
}

/// bumped whenever My List / a list / watched changes, so an open title page re-reads its state
static LIB_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn library_changed() {
    LIB_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}
thread_local! {
    /// a trailer is filling the page (the top bar steps away)
    static THEATER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static TOP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub fn theater_on() -> bool {
    THEATER.with(|t| t.get())
}
/// the next title page opens at its top
pub fn to_top() {
    TOP.with(|t| t.set(true));
}
pub fn lib_gen() -> u64 {
    LIB_GEN.load(std::sync::atomic::Ordering::Relaxed)
}

thread_local! {
    static LOADED: std::cell::RefCell<std::collections::HashMap<String, Loaded>> = std::cell::RefCell::new(std::collections::HashMap::new());
    static FETCHING: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

/// Start fetching a title's page (on hovering its card) so opening it is instant.
pub fn prefetch(m: &Meta, cx: &mut gpui::App) {
    let id = m.id.clone();
    if id.is_empty() || LOADED.with(|c| c.borrow().contains_key(&id)) || !FETCHING.with(|f| f.borrow_mut().insert(id.clone())) {
        return;
    }
    let kind = m.kind.clone();
    cx.spawn(async move |_| {
        let l = load(kind, id.clone()).await;
        LOADED.with(|c| {
            let mut c = c.borrow_mut();
            if c.len() > 80 {
                c.clear();
            }
            c.insert(id.clone(), l);
        });
        FETCHING.with(|f| f.borrow_mut().remove(&id));
    })
    .detach();
}

/// all of it at once (not one after another)
async fn load(kind: String, id: String) -> Loaded {
    let rid = id.split(':').next().unwrap_or("").to_string();
    let (full, extra, favs, reviews) = futures::join!(
        api::call::<Value>("meta_get", json!({ "kind": kind, "id": id, "hint": Value::Null })),
        api::call::<Value>("meta_enrich", json!({ "kind": kind, "id": id })),
        api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/favs", "body": Value::Null })),
        api::call::<Value>("crew_social", json!({ "method": "GET", "path": format!("/v1/reviews/{}", urlencoding::encode(&rid)), "body": Value::Null })),
    );
    let lid = full.as_ref().ok().and_then(|f| f["id"].as_str().map(|s| s.to_string())).unwrap_or(id.clone());
    let studio_c = extra.as_ref().ok().and_then(|e| e["companies"].as_array()?.first().cloned());
    let studio_f = async move {
        let c = studio_c?;
        let (cid, cname, logo) = (c["id"].as_u64().unwrap_or(0), c["name"].as_str().unwrap_or("").to_string(), c["logo"].as_str().map(String::from));
        let pic = match &logo {
            Some(u) if !u.ends_with(".svg") => {
                let u = u.clone();
                couch::rt::spawn(async move {
                    let core = couch::state::get().core.clone();
                    couch::images::load(&core, &u).await.ok().and_then(|(b, _)| crate::ui::readable_logo(&b))
                })
                .await
                .ok()
                .flatten()
            }
            _ => None,
        };
        Some((cid, cname, pic, logo.filter(|u| u.ends_with(".svg"))))
    };
    let (item, rating, studio) = futures::join!(
        api::call::<Value>("library_item", json!({ "id": lid })),
        api::call::<Value>("simkl_item_status", json!({ "kind": kind, "id": lid })),
        studio_f,
    );
    Loaded {
        full: full.ok(),
        extra: extra.ok(),
        item: item.ok(),
        favs: favs.ok().and_then(|v| v["favs"].as_array().cloned()),
        rating: rating.ok().and_then(|v| v["rating"].as_u64()).map(|r| r as u8),
        studio,
        reviews: reviews.ok().map(|r| r["reviews"].as_array().cloned().unwrap_or_default()).unwrap_or_default(),
    }
}


impl Detail {
    pub fn new(base: Meta, cx: &mut Context<Self>) -> Self {
        let (kind, id) = (base.kind.clone(), base.id.clone());
        // fetched already (hovering its card starts that): the page is whole at once
        let ready = LOADED.with(|c| c.borrow().get(&id).cloned());
        if ready.is_none() {
            cx.spawn(async move |this, cx| {
                let l = load(kind, id.clone()).await;
                LOADED.with(|c| c.borrow_mut().insert(id, l.clone()));
                let _ = this.update(cx, |d, cx| {
                    d.fill(l);
                    d.shown = Some(std::time::Instant::now());
                    cx.notify();
                });
            })
            .detach();
        }
        cx.observe(&crate::ui::video::get(cx), |_, _, cx| cx.notify()).detach();
        let (k2, i2) = (base.kind.clone(), base.id.clone());
        cx.spawn(async move |this, cx| {
            let key = api::call::<Option<String>>("trailer_get", json!({ "kind": k2, "id": i2 })).await.ok().flatten().unwrap_or_default();
            let _ = this.update(cx, |d, cx| {
                d.trailer = Some(key.clone());
                cx.notify();
            });
        })
        .detach();
        // the logo's colour, for the heart and the rating
        if let Some(src) = base.logo.clone().or(base.background.clone()) {
            cx.spawn(async move |this, cx| {
                let t = couch::rt::spawn(async move {
                    let core = couch::state::get().core.clone();
                    let (b, _) = couch::images::load(&core, &sized(&src, "w500")).await.ok()?;
                    crate::views::player::tint_of(&b)
                })
                .await
                .ok()
                .flatten();
                let _ = this.update(cx, |d, cx| {
                    if let Some(t) = t {
                        crate::views::player::set_last_tint(t);
                    }
                    d.tint = t.map(|t| t[0]);
                    cx.notify();
                });
            })
            .detach();
        }
        let review_text = cx.new(|cx| crate::ui::input::TextInput::new("what did you think? (emoji welcome)", cx));
        cx.subscribe(&review_text, |d: &mut Self, _, _: &crate::ui::input::Submitted, cx| d.post_review(cx)).detach();
        let mut me = Self { theater: None, base, full: None, extra: None, item: None, season: None, rating: None, hover_star: 0, rate_box: Default::default(), lib_seen: LIB_GEN.load(std::sync::atomic::Ordering::Relaxed), favs: None, trailer: None, studio: None, reviews: vec![], writing: false, review_text, scroll: ScrollHandle::new(), shown: None, recs: None, recs_asked: false, tint: None };
        if let Some(l) = ready {
            me.fill(l);
            // fresh copy in the background (your progress, ratings, reviews may have moved)
            let (kind, id) = (me.base.kind.clone(), me.base.id.clone());
            cx.spawn(async move |this, cx| {
                let l = load(kind, id.clone()).await;
                LOADED.with(|c| c.borrow_mut().insert(id, l.clone()));
                let _ = this.update(cx, |d, cx| {
                    let season = d.season;
                    d.fill(l);
                    d.season = season.or(d.season);
                    cx.notify();
                });
            })
            .detach();
        }
        me
    }

    fn fill(&mut self, l: Loaded) {
        self.full = l.full;
        self.extra = l.extra;
        self.item = l.item.filter(|v| !v.is_null());
        self.rating = l.rating;
        self.favs = l.favs;
        self.studio = l.studio;
        self.reviews = l.reviews;
        // open on the season you're in
        if let Some(f) = &self.full {
            let at = self.item.as_ref().and_then(|i| i["state"]["season"].as_i64());
            let seasons = seasons(f);
            self.season = at.filter(|s| seasons.contains(s)).or_else(|| seasons.iter().copied().find(|s| *s > 0)).or(seasons.first().copied());
        }
    }

    /// post your review (with your rating) to the couch
    fn post_review(&mut self, cx: &mut Context<Self>) {
        let text = self.review_text.read(cx).content.trim().to_string();
        if text.is_empty() {
            return;
        }
        let m = self.meta();
        let title = json!({ "id": m.id.split(':').next().unwrap_or(""), "type": m.kind, "name": m.name, "poster": m.poster, "background": m.background, "logo": m.logo });
        let rating = self.rating;
        self.writing = false;
        self.review_text.update(cx, |i, cx| i.set_text("", cx));
        let rid = m.id.split(':').next().unwrap_or("").to_string();
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("crew_social", json!({ "method": "POST", "path": "/v1/activity", "body": { "kind": "review", "title": title, "rating": rating, "text": text } })).await;
            let list = api::call::<Value>("crew_social", json!({ "method": "GET", "path": format!("/v1/reviews/{}", urlencoding::encode(&rid)), "body": Value::Null })).await.unwrap_or_default();
            let _ = this.update(cx, |d, cx| {
                d.reviews = list["reviews"].as_array().cloned().unwrap_or_default();
                Nav::toast(if r.is_ok() { "posted to the couch".into() } else { "couldn't post that, try again".into() }, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// my Discord id (who "you" is in the reviews)
    fn me_id(&self) -> String {
        couch::state::get().core.db.get::<Value>("discord_user").and_then(|u| u["id"].as_str().map(String::from)).unwrap_or_default()
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

    /// "1h 12m left" for where you stopped (in `video`, for a show), if anywhere
    fn left(&self, video: Option<&str>) -> Option<String> {
        let i = self.item.as_ref()?;
        let st = &i["state"];
        if let Some(v) = video {
            if st["video_id"].as_str() != Some(v) {
                return None;
            }
        }
        let (d, o) = (st["duration"].as_f64()?, st["timeOffset"].as_f64().unwrap_or(0.));
        if d <= 0. || o / d < 0.02 {
            return None;
        }
        // the library keeps milliseconds
        let secs = ((d - o).max(0.) / if d > 100_000. { 1000. } else { 1. }) as i64;
        let m = (secs + 59) / 60;
        Some(if m >= 60 { format!("{}h {}m left", m / 60, m % 60) } else { format!("{m}m left") })
    }

    /// What the big button does: resume, next unwatched episode, or start.
    fn primary(&self) -> (String, Option<Value>, String) {
        let Some(f) = &self.full else { return ("Play".into(), None, String::new()) };
        let eps: Vec<Value> = playback::ordered(f);
        if f["type"] != "series" || eps.is_empty() {
            return match self.left(None) {
                Some(l) => ("Resume".into(), None, l),
                None => ("Play".into(), None, String::new()),
            };
        }
        let label = |v: &Value| format!("S{} E{}", v["season"].as_i64().unwrap_or(0), v["episode"].as_i64().or(v["number"].as_i64()).unwrap_or(0));
        if let Some(vid) = self.item.as_ref().and_then(|i| i["state"]["video_id"].as_str()) {
            if self.progress(Some(vid)) > 0.02 {
                if let Some(v) = eps.iter().find(|x| x["id"].as_str() == Some(vid)) {
                    let sub = match self.left(Some(vid)) {
                        Some(l) => format!("{} · {l}", label(v)),
                        None => label(v),
                    };
                    return ("Resume".into(), Some(v.clone()), sub);
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
    /// your score: one bar you slide (or click) along, 1–10
    fn stars(&self, m: &Meta, cx: &mut Context<Self>) -> impl IntoElement {
        const W: f32 = 250.;
        let shown = if self.hover_star > 0 { self.hover_star } else { self.rating.unwrap_or(0) };
        let at = self.rate_box.clone();
        let at2 = at.clone();
        let at3 = at.clone();
        let pick = |x: f32, b: (f32, f32)| -> u8 { (((x - b.0) / b.1.max(1.)) * 10.).ceil().clamp(1., 10.) as u8 };
        let word = match shown {
            0 => "",
            1..=2 => "nope",
            3..=4 => "meh",
            5..=6 => "alright",
            7 => "good",
            8 => "great",
            9 => "amazing",
            _ => "masterpiece",
        };
        let previewing = self.hover_star > 0 && Some(self.hover_star) != self.rating;
        let accent: gpui::Hsla = self.tint.unwrap_or(theme::lamp().into());
        let (kind, id, name) = (m.kind.clone(), self.lid(), m.name.clone());
        let title = json!({ "id": m.id, "type": m.kind, "name": m.name, "poster": m.poster, "background": m.background, "logo": m.logo });
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(div().text_size(px(14.)).text_color(theme::label3()).child("your rating"))
            .child(
                div()
                    .id("rate-bar")
                    .relative()
                    .w(px(W))
                    .h(px(26.))
                    .flex_none()
                    .cursor_pointer()
                    .child(gpui::canvas(move |b, _, _| at3.set((f32::from(b.origin.x), f32::from(b.size.width))), |_, _, _, _| {}).absolute().size_full())
                    // ten steps, lit up to the score (dimmer while you're only pointing)
                    .child(
                        div().absolute().top_0().left_0().size_full().flex().gap(px(3.)).children((1..=10u8).map(|i| {
                            let lit = i <= shown;
                            div().flex_1().h_full().rounded(px(6.)).bg(if lit { accent } else { theme::white(0.09) })
                            .when(lit && previewing, |d| d.opacity(0.6))
                        })),
                    )
                    .when(shown == 0, |d| d.child(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child("slide to rate")))
                    .on_hover(cx.listener(|d, on: &bool, _, cx| {
                        if !*on {
                            d.hover_star = 0;
                            cx.notify();
                        }
                    }))
                    .on_mouse_move(cx.listener(move |d, e: &gpui::MouseMoveEvent, _, cx| {
                        let n = pick(f32::from(e.position.x), at.get());
                        if d.hover_star != n {
                            d.hover_star = n;
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(gpui::MouseButton::Left, cx.listener(move |d, e: &gpui::MouseUpEvent, window, cx| {
                        let n = pick(f32::from(e.position.x), at2.get());
                        // the same score again takes it off
                        let v = if d.rating == Some(n) { None } else { Some(n) };
                        d.rating = v;
                        d.hover_star = 0;
                        // rated: a few words about it?
                        if v.is_some() {
                            d.writing = true;
                            let f = gpui::Focusable::focus_handle(d.review_text.read(cx), cx);
                            window.focus(&f, cx);
                        }
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
                    })),
            )
            // always the same width, so nothing beside it moves while you point
            .child(
                div()
                    .w(px(150.))
                    .flex_none()
                    .flex()
                    .items_end()
                    .gap(px(6.))
                    .when(shown > 0, |d| {
                        d.child(div().text_size(px(22.)).line_height(px(22.)).font_weight(FontWeight::BLACK).text_color(accent).child(format!("{shown}")))
                            .child(div().text_size(px(13.)).text_color(theme::label2()).child(format!("/10 · {word}")))
                    }),
            )
    }
}

fn act(id: &'static str, ic: impl IntoElement, label: &str) -> gpui::Stateful<gpui::Div> {
    // the same family as Play: a rounded square, a quiet fill, the word under it
    div()
        .id(id)
        .flex()
        .flex_col()
        .items_center()
        .gap(px(7.))
        .w(px(66.))
        .cursor_pointer()
        .group("act")
        .child(
            div()
                .size(px(54.))
                .rounded(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .bg(theme::white(0.12))
                .border_1()
                .border_color(theme::white(0.08))
                .group_hover("act", |s| s.bg(theme::white(0.2)))
                .child(ic),
        )
        .child(div().w_full().text_center().text_size(px(12.5)).line_height(px(15.)).font_weight(FontWeight::MEDIUM).text_color(theme::label2()).child(label.to_string()))
}

impl Render for Detail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if TOP.with(|t| t.replace(false)) {
            self.scroll.set_offset(gpui::point(px(0.), px(0.)));
            crate::ui::smooth::reset("detail");
        }
        // more like this: TMDB's ids turned into the real ones, all at once
        if !self.recs_asked {
            if let Some(list) = self.extra.as_ref().and_then(|e| e["recommendations"].as_array()).filter(|a| !a.is_empty()) {
                self.recs_asked = true;
                let list: Vec<Meta> = list.iter().filter_map(|r| serde_json::from_value(r.clone()).ok()).collect();
                cx.spawn(async move |this, cx| {
                    let out = futures::future::join_all(list.into_iter().map(|r| async move {
                        if !r.id.starts_with("tmdb:") {
                            return r;
                        }
                        match api::call::<Value>("tmdb_resolve", json!({ "kind": r.kind, "id": r.id })).await {
                            Ok(v) if v["id"].as_str().map_or(false, |i| i.starts_with("tt")) => {
                                let mut m = r.clone();
                                m.id = v["id"].as_str().unwrap_or(&r.id).to_string();
                                m.kind = v["type"].as_str().unwrap_or(&r.kind).to_string();
                                m
                            }
                            _ => r,
                        }
                    }))
                    .await;
                    let _ = this.update(cx, |d, cx| {
                        d.recs = Some(out);
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        let vh = window.viewport_size().height;
        let vw = window.viewport_size().width;
        // opened from a card: its picture (and logo) fly into place, the rest follows
        let op = crate::ui::zoom::opening(window);
        let grow = op.as_ref().map_or(1., |o| o.k);
        let rz: Vec<(f32, f32)> = (0..8).map(|i| crate::ui::zoom::rise(window, i)).collect();
        let art_h = vh * 0.92;
        let m = self.meta();
        let g = theme::GUTTER;
        let art = m.background.clone().or(m.poster.clone()).map(|u| crate::net::fit(&sized(&u, "original"), crate::ui::view_w()));
        let logo = m.logo.clone().map(|u| crate::net::logo(&sized(&u, "large"), 480.));
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

        let (mut plabel, mut pvideo, mut psub) = self.primary();
        // all of it seen: the film's flag, or every released episode of a show
        let all_seen = {
            let flagged_now = self.item.as_ref().map_or(false, |i| crate::api::flag(&i["state"]["flaggedWatched"]));
            match &full {
                Some(f) if f["type"] == "series" => {
                    let today = chrono_now();
                    let w = self.watched();
                    let eps: Vec<String> = playback::ordered(f).iter().filter(|v| v["season"].as_i64().unwrap_or(0) > 0 && released(v, &today)).filter_map(|v| v["id"].as_str().map(String::from)).collect();
                    !eps.is_empty() && eps.iter().all(|e| w.contains(e))
                }
                _ => flagged_now || self.item.as_ref().map_or(false, |i| i["state"]["timesWatched"].as_i64().unwrap_or(0) > 0),
            }
        };
        // all seen and nothing half-way: watch it again from the start
        if all_seen && plabel != "Resume" {
            plabel = "Rewatch".into();
            if let Some(f) = &full {
                if f["type"] == "series" {
                    let first = playback::ordered(f).into_iter().find(|v| v["season"].as_i64().unwrap_or(0) > 0);
                    psub = first.as_ref().map(|v| format!("S{} E{}", v["season"].as_i64().unwrap_or(0), v["episode"].as_i64().or(v["number"].as_i64()).unwrap_or(0))).unwrap_or_default();
                    pvideo = first;
                }
            }
        }
        let flagged = self.item.as_ref().map_or(false, |i| crate::api::flag(&i["state"]["flaggedWatched"]));
        // in My List, or in any of your couch lists
        let base_id = m.id.split(':').next().unwrap_or("").to_string();
        let owned = self.item.as_ref().map_or(false, |i| i["removed"].as_bool() != Some(true) && i["temp"].as_bool() != Some(true))
            || crate::views::addto::load().iter().any(|l| l["items"].as_array().map_or(false, |a| a.iter().any(|x| x["id"].as_str().map_or(false, |x| x.split(':').next() == Some(base_id.as_str())))));
        // the library changed somewhere else (the + sheet): read this title again
        let lg = LIB_GEN.load(std::sync::atomic::Ordering::Relaxed);
        if lg != self.lib_seen {
            self.lib_seen = lg;
            self.refresh_item(cx);
        }
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
        // the trailer: the page fades back, a scroll or ^ (or its end) brings it back
        let th_on = matches!(self.theater, Some((_, true)));
        if th_on {
            let started = self.theater.map_or(0., |t| t.0.elapsed().as_secs_f32());
            let scrolled = -f32::from(self.scroll.offset().y) > 30.;
            let over = started > 2. && !trailer_on;
            if scrolled || over {
                let o = towner.clone();
                vid.update(cx, |x, cx| x.stop(&o, cx));
                self.theater = Some((std::time::Instant::now(), false));
            }
        }
        let th = match self.theater {
            Some((t, on)) => {
                let x = (t.elapsed().as_secs_f32() / 0.6).min(1.);
                if x < 1. {
                    window.request_animation_frame();
                }
                let e = x * x * (3. - 2. * x);
                if on { e } else { 1. - e }
            }
            None => 0.,
        };
        let th_on = matches!(self.theater, Some((_, true)));
        THEATER.with(|t| t.set(th_on));
        // opened from a card: the words wait for the picture to land
        let ca = crate::ui::zoom::content_alpha(window);
        // the card's logo is flying into this one: this one waits
        // (the card's logo doesn't fly in: a card's and a page's logos aren't always the same picture)
        let flying_logo = false;
        let kw: Vec<String> = ex["keywords"].as_array().into_iter().flatten().filter_map(|k| k.as_str().map(String::from).or(k["name"].as_str().map(String::from))).take(8).collect();
        // the mood goes on the right, across from the words, when the window is wide enough
        let mood_right = f32::from(vw) > 1180.;
        let fly = match (&op, flying_logo) {
            (Some(o), true) => o.logo.clone().zip(o.target).map(|((from, small), to)| {
                // the page's logo box: as tall as it gets, as wide as this logo's shape
                let aspect = f32::from(from.size.width) / f32::from(from.size.height).max(1.);
                let tw = (160. * aspect).min(480.);
                let k = o.k;
                let lerp = |a: f32, b: f32| a + (b - a) * k;
                let (x, y) = (lerp(f32::from(from.origin.x), f32::from(to.origin.x)), lerp(f32::from(from.origin.y), f32::from(to.origin.y)));
                let (w, h) = (lerp(f32::from(from.size.width), tw), lerp(f32::from(from.size.height), 160.));
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .child(img(small).absolute().size_full().object_fit(ObjectFit::Contain))
                    .children(logo.clone().map(|u| img(u).absolute().size_full().object_fit(ObjectFit::Contain)))
            }),
            _ => None,
        };
        let hero = div()
            .relative()
            .w_full()
            .min_h(vh * 0.92)
            .flex_none()
            .flex()
            .flex_col()
            .justify_end()
            // the art, and the trailer over it once it's playing
            // (a fixed height: the page growing taller never zooms the picture)
            .child(div().absolute().top_0().left_0().w_full().h(art_h).children(match (&op, &tframe) {
                // still flying in from the card
                (Some(o), _) if o.t < crate::ui::zoom::DUR => {
                    let k = o.k;
                    let lerp = |a: gpui::Pixels, b: gpui::Pixels| a + (b - a) * k;
                    let big_a = ((k - 0.15) / 0.55).clamp(0., 1.);
                    vec![div()
                        .absolute()
                        .left(lerp(o.from.origin.x, px(0.)))
                        .top(lerp(o.from.origin.y, px(0.)))
                        .w(lerp(o.from.size.width, vw))
                        .h(lerp(o.from.size.height, art_h))
                        .rounded(px(o.radius * (1. - k)))
                        .overflow_hidden()
                        .children(o.small.clone().map(|u| img(u).absolute().size_full().object_fit(ObjectFit::Cover).opacity(1. - big_a)))
                        .children(o.big.clone().or(art.clone()).map(|u| img(u).absolute().size_full().object_fit(ObjectFit::Cover).opacity(big_a)))
                        .into_any_element()]
                }
                (_, Some((p, a))) => crate::ui::video::layers(art.clone(), p, *a, 0.),
                _ => art.clone().map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).into_any_element()).into_iter().collect(),
            }))
            // the shade and the melt come in with the picture, and then simply stay
            .child(div().absolute().top_0().left_0().w_full().h(art_h).opacity((1. - th) * grow).bg(linear_gradient(90., linear_color_stop(theme::black(0.85), 0.), linear_color_stop(theme::black(0.0), 0.7))))
            .child(div().absolute().left_0().top(art_h - vh * 0.5).w_full().h(vh * 0.5).opacity((1. - th * 0.85) * grow).child(crate::ui::ambient::melt()))
            // watching the trailer: while it loads the couch turns; a hint at the
            // bottom says how to get back (scroll down, or click it)
            .when(th_on, |d| {
                let o = towner.clone();
                let waiting = tframe.is_none();
                let spin = waiting.then(|| crate::couch_spin_frame(self.theater.map_or(0., |t| t.0.elapsed().as_secs_f32()))).flatten();
                if waiting {
                    window.request_animation_frame();
                }
                d.when(waiting, |d| {
                    d.child(
                        div().absolute().top_0().left_0().w_full().h(art_h).flex().items_center().justify_center().opacity(th).child(
                            div().w(px(150.)).h(px(135.)).children(spin.map(|f| img(gpui::ImageSource::Render(f)).size_full().object_fit(ObjectFit::Contain))),
                        ),
                    )
                })
                .child(
                    div().absolute().bottom(px(22.)).left_0().w_full().flex().justify_center().opacity(th).child(
                        div()
                            .id("d-back-up")
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(4.))
                            .px(px(16.))
                            .py(px(8.))
                            .rounded(px(14.))
                            .cursor_pointer()
                            .text_color(theme::white(0.8))
                            .hover(|s| s.bg(theme::white(0.08)).text_color(theme::text()))
                            .on_click(cx.listener(move |me, _, _, cx| {
                                let o = o.clone();
                                crate::ui::video::get(cx).update(cx, |x, cx| x.stop(&o, cx));
                                me.theater = Some((std::time::Instant::now(), false));
                                cx.notify();
                            }))
                            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("scroll down to exit"))
                            .child(icon("chevronDown", 22.)),
                    ),
                )
            })
            .child(
                // the words start well below the tabs so the art shows; a tall
                // block makes the billboard taller rather than climbing up into it
                div()
                    .relative()
                    .opacity(1. - th)
                    .ml(px(g))
                    .pt(vh * 0.4)
                    .pb(px(22.))
                    .w(px(640.))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    // the main studio, above the title (opens its page)
                    .children(self.studio.clone().map(|(cid, cname, pic, svg)| {
                        let n2 = cname.clone();
                        div()
                            .id("d-studio")
                            .relative()
                            .top(px(rz[1].1 * 2.6))
                            .opacity(rz[1].0)
                            .flex()
                            .h(px(34.))
                            .items_center()
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.75))
                            .on_click(move |_, _, cx| Nav::go(crate::Route::Studio(cid, n2.clone()), cx))
                            .child(match (pic, svg) {
                                (Some(i), _) => img(i).h(px(30.)).max_w(px(200.)).object_fit(ObjectFit::Contain).into_any_element(),
                                (None, Some(u)) => img(u).h(px(30.)).max_w(px(200.)).object_fit(ObjectFit::Contain).into_any_element(),
                                _ => div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.85)).child(cname.to_uppercase()).into_any_element(),
                            })
                    }))
                    .child(
                        div()
                            .relative()
                            .flex()
                            .opacity(if flying_logo { 0. } else if op.is_some() { rz[0].0 } else { 1. })
                            .child(crate::ui::zoom::track_target())
                            .child(match logo.clone() {
                                Some(u) => img(u)
                                    .h(px(160.))
                                    .max_w(px(480.))
                                    .object_fit(ObjectFit::Contain)
                                    .with_fallback(move || big_name(&name))
                                    .into_any_element(),
                                None => big_name(&m.name),
                            }),
                    )
                    .child(div().relative().top(px(rz[2].1)).opacity(rz[2].0).flex().gap(px(10.)).text_size(px(16.)).font_weight(FontWeight::MEDIUM).children(facts.into_iter().enumerate().flat_map(|(i, f)| {
                        let dot = (i > 0).then(|| div().text_color(theme::label3()).child("·").into_any_element());
                        dot.into_iter().chain(std::iter::once(div().child(f).into_any_element()))
                    })))
                    .child(
                        div()
                            .relative()
                            .top(px(rz[3].1))
                            .opacity(rz[3].0)
                            .flex()
                            .items_start()
                            .gap(px(8.))
                            .child(
                                div()
                                    .id("d-play")
                                    .mr(px(8.))
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
                                        let again = plabel == "Rewatch";
                                        move |_, _, cx| {
                                            // "Rewatch" is your say-so: Simkl may log this as a rewatch (if that's on)
                                            if again {
                                                let _ = couch::state::get().core.db.set("simkl_rewatch_for", &m.id.split(':').next().unwrap_or("").to_string());
                                            }
                                            match &pvideo {
                                            Some(v) => Nav::play_episode(&m, v["id"].as_str().unwrap_or("").to_string(), cx),
                                            None => Nav::play(&m, cx),
                                            }
                                        }
                                    })
                                    .child(icon_fill("play", 20.).text_color(gpui::rgb(0x000000)))
                                    .child(plabel.clone())
                                    .children((!psub.is_empty()).then(|| div().text_size(px(15.)).font_weight(FontWeight::MEDIUM).opacity(0.6).child(psub))),
                            )
                            .child(act("d-seen", icon(if all_seen { "checkCircle" } else { "eye" }, 22.), if all_seen { "Watched" } else { "Mark watched" }).on_click(
                                cx.listener(move |d, _, _, cx| {
                                    let Some(f) = d.full.clone() else { return };
                                    // a show: every released episode at once, on or off
                                    let on = !all_seen;
                                    let today = chrono_now();
                                    let all: Option<Vec<String>> = (f["type"] == "series").then(|| {
                                        playback::ordered(&f)
                                            .iter()
                                            .filter(|v| v["season"].as_i64().unwrap_or(0) > 0 && released(v, &today))
                                            .filter_map(|v| v["id"].as_str().map(|s| s.to_string()))
                                            .collect()
                                    });
                                    // show it straight away; the library's answer follows
                                    if let Some(i) = d.item.as_mut() {
                                        i["state"]["flaggedWatched"] = json!(if on { 1 } else { 0 });
                                        i["state"]["timesWatched"] = json!(if on { 1 } else { 0 });
                                        if let Some(a) = &all {
                                            i["state"]["watchedIds"] = json!(if on { a.clone() } else { vec![] });
                                        }
                                    }
                                    cx.notify();
                                    cx.spawn(async move |this, cx| {
                                        let r = api::call::<Value>("library_set_watched", json!({ "meta": f, "videoId": Value::Null, "watched": on, "allVideos": all })).await;
                                        let _ = this.update(cx, |d, cx| {
                                            match r {
                                                Ok(item) if !item.is_null() => {
                                                    d.item = Some(item);
                                                    cx.notify();
                                                }
                                                _ => d.refresh_item(cx),
                                            }
                                            Nav::toast(if on { "marked watched".into() } else { "marked unwatched".into() }, cx);
                                        });
                                        crate::views::detail::library_changed();
                                    })
                                    .detach();
                                }),
                            ))
                            .child(act("d-list", icon(if owned { "check" } else { "plus" }, 22.), "My List").on_click({
                                // My List, your couch lists, or a new one
                                let m = m.clone();
                                move |_, _, cx| Nav::add(&m, cx)
                            }))
                            .child({
                                let base = m.id.split(':').next().unwrap_or("").to_string();
                                let fav = self.favs.as_ref().map_or(false, |f| f.iter().any(|x| x["id"].as_str() == Some(base.as_str())));
                                let r = json!({ "id": base, "type": m.kind, "name": m.name, "poster": m.poster, "background": m.background, "logo": m.logo });
                                act("d-fav", if fav { icon_fill("heart", 21.).text_color(self.tint.unwrap_or(theme::neon_soft().into())) } else { icon("heart", 21.) }, "Favourite").on_click(cx.listener(move |d, _, _, cx| {
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
                            .when(self.trailer.as_ref().map_or(false, |k| !k.is_empty()), |d| {
                                let key = self.trailer.clone().unwrap_or_default();
                                let o = towner.clone();
                                d.child(act("d-trailer", icon("film", 22.), "Trailer").on_click(cx.listener(move |me, _, _, cx| {
                                    if th > 0.01 {
                                        return;
                                    }
                                    me.scroll.set_offset(gpui::point(px(0.), px(0.)));
                                    me.theater = Some((std::time::Instant::now(), true));
                                    let url = format!("https://www.youtube.com/watch?v={key}");
                                    let o = o.clone();
                                    crate::ui::video::get(cx).update(cx, |x, cx| x.play_once(&o, &url, cx));
                                    cx.notify();
                                })))
                            })
                            .child(act("d-share", icon("share", 22.), "Share").on_click({
                                let m = m.clone();
                                move |_, _, cx| {
                                    let url = couch::commands::share_link(m.kind.clone(), m.id.clone(), m.name.clone());
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(url));
                                    Nav::toast("link copied. paste it anywhere".into(), cx);
                                }
                            }))
                            .child(act("d-party", icon("couch", 22.), "Watch party").on_click({
                                let m = m.clone();
                                move |_, _, cx| Nav::plan_party(&m, cx)
                            }))
                    )
                    .children(ex["tagline"].as_str().map(|t| div().relative().top(px(rz[4].1)).opacity(rz[4].0).text_size(px(17.)).italic().text_color(theme::label2()).child(t.to_string())))
                    .children(m.description.clone().or(ex["overview"].as_str().map(|s| s.to_string())).map(|d| {
                        div().relative().top(px(rz[5].1)).opacity(rz[5].0).max_w(px(600.)).text_size(px(17.)).line_height(px(25.)).line_clamp(4).text_ellipsis().text_color(theme::white(0.88)).child(d)
                    }))
                    // the mood (TMDB keywords), as quiet chips (here only in a narrow window)
                    .children({
                        let kw: Vec<String> = if mood_right { vec![] } else { kw.clone() };
                        (!kw.is_empty()).then(|| {
                            div().flex().flex_wrap().gap(px(6.)).max_w(px(620.)).children(kw.into_iter().map(|k| {
                                div().px(px(10.)).py(px(3.)).rounded_full().bg(theme::white(0.08)).text_size(px(13.)).text_color(theme::label2()).child(k)
                            }))
                        })
                    })
                    .when(seen_it, |d| {
                        d.child(
                            div()
                                .relative()
                                .top(px(rz[6].1))
                                .opacity(rz[6].0)
                                .flex()
                                .items_center()
                                .gap(px(16.))
                                .child(self.stars(&m, cx))
                        )
                        .when(self.writing, |d| {
                            d.child(
                                div()
                                    .max_w(px(620.))
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(div().flex_1().h(px(44.)).px(px(14.)).flex().items_center().rounded(px(12.)).bg(theme::white(0.1)).child(self.review_text.clone()))
                                    .child(
                                        div()
                                            .id("d-post")
                                            .px(px(16.))
                                            .h(px(44.))
                                            .flex()
                                            .items_center()
                                            .rounded(px(12.))
                                            .bg(theme::neon())
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .cursor_pointer()
                                            .hover(|s| s.opacity(0.85))
                                            .on_click(cx.listener(|d, _, _, cx| d.post_review(cx)))
                                            .child("post"),
                                    ),
                            )
                        })
                    }),
            )
            .when(mood_right && !kw.is_empty(), |d| {
                d.child(
                    div()
                        .absolute()
                        .right(px(g))
                        .bottom(px(30. - rz[5].1))
                        .w(px(340.))
                        .opacity((1. - th) * rz[5].0)
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap(px(10.))
                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child("the mood"))
                        .child(div().flex().flex_wrap().justify_end().gap(px(6.)).children(kw.iter().map(|k| {
                            div().px(px(10.)).py(px(3.)).rounded_full().bg(theme::white(0.08)).text_size(px(13.)).text_color(theme::label2()).child(k.clone())
                        }))),
                )
            })
            .children(fly);

        let page = div().id("detail").size_full().overflow_y_scroll().track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "detail")).child(hero);
        // what arrived after the page opened eases in instead of popping
        let fade = match self.shown {
            Some(t) if t.elapsed().as_secs_f32() < 0.35 => {
                window.request_animation_frame();
                let e = t.elapsed().as_secs_f32() / 0.35;
                1. - (1. - e).powi(3)
            }
            _ => 1.,
        };
        let mut rest = div().flex().flex_col().relative().top(px((1. - fade) * 16.)).opacity(fade * (1. - th) * ca);

        // ------------------------------------------------- what the crew said
        let said: Vec<&Value> = self.reviews.iter().filter(|r| r["text"].as_str().map_or(false, |t| !t.is_empty()) || r["rating"].is_number()).collect();
        if !said.is_empty() {
            let me = self.me_id();
            rest = rest.child(
                // the crew's reviews, no heading: snug cards, the avatar beside the words
                div().mt(px(4.)).px(px(g)).flex().flex_wrap().items_start().gap(px(10.)).children(said.into_iter().map(|r| {
                    let who = if r["who"].as_str() == Some(me.as_str()) { "you".to_string() } else { r["name"].as_str().unwrap_or("someone").to_string() };
                    let wid = r["who"].as_str().unwrap_or("").to_string();
                    let text = r["text"].as_str().filter(|t| !t.is_empty()).map(String::from);
                    div()
                        .max_w(px(440.))
                        .py(px(10.))
                        .pl(px(10.))
                        .pr(px(16.))
                        .rounded(px(18.))
                        .bg(theme::white(0.05))
                        .flex()
                        .items_start()
                        .gap(px(10.))
                        .child(div().flex_none().size(px(28.)).rounded_full().when(r["avatar"].as_str().is_none(), |d| d.bg(theme::surface2())).flex().items_center().justify_center().text_size(px(12.)).font_weight(FontWeight::BLACK).child(match r["avatar"].as_str() {
                            Some(u) => img(crate::net::face(u, 28.)).size_full().into_any_element(),
                            None => div().child(who.chars().next().unwrap_or('?').to_uppercase().to_string()).into_any_element(),
                        }))
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .when(text.is_none(), |d| d.h(px(28.)).justify_center())
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .text_size(px(14.))
                                        .child(div().id(SharedString::from(format!("rv-{wid}"))).font_weight(FontWeight::SEMIBOLD).cursor_pointer().hover(|s| s.underline()).on_click(move |_, _, cx| Nav::go(crate::Route::Member(wid.clone()), cx)).child(crate::ui::emoji::text(&who, 14.)))
                                        .children(r["rating"].as_i64().map(|n| div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::lamp()).child(format!("★ {n}/10")))),
                                )
                                .children(text.map(|t| div().text_size(px(14.)).line_height(px(20.)).text_color(theme::white(0.75)).child(crate::ui::emoji::text(&t, 14.)))),
                        )
                })),
            );
        }

        // ---------------------------------------------------------- episodes
        if let Some(f) = &full {
            let eps = playback::ordered(f);
            if !eps.is_empty() {
                let ss = seasons(f);
                let cur = self.season.unwrap_or(ss.first().copied().unwrap_or(1));
                let watched = self.watched();
                let today = chrono_now();
                let back_art = m.background.clone().map(|u| sized(&u, "w780"));
                rest = rest.child(
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
                        .child(crate::ui::rows::scroller("eps", div().id("eps").overflow_x_scroll().flex().gap(px(14.)).px(px(g)).children(
                                eps.iter().filter(|v| v["season"].as_i64().unwrap_or(0) == cur).map(|v| {
                                    let vid = v["id"].as_str().unwrap_or("").to_string();
                                    let n = v["episode"].as_i64().or(v["number"].as_i64()).unwrap_or(0);
                                    let seen = watched.contains(&vid);
                                    let future = !released(v, &today);
                                    let p = self.progress(Some(&vid));
                                    // under 10% to go: it's as good as watched
                                    let nearly = p >= 0.9;
                                    let p = if nearly { 0. } else { p };
                                    let left = if nearly { None } else { self.left(Some(&vid)) };
                                    let imdb = api::imdb_episode(&vid);
                                    let thumb = v["thumbnail"].as_str().map(|s| s.to_string()).or(imdb.clone()).or(back_art.clone()).map(|u| crate::net::fit(&u, 300.));
                                    let alt = imdb.or(back_art.clone()).map(|u| crate::net::fit(&u, 300.));
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
                                                .on_click(move |_, _, cx| {
                                                    if !future && !crate::ui::rows::dragged() {
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
                                                // watched: a soft shade from the top right corner
                                                .when(seen, |d| d.child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(225., linear_color_stop(theme::black(0.55), 0.), linear_color_stop(theme::black(0.0), 0.6)))))
                                                // where you stopped: how much is left
                                                .children(left.map(|l| {
                                                    div()
                                                        .absolute()
                                                        .right(px(10.))
                                                        .bottom(px(12.))
                                                        .px(px(8.))
                                                        .py(px(2.))
                                                        .rounded_full()
                                                        .bg(theme::black(0.6))
                                                        .text_size(px(12.))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(l)
                                                }))
                                                .when(p > 0.01, |d| {
                                                    d.child(div().absolute().left_0().bottom_0().w_full().h(px(4.)).bg(theme::white(0.25)).child(
                                                        div().h_full().w(px(300. * p as f32)).bg(gpui::white()),
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
                            ),)),
                );
            }
        }

        // -------------------------------------------------------------- cast
        if let Some(cast) = ex["cast"].as_array().filter(|c| !c.is_empty()) {
            rest = rest.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .mt(px(28.))
                    .child(div().px(px(g)).text_size(px(24.)).font_weight(FontWeight::BLACK).child("cast"))
                    .child(crate::ui::rows::scroller("cast", div().id("cast").overflow_x_scroll().flex().gap(px(16.)).px(px(g)).children(cast.iter().take(20).map(|c| {
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
                    })))),
            );
        }

        // -------------------------------------------------------- more like it
        if let Some(recs) = ex["recommendations"].as_array().filter(|c| !c.is_empty()) {
            let recs: Vec<Meta> = self.recs.clone().unwrap_or_else(|| recs.iter().filter_map(|r| serde_json::from_value(r.clone()).ok()).collect());
            // the one previewing under the pointer
            let owner = vid.read(cx).owner.clone();
            let clip = if owner.starts_with("card:") {
                let o = owner.clone();
                vid.update(cx, |x, _| {
                    let f = x.frame(&o, window)?;
                    let t = x.since.map_or(0., |t| t.elapsed().as_secs_f32());
                    Some((f, (t / 0.5).min(1.), t))
                })
            } else {
                None
            };
            if clip.as_ref().map_or(false, |c| c.2 < 3.) {
                window.request_animation_frame();
            }
            let cowner = owner.trim_start_matches("card:").to_string();
            rest = rest.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .mt(px(36.))
                    .child(div().px(px(g)).text_size(px(24.)).font_weight(FontWeight::BLACK).child("more like this"))
                    .child(crate::ui::rows::scroller("recs", div().id("recs").overflow_x_scroll().flex().gap(px(10.)).px(px(g)).children(recs.into_iter().enumerate().map(|(i, r)| {
                        let rr = r.clone();
                        let c = if r.id == cowner { clip.clone() } else { None };
                        card::card_preview(SharedString::from(format!("rec-{i}")), &r, c, move |_, _, cx| open_rec(&rr, cx))
                    })))),
            );
        }
        page.child(rest).child(div().h(px(90.)))
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

/// the title in words when there's no logo: smaller for long names, never
/// more lines than fit where the logo would have gone (160px)
fn big_name(name: &str) -> gpui::AnyElement {
    let long = name.chars().count() > 22;
    let (size, lh, lines) = if long { (44., 48., 3) } else { (60., 64., 2) };
    div().max_w(px(480.)).text_size(px(size)).line_height(px(lh)).line_clamp(lines).text_ellipsis().font_weight(FontWeight::BLACK).child(name.to_string()).into_any_element()
}
