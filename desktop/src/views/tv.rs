//! TV mode: couch from across the room, after the Apple TV app (tvOS 26).
//! Full screen, the focused title's art filling it, tall "cinematic" posters
//! that lift when focused, glass buttons and a glass sidebar. Arrows (or a
//! controller: d-pad / stick, A, B, Y, Start) get everywhere; Enter plays.
use crate::api::{self, sized, Meta};
use crate::theme;
use crate::ui::icon;
use crate::views::home::Home;
use crate::Nav;
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, Context, FontWeight, Hsla, ObjectFit, WeakEntity, Window};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq)]
pub enum Section {
    Home,
    Shows,
    Movies,
    List,
}

const SIDE: &[(&str, &str)] = &[("home", "Home"), ("tv", "Shows"), ("film", "Movies"), ("bookmark", "My List"), ("search", "Search"), ("settings", "Settings"), ("close", "Exit TV mode")];

struct Row {
    key: String,
    title: String,
    items: Vec<Meta>,
    /// (progress, "S2 E4\t12m left") for up next
    extra: Vec<(Option<f32>, Option<String>)>,
    /// landscape cards (up next), else tall posters
    wide: bool,
}

/// a title opened (Y / i, or Enter on a show): play, list, its episodes
struct Page {
    meta: Meta,
    eps: Vec<Value>,
    seasons: Vec<i64>,
    season: i64,
    seen: Vec<String>,
    /// where you are in it: (video id, seconds in)
    resume: Option<(String, f64)>,
    /// 0 the buttons, 1 the seasons, 2 the episodes
    line: usize,
    col: [usize; 3],
}

pub struct Tv {
    pub on: bool,
    homes: [Option<WeakEntity<Home>>; 3],
    section: Section,
    lists: Vec<Row>,
    heroes: Vec<Meta>,
    hero_i: usize,
    side: bool,
    side_i: usize,
    /// 0: the billboard, 1..: the rows
    row: usize,
    cols: HashMap<String, usize>,
    page: Option<Page>,
    /// the backdrop: the one before, the one now, when it changed
    bg: (Option<String>, Option<String>, Instant),
    focus_at: Instant,
    /// eased scroll: rows up/down, each row sideways
    y: f32,
    xs: HashMap<String, f32>,
    last: Instant,
    clip: Option<(String, String)>,
    /// the billboard waited on this title before its trailer
    clip_for: String,
}

const OWNER: &str = "tv:";

fn ease(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

impl Tv {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            on: false,
            homes: [None, None, None],
            section: Section::Home,
            lists: vec![],
            heroes: vec![],
            hero_i: 0,
            side: false,
            side_i: 0,
            row: 0,
            cols: HashMap::new(),
            page: None,
            bg: (None, None, Instant::now()),
            focus_at: Instant::now(),
            y: -1.,
            xs: HashMap::new(),
            last: Instant::now(),
            clip: None,
            clip_for: String::new(),
        }
    }

    pub fn enter(&mut self, homes: [Option<WeakEntity<Home>>; 3], cx: &mut Context<Self>) {
        self.on = true;
        self.homes = homes;
        self.page = None;
        self.side = false;
        self.row = 0;
        self.y = -1.;
        self.load_heroes(cx);
        cx.notify();
    }

    pub fn leave(&mut self, cx: &mut Context<Self>) {
        self.on = false;
        self.stop_clip(cx);
        cx.notify();
    }

    fn scope(&self) -> &'static str {
        match self.section {
            Section::Shows => "series",
            Section::Movies => "movie",
            _ => "all",
        }
    }

    fn load_heroes(&mut self, cx: &mut Context<Self>) {
        let scope = self.scope();
        self.heroes.clear();
        self.hero_i = 0;
        cx.spawn(async move |this, cx| {
            let list = api::call::<Vec<Meta>>("hero_items", json!({})).await.unwrap_or_default();
            let list: Vec<Meta> = list.into_iter().filter(|m| m.background.is_some() && (scope == "all" || m.kind == scope)).take(8).collect();
            let _ = this.update(cx, |t, cx| {
                t.heroes = list;
                t.focus_at = Instant::now();
                cx.notify();
            });
        })
        .detach();
    }

    fn load_lists(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let v = api::call::<Value>("my_lists", json!({})).await.unwrap_or_default();
            let metas = |v: &Value| -> Vec<Meta> { v.as_array().into_iter().flatten().filter_map(|m| serde_json::from_value(m.clone()).ok()).filter(|m: &Meta| !m.id.is_empty()).collect() };
            let rows: Vec<Row> = [("plan", "on your list"), ("watching", "watching"), ("completed", "watched")]
                .iter()
                .map(|(k, t)| Row { key: format!("list-{k}"), title: t.to_string(), items: metas(&v[*k]), extra: vec![], wide: false })
                .filter(|r| !r.items.is_empty())
                .collect();
            let _ = this.update(cx, |t, cx| {
                t.lists = rows;
                cx.notify();
            });
        })
        .detach();
    }

    fn rows(&self, cx: &gpui::App) -> Vec<Row> {
        if self.section == Section::List {
            return self.lists.iter().map(|r| Row { key: r.key.clone(), title: r.title.clone(), items: r.items.clone(), extra: vec![], wide: false }).collect();
        }
        let i = match self.section {
            Section::Shows => 1,
            Section::Movies => 2,
            _ => 0,
        };
        let Some(h) = self.homes[i].as_ref().and_then(|h| h.upgrade()) else { return vec![] };
        let (_, rows) = h.read(cx).tv();
        rows.iter()
            .filter(|r| !r.items.is_empty())
            .map(|r| Row { key: r.key.clone(), title: r.title.clone(), items: r.items.clone(), extra: r.extra.clone(), wide: r.key == "cw" })
            .collect()
    }

    fn focused(&self, rows: &[Row]) -> Option<Meta> {
        if let Some(p) = &self.page {
            return Some(p.meta.clone());
        }
        if self.row == 0 {
            return self.heroes.get(self.hero_i).cloned();
        }
        let r = rows.get(self.row - 1)?;
        let c = *self.cols.get(&r.key).unwrap_or(&0);
        r.items.get(c.min(r.items.len().saturating_sub(1))).cloned()
    }

    fn moved(&mut self, cx: &mut Context<Self>) {
        self.focus_at = Instant::now();
        self.stop_clip(cx);
        cx.notify();
    }

    fn stop_clip(&mut self, cx: &mut Context<Self>) {
        if self.clip.take().is_some() {
            let v = crate::ui::video::get(cx);
            v.update(cx, |v, cx| v.stop_prefix(OWNER, cx));
        }
        self.clip_for.clear();
    }

    /// a key (or a controller button, already turned into one)
    pub fn press(&mut self, k: &str, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        if self.page.is_some() {
            return self.press_page(k, cx);
        }
        if self.side {
            match k {
                "up" => self.side_i = self.side_i.saturating_sub(1),
                "down" => self.side_i = (self.side_i + 1).min(SIDE.len() - 1),
                "right" | "escape" | "backspace" => self.side = false,
                "enter" | "space" => return self.side_pick(cx),
                _ => {}
            }
            return self.moved(cx);
        }
        match k {
            "up" => {
                if self.row > 0 {
                    self.row -= 1;
                }
            }
            "down" => {
                if self.row < rows.len() {
                    self.row += 1;
                }
            }
            "left" => {
                if self.row == 0 {
                    if self.hero_i == 0 || self.heroes.is_empty() {
                        self.side = true;
                    } else {
                        self.hero_i -= 1;
                    }
                } else if let Some(r) = rows.get(self.row - 1) {
                    let c = self.cols.entry(r.key.clone()).or_insert(0);
                    if *c == 0 {
                        self.side = true;
                    } else {
                        *c -= 1;
                    }
                }
            }
            "right" => {
                if self.row == 0 {
                    if !self.heroes.is_empty() {
                        self.hero_i = (self.hero_i + 1) % self.heroes.len();
                    }
                } else if let Some(r) = rows.get(self.row - 1) {
                    let c = self.cols.entry(r.key.clone()).or_insert(0);
                    *c = (*c + 1).min(r.items.len().saturating_sub(1));
                }
            }
            "enter" => {
                let Some(m) = self.focused(&rows) else { return };
                // up next and films play; a show opens (its episodes)
                let up_next = self.row > 0 && rows.get(self.row - 1).map_or(false, |r| r.wide);
                if m.kind == "series" && !up_next && self.row > 0 {
                    return self.open_page(m, cx);
                }
                return Nav::play(&m, cx);
            }
            "space" | "play" => {
                if let Some(m) = self.focused(&rows) {
                    return Nav::play(&m, cx);
                }
            }
            "i" | "info" => {
                if let Some(m) = self.focused(&rows) {
                    return self.open_page(m, cx);
                }
            }
            "escape" | "backspace" => {
                if self.row > 0 {
                    self.row = 0;
                } else {
                    self.side = true;
                }
            }
            _ => return,
        }
        self.moved(cx);
    }

    fn side_pick(&mut self, cx: &mut Context<Self>) {
        let (sec, go) = match self.side_i {
            0 => (Some(Section::Home), None),
            1 => (Some(Section::Shows), None),
            2 => (Some(Section::Movies), None),
            3 => (Some(Section::List), None),
            4 => (None, Some(crate::Route::Search)),
            5 => (None, Some(crate::Route::Settings)),
            _ => (None, None),
        };
        self.side = false;
        if let Some(s) = sec {
            self.section = s;
            self.row = 0;
            self.y = -1.;
            if s == Section::List {
                self.load_lists(cx);
            }
            self.load_heroes(cx);
            return self.moved(cx);
        }
        // search, settings, or out: back to the desktop look
        self.leave(cx);
        crate::Nav::tv_off(cx);
        if let Some(r) = go {
            Nav::go(r, cx);
        }
    }

    fn open_page(&mut self, meta: Meta, cx: &mut Context<Self>) {
        self.stop_clip(cx);
        self.page = Some(Page { meta: meta.clone(), eps: vec![], seasons: vec![], season: 1, seen: vec![], resume: None, line: 0, col: [0; 3] });
        self.focus_at = Instant::now();
        cx.spawn(async move |this, cx| {
            let full = api::call::<Value>("meta_get", json!({ "kind": meta.kind, "id": meta.id, "hint": Value::Null })).await.unwrap_or_default();
            let item = api::call::<Value>("library_item", json!({ "id": meta.id })).await.unwrap_or_default();
            let eps: Vec<Value> = crate::playback::ordered(&full).into_iter().filter(|e| e["season"].as_i64().unwrap_or(0) > 0).collect();
            let mut seasons: Vec<i64> = eps.iter().filter_map(|e| e["season"].as_i64()).collect();
            seasons.dedup();
            let st = &item["state"];
            let seen: Vec<String> = st["watchedIds"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            let resume = st["video_id"].as_str().filter(|v| !v.is_empty()).map(|v| (v.to_string(), st["timeOffset"].as_f64().unwrap_or(0.) / 1000.));
            let full_meta: Option<Meta> = serde_json::from_value(full.clone()).ok();
            let _ = this.update(cx, |t, cx| {
                if let Some(p) = t.page.as_mut().filter(|p| p.meta.id == meta.id) {
                    if let Some(mut f) = full_meta {
                        if f.id.is_empty() {
                            f.id = p.meta.id.clone();
                        }
                        if f.kind.is_empty() {
                            f.kind = p.meta.kind.clone();
                        }
                        p.meta = f;
                    }
                    // the season you're on
                    let at = resume.as_ref().and_then(|(v, _)| eps.iter().find(|e| e["id"].as_str() == Some(v.as_str())));
                    p.season = at.and_then(|e| e["season"].as_i64()).or(seasons.first().copied()).unwrap_or(1);
                    p.col[1] = seasons.iter().position(|s| *s == p.season).unwrap_or(0);
                    let in_season: Vec<&Value> = eps.iter().filter(|e| e["season"].as_i64() == Some(p.season)).collect();
                    p.col[2] = at.and_then(|a| in_season.iter().position(|e| e["id"] == a["id"])).unwrap_or(0);
                    p.eps = eps;
                    p.seasons = seasons;
                    p.seen = seen;
                    p.resume = resume;
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn press_page(&mut self, k: &str, cx: &mut Context<Self>) {
        let Some(p) = self.page.as_mut() else { return };
        let series = !p.eps.is_empty();
        let lines = if series { 3 } else { 1 };
        let in_season: Vec<Value> = p.eps.iter().filter(|e| e["season"].as_i64() == Some(p.season)).cloned().collect();
        let len = [2, p.seasons.len(), in_season.len()];
        match k {
            "up" => p.line = p.line.saturating_sub(1),
            "down" => p.line = (p.line + 1).min(lines - 1),
            "left" => p.col[p.line] = p.col[p.line].saturating_sub(1),
            "right" => p.col[p.line] = (p.col[p.line] + 1).min(len[p.line].saturating_sub(1)),
            "escape" | "backspace" => {
                self.page = None;
                return self.moved(cx);
            }
            "enter" | "space" | "play" => {
                let m = p.meta.clone();
                match (p.line, k) {
                    (0, "enter") if p.col[0] == 1 => {
                        Nav::add(&m, cx);
                    }
                    (2, _) => {
                        if let Some(id) = in_season.get(p.col[2]).and_then(|e| e["id"].as_str()) {
                            Nav::play_episode(&m, id.to_string(), cx);
                        }
                    }
                    (1, "enter") => {}
                    _ => Nav::play(&m, cx),
                }
                return;
            }
            _ => return,
        }
        // a new season: its episodes, from the first
        if p.line == 1 {
            if let Some(s) = p.seasons.get(p.col[1]).copied() {
                if s != p.season {
                    p.season = s;
                    p.col[2] = 0;
                }
            }
        }
        cx.notify();
    }
}

// ------------------------------------------------------------------ drawing

/// liquid glass: a pale see-through fill, a bright rim on top fading down
fn glass(d: gpui::Div, u: f32, on: bool) -> gpui::Div {
    d.bg(if on { theme::white(0.92) } else { theme::white(0.13) })
        .border_1()
        .border_color(if on { theme::white(1.) } else { theme::white(0.22) })
        .shadow(vec![gpui::BoxShadow { color: theme::black(if on { 0.45 } else { 0.25 }), offset: gpui::point(px(0.), px(12. * u)), blur_radius: px(34. * u), spread_radius: px(0.), inset: false }])
}

fn glass_btn(label: Option<&str>, ic: &str, on: bool, u: f32) -> AnyElement {
    let fg: Hsla = if on { gpui::black() } else { theme::white(0.95) };
    glass(div(), u, on)
        .h(px(64. * u))
        .px(px(if label.is_some() { 34. } else { 0. } * u))
        .w(if label.is_some() { gpui::Length::Auto } else { px(64. * u).into() })
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(12. * u))
        .text_size(px(22. * u))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(fg)
        .child(icon(ic, 26. * u).text_color(fg))
        .children(label.map(|l| l.to_string()))
        .into_any_element()
}

fn facts(m: &Meta) -> String {
    let mut f: Vec<String> = vec![if m.kind == "series" { "Series".into() } else { "Movie".into() }];
    if let Some(g) = m.genres.as_ref() {
        f.extend(g.iter().take(2).cloned());
    }
    if let Some(y) = m.year() {
        f.push(y);
    }
    if let Some(r) = m.imdb_rating.as_ref().and_then(|r| r.as_str().map(String::from).or(r.as_f64().map(|x| format!("{x:.1}")))) {
        f.push(format!("★ {r}"));
    }
    f.join("  ·  ")
}

fn logo_or_name(m: &Meta, h: f32, max_w: f32, u: f32) -> AnyElement {
    let name = m.name.clone();
    let words = move || div().max_w(px(max_w)).text_size(px(h * 0.42)).line_height(px(h * 0.46)).line_clamp(2).font_weight(FontWeight::BLACK).child(name.clone()).into_any_element();
    match m.logo.clone() {
        Some(u2) => img(crate::net::logo(&sized(&u2, "large"), max_w)).h(px(h)).max_w(px(max_w)).object_fit(ObjectFit::Contain).with_fallback(words).into_any_element(),
        None => {
            let _ = u;
            words()
        }
    }
}

impl Render for Tv {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vs = window.viewport_size();
        let (vw, vh) = (f32::from(vs.width), f32::from(vs.height));
        let u = (vh / 1080.).max(0.5);
        let g = 96. * u;
        let rows = self.rows(cx);
        let dt = self.last.elapsed().as_secs_f32().min(0.05);
        self.last = Instant::now();
        let mut busy = false;

        // the backdrop follows focus (a short cross-fade)
        let cur = self.focused(&rows);
        let art = cur.as_ref().and_then(|m| m.background.clone().or(m.poster.clone())).map(|u2| crate::net::fit(&sized(&u2, "original"), vw));
        if art != self.bg.1 {
            self.bg = (self.bg.1.take(), art.clone(), Instant::now());
        }
        let fade = ease(self.bg.2.elapsed().as_secs_f32() / 0.45);
        busy |= fade < 1.;

        // the billboard's trailer, after a few seconds on it
        let on_hero = self.row == 0 && self.page.is_none() && !self.side;
        if let Some(m) = cur.as_ref().filter(|_| on_hero) {
            if self.clip_for != m.id && self.focus_at.elapsed().as_secs_f32() > 2.5 {
                self.clip_for = m.id.clone();
                let m = m.clone();
                cx.spawn(async move |this, cx| {
                    let url = api::call::<Option<String>>("trailer_get", json!({ "kind": m.kind, "id": m.id })).await.ok().flatten().filter(|k| !k.is_empty()).map(|k| format!("https://www.youtube.com/watch?v={k}"));
                    let Some(url) = url else { return };
                    let owner = format!("{OWNER}{}", m.id);
                    let _ = this.update(cx, |t, cx| {
                        if t.clip_for == m.id && t.row == 0 && t.on {
                            t.clip = Some((owner.clone(), url.clone()));
                            let v = crate::ui::video::get(cx);
                            v.update(cx, |x, cx| x.play_once(&owner, &url, cx));
                            cx.notify();
                        }
                    });
                })
                .detach();
            } else if self.clip_for.is_empty() {
                // check again once the wait is up
                busy = true;
            }
        }
        let clip = self.clip.as_ref().and_then(|(o, _)| {
            let o = o.clone();
            let vid = crate::ui::video::get(cx);
            vid.update(cx, |x, _| {
                let f = x.frame(&o, window)?;
                Some((f, x.since.map_or(0., |t| (t.elapsed().as_secs_f32() / 1.2).min(1.))))
            })
        });
        busy |= clip.as_ref().map_or(false, |c| c.1 < 1.);

        let mut root = div().id("tv").absolute().top_0().left_0().size_full().occlude().bg(gpui::black()).overflow_hidden().text_color(theme::white(0.96));
        // the art (and the trailer through it)
        root = root.children(self.bg.0.clone().filter(|_| fade < 1.).map(|a| img(a).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)));
        match &clip {
            Some((p, a)) => {
                for el in crate::ui::video::layers(art.clone(), p, *a, 0.) {
                    root = root.child(div().absolute().top_0().left_0().size_full().opacity(fade).child(el));
                }
            }
            None => {
                root = root.children(art.clone().map(|a| img(a).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).opacity(fade)));
            }
        }
        // shade: the left for words, the bottom for rows (deeper when browsing)
        let deep = if self.row > 0 || self.page.is_some() { 1. } else { 0. };
        root = root
            .child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(90., linear_color_stop(theme::black(0.82), 0.), linear_color_stop(theme::black(0.), 0.66))))
            .child(div().absolute().left_0().bottom_0().w_full().h(px(vh * (0.55 + 0.25 * deep))).bg(linear_gradient(180., linear_color_stop(theme::black(0.), 0.), linear_color_stop(theme::black(0.92), 1.))));

        if let Some(p) = &self.page {
            root = root.child(self.page_view(p, u, g, vw, vh));
        } else {
            // the words: the billboard (with buttons), or the focused title
            if let Some(m) = cur.as_ref() {
                let a = ease(self.focus_at.elapsed().as_secs_f32() / 0.35);
                busy |= a < 1.;
                let top = if on_hero || self.side && self.row == 0 { vh * 0.36 } else { vh * 0.12 };
                let mut words = div()
                    .absolute()
                    .left(px(g))
                    .top(px(top + (1. - a) * 14. * u))
                    .w(px(820. * u))
                    .opacity(a)
                    .flex()
                    .flex_col()
                    .gap(px(18. * u))
                    .child(logo_or_name(m, if self.row == 0 { 190. * u } else { 130. * u }, 620. * u, u))
                    .child(div().text_size(px(22. * u)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.78)).child(facts(m)))
                    .children(m.description.clone().map(|d| div().max_w(px(760. * u)).text_size(px(23. * u)).line_height(px(33. * u)).line_clamp(if self.row == 0 { 3 } else { 2 }).text_ellipsis().text_color(theme::white(0.86)).child(d)));
                if self.row == 0 {
                    words = words.child(div().mt(px(10. * u)).flex().gap(px(16. * u)).child(glass_btn(Some("Play"), "play", !self.side, u)).child(glass_btn(None, "plus", false, u)).child(glass_btn(None, "info", false, u)));
                    // which billboard: dots
                    if self.heroes.len() > 1 {
                        words = words.child(div().mt(px(18. * u)).flex().gap(px(10. * u)).children((0..self.heroes.len()).map(|i| {
                            div().h(px(8. * u)).w(px(if i == self.hero_i { 28. } else { 8. } * u)).rounded_full().bg(theme::white(if i == self.hero_i { 0.95 } else { 0.35 }))
                        })));
                    }
                }
                root = root.child(words);
            }
            // the rows: the focused one sits just under the words
            let heights: Vec<f32> = rows.iter().map(|r| if r.wide { 340. * u } else { 420. * u }).collect();
            let target = if self.row == 0 { vh * 0.86 } else { vh * 0.47 - heights[..self.row - 1].iter().sum::<f32>() };
            if self.y < 0. {
                self.y = target;
            }
            self.y += (target - self.y) * (dt * 11.).min(1.);
            busy |= (target - self.y).abs() > 0.5;
            let mut col = div().absolute().left_0().top(px(self.y)).w_full().flex().flex_col();
            for (ri, r) in rows.iter().enumerate() {
                let focused_row = self.row == ri + 1 && !self.side;
                // rows that went up under the words fade away
                let gone = self.row > ri + 1;
                let c = (*self.cols.get(&r.key).unwrap_or(&0)).min(r.items.len().saturating_sub(1));
                let (cw, ch) = if r.wide { (400. * u, 225. * u) } else { (200. * u, 300. * u) };
                let gap = 32. * u;
                let tx = (c as f32 - 1.).max(0.) * (cw + gap);
                let x = self.xs.entry(r.key.clone()).or_insert(tx);
                *x += (tx - *x) * (dt * 12.).min(1.);
                busy |= (tx - *x).abs() > 0.5;
                let x = *x;
                let lift = if focused_row { ease(self.focus_at.elapsed().as_secs_f32() / 0.2) } else { 0. };
                busy |= focused_row && lift < 1.;
                let cards = r.items.iter().enumerate().map(|(i, m)| {
                    let on = focused_row && i == c;
                    let s = if on { 1. + 0.1 * lift } else { 1. };
                    let (w, h) = (cw * s, ch * s);
                    let pic = if r.wide { m.background.clone().or(m.poster.clone()) } else { m.poster.clone() };
                    let pic = pic.map(|p| crate::net::fit(&sized(&p, if r.wide { "w780" } else { "w342" }), w));
                    let mut card = div()
                        .absolute()
                        .left(px((cw - w) / 2.))
                        .top(px((ch - h) / 2.))
                        .w(px(w))
                        .h(px(h))
                        .rounded(px(18. * u))
                        .overflow_hidden()
                        .bg(theme::surface2())
                        .when(on, |d| d.shadow(vec![gpui::BoxShadow { color: theme::black(0.7), offset: gpui::point(px(0.), px(26. * u)), blur_radius: px(44. * u), spread_radius: px(0.), inset: false }]))
                        .child(div().absolute().top_0().left_0().size_full().p(px(14. * u)).flex().items_end().text_size(px(22. * u)).font_weight(FontWeight::BLACK).line_clamp(3).child(m.name.clone()))
                        .children(pic.map(|p| img(p).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)));
                    if r.wide {
                        // up next: the logo, what's on, how far in
                        let (prog, sub) = r.extra.get(i).cloned().unwrap_or((None, None));
                        card = card
                            .child(div().absolute().left_0().bottom_0().w_full().h(px(h * 0.6)).bg(linear_gradient(180., linear_color_stop(theme::black(0.), 0.), linear_color_stop(theme::black(0.85), 1.))))
                            .children(m.logo.clone().map(|l| img(crate::net::logo(&sized(&l, "medium"), w * 0.5)).absolute().left(px(18. * u)).bottom(px(46. * u)).h(px(54. * u)).max_w(px(w * 0.55)).object_fit(ObjectFit::Contain)))
                            .children(sub.map(|t| {
                                let t = t.replace('\t', "  ·  ");
                                div().absolute().left(px(18. * u)).bottom(px(18. * u)).text_size(px(17. * u)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.85)).child(t.trim_matches(|c| c == ' ' || c == '·').to_string())
                            }))
                            .children(prog.map(|p| div().absolute().left_0().bottom_0().w_full().h(px(5. * u)).bg(theme::white(0.2)).child(div().h_full().w(gpui::relative(p.clamp(0., 1.))).bg(theme::white(0.95)))));
                    }
                    if on {
                        // the light catching the glass as it lifts
                        card = card.child(div().absolute().top_0().left_0().size_full().opacity(lift).bg(linear_gradient(135., linear_color_stop(theme::white(0.24), 0.), linear_color_stop(theme::white(0.), 0.45))));
                    }
                    div()
                        .flex_none()
                        .w(px(cw))
                        .flex()
                        .flex_col()
                        .gap(px(14. * u))
                        .child(div().relative().w(px(cw)).h(px(ch)).child(card))
                        // the focused poster's name under it
                        .child(div().h(px(28. * u)).px(px(4. * u)).text_size(px(19. * u)).font_weight(FontWeight::SEMIBOLD).line_clamp(1).text_ellipsis().text_color(theme::white(if on { 0.95 } else { 0. })).child(if r.wide { String::new() } else { m.name.clone() }))
                });
                col = col.child(
                    div()
                        .h(px(heights[ri]))
                        .flex()
                        .flex_col()
                        .gap(px(18. * u))
                        .opacity(if gone { 0. } else if focused_row || self.row == 0 { 1. } else { 0.82 })
                        .child(div().pl(px(g)).text_size(px(26. * u)).font_weight(FontWeight::BOLD).text_color(theme::white(if focused_row { 0.95 } else { 0.7 })).child(r.title.clone()))
                        .child(div().relative().w_full().h(px(ch + 42. * u)).child(div().absolute().top_0().left(px(g - x)).flex().gap(px(gap)).children(cards))),
                );
            }
            root = root.child(col);
        }

        // the sidebar: glass, sliding out from the left
        root = root.child(self.sidebar(u, vh));
        if busy {
            window.request_animation_frame();
        } else if on_hero && self.clip_for.is_empty() {
            // the trailer's wait: look again in a moment
            cx.spawn(async move |this, cx| {
                crate::timer::after(std::time::Duration::from_millis(400)).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        root
    }
}

impl Tv {
    fn sidebar(&self, u: f32, vh: f32) -> AnyElement {
        let open = self.side;
        let cur = match self.section {
            Section::Home => 0,
            Section::Shows => 1,
            Section::Movies => 2,
            Section::List => 3,
        };
        let w = if open { 380. * u } else { 84. * u };
        let items = SIDE.iter().enumerate().map(|(i, (ic, label))| {
            let focus = open && i == self.side_i;
            let here = i == cur;
            let fg: Hsla = if focus { gpui::black() } else { theme::white(if here { 0.98 } else { 0.7 }) };
            div()
                .h(px(62. * u))
                .px(px(20. * u))
                .rounded_full()
                .flex()
                .items_center()
                .gap(px(18. * u))
                .when(focus, |d| d.bg(theme::white(0.92)))
                .when(!focus && here && open, |d| d.bg(theme::white(0.12)))
                .child(if here { crate::ui::icon_fill(ic, 28. * u) } else { icon(ic, 28. * u) }.text_color(fg))
                .when(open, |d| d.child(div().text_size(px(23. * u)).font_weight(if here { FontWeight::BOLD } else { FontWeight::MEDIUM }).text_color(fg).child(label.to_string())))
        });
        let panel = glass(div(), u, false)
            .bg(theme::black(if open { 0.55 } else { 0.25 }))
            .w(px(w))
            .rounded(px(34. * u))
            .p(px(11. * u))
            .flex()
            .flex_col()
            .gap(px(6. * u))
            .children(items);
        div()
            .absolute()
            .left(px(26. * u))
            .top(px(0.))
            .h(px(vh))
            .flex()
            .items_center()
            .opacity(if open || self.row == 0 { 1. } else { 0. })
            .child(panel)
            .into_any_element()
    }

    fn page_view(&self, p: &Page, u: f32, g: f32, vw: f32, vh: f32) -> AnyElement {
        let m = &p.meta;
        let a = ease(self.focus_at.elapsed().as_secs_f32() / 0.35);
        let series = !p.eps.is_empty();
        let in_season: Vec<&Value> = p.eps.iter().filter(|e| e["season"].as_i64() == Some(p.season)).collect();
        // play: where you are, or the start
        let play_label = match (&p.resume, series) {
            (Some((v, t)), true) => {
                let parts: Vec<&str> = v.split(':').collect();
                if parts.len() == 3 { format!("{} S{} E{}", if *t > 30. { "Resume" } else { "Play" }, parts[1], parts[2]) } else { "Play".into() }
            }
            (Some((_, t)), false) if *t > 30. => "Resume".into(),
            (_, true) => "Play S1 E1".into(),
            _ => "Play".into(),
        };
        let on = |line: usize, i: usize| p.line == line && p.col[line] == i;
        let mut col = div()
            .absolute()
            .left(px(g))
            .top(px(vh * if series { 0.14 } else { 0.40 }))
            .w(px(vw - g * 2.))
            .opacity(a)
            .flex()
            .flex_col()
            .gap(px(20. * u))
            .child(logo_or_name(m, 170. * u, 620. * u, u))
            .child(div().text_size(px(22. * u)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.78)).child(facts(m)))
            .children(m.description.clone().map(|d| div().max_w(px(820. * u)).text_size(px(23. * u)).line_height(px(33. * u)).line_clamp(3).text_ellipsis().text_color(theme::white(0.86)).child(d)))
            .child(div().mt(px(8. * u)).flex().gap(px(16. * u)).child(glass_btn(Some(&play_label), "play", on(0, 0), u)).child(glass_btn(Some("My List"), "plus", on(0, 1), u)));
        if series {
            col = col.child(div().mt(px(26. * u)).flex().gap(px(12. * u)).children(p.seasons.iter().enumerate().map(|(i, s)| {
                let f = on(1, i);
                let here = *s == p.season;
                div()
                    .h(px(50. * u))
                    .px(px(24. * u))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .text_size(px(20. * u))
                    .font_weight(FontWeight::SEMIBOLD)
                    .when(f, |d| d.bg(theme::white(0.92)).text_color(gpui::black()))
                    .when(!f && here, |d| d.bg(theme::white(0.16)))
                    .when(!f && !here, |d| d.text_color(theme::white(0.6)))
                    .child(format!("Season {s}"))
            })));
            let (cw, ch) = (380. * u, 214. * u);
            let gap = 30. * u;
            let c = p.col[2];
            let x = (c as f32 - 1.).max(0.) * (cw + gap);
            let cards = in_season.iter().enumerate().map(|(i, e)| {
                let f = on(2, i);
                let s = if f { 1.08 } else { 1. };
                let (w, h) = (cw * s, ch * s);
                let id = e["id"].as_str().unwrap_or("");
                let seen = p.seen.iter().any(|x| x == id);
                let still = e["thumbnail"].as_str().map(|t| crate::net::fit(t, w));
                let n = e["episode"].as_i64().or(e["number"].as_i64()).unwrap_or(0);
                let name = e["name"].as_str().or(e["title"].as_str()).unwrap_or("").to_string();
                div()
                    .flex_none()
                    .w(px(cw))
                    .flex()
                    .flex_col()
                    .gap(px(12. * u))
                    .child(
                        div().relative().w(px(cw)).h(px(ch)).child(
                            div()
                                .absolute()
                                .left(px((cw - w) / 2.))
                                .top(px((ch - h) / 2.))
                                .w(px(w))
                                .h(px(h))
                                .rounded(px(16. * u))
                                .overflow_hidden()
                                .bg(theme::surface2())
                                .when(f, |d| d.shadow(vec![gpui::BoxShadow { color: theme::black(0.7), offset: gpui::point(px(0.), px(22. * u)), blur_radius: px(40. * u), spread_radius: px(0.), inset: false }]))
                                .children(still.map(|t| img(t).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
                                .when(seen, |d| d.child(div().absolute().top_0().left_0().size_full().bg(theme::black(0.45))).child(div().absolute().top(px(12. * u)).right(px(12. * u)).child(icon("check", 26. * u))))
                                .when(f, |d| d.child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(135., linear_color_stop(theme::white(0.22), 0.), linear_color_stop(theme::white(0.), 0.45))))),
                        ),
                    )
                    .child(div().text_size(px(17. * u)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.6)).child(format!("Episode {n}")))
                    .child(div().text_size(px(20. * u)).font_weight(FontWeight::SEMIBOLD).line_clamp(1).text_ellipsis().text_color(theme::white(if f { 0.98 } else { 0.8 })).child(name))
            });
            col = col.child(div().relative().w_full().h(px(ch + 90. * u)).child(div().absolute().top_0().left(px(-x)).flex().gap(px(gap)).children(cards)));
        }
        col.into_any_element()
    }
}
