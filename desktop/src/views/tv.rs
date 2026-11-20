//! TV mode: couch from across the room, after the Apple TV app (tvOS 26).
//! Full screen, the focused title's art filling it, tall "cinematic" posters
//! that lift when focused, glass buttons and a glass sidebar, search with an
//! on-screen keyboard. Arrows, a controller (d-pad / stick, A, B, Y, Start)
//! or the mouse (point, click, wheel) get everywhere; Enter plays.
use crate::api::{self, sized, Meta};
use crate::theme;
use crate::ui::icon;
use crate::views::home::Home;
use crate::{host, Nav};
use futures::StreamExt;
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, App, ClickEvent, Context, FontWeight, Hsla, ObjectFit, SharedString, WeakEntity, Window};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq)]
pub enum Section {
    Home,
    Shows,
    Movies,
    List,
    Search,
}

const SIDE: &[(&str, &str)] = &[("search", "Search"), ("home", "Home"), ("tv", "Shows"), ("film", "Movies"), ("bookmark", "My List"), ("settings", "Settings"), ("close", "Exit TV mode")];
const KEYS: &str = "abcdefghijklmnopqrstuvwxyz1234567890";
const KB_COLS: usize = 6;
/// the keyboard's last line: space, delete, clear
const KB_WIDE: [&str; 3] = ["space", "delete", "clear"];

struct Row {
    key: String,
    title: String,
    items: Vec<Meta>,
    /// (progress, "S2 E4\t12m left") for up next
    extra: Vec<(Option<f32>, Option<String>)>,
    /// landscape cards (up next), else tall posters
    wide: bool,
}

/// a title opened (Y / i, a click, or Enter on a show): play, list, episodes
struct Page {
    meta: Meta,
    eps: Vec<Value>,
    seasons: Vec<i64>,
    season: i64,
    seen: Vec<String>,
    /// where you are in it: (video id, seconds in, length)
    resume: Option<(String, f64, f64)>,
    /// 0 the buttons, 1 the seasons, 2 the episodes
    line: usize,
    col: [usize; 3],
    loading: bool,
}

#[derive(Default)]
struct Find {
    query: String,
    results: Vec<Meta>,
    searching: bool,
    req: u64,
    /// on the keyboard (Some(key index)) or in the results (None)
    kb: Option<usize>,
    hit: usize,
    /// what was searched last (typing waits a moment before searching)
    asked: String,
    typed_at: Option<Instant>,
}

pub struct Tv {
    pub on: bool,
    homes: [Option<WeakEntity<Home>>; 3],
    section: Section,
    section_at: Instant,
    lists: Vec<Row>,
    lists_loading: bool,
    heroes: Vec<Meta>,
    hero_i: usize,
    side: bool,
    side_i: usize,
    /// 0: the billboard, 1..: the rows
    row: usize,
    cols: HashMap<String, usize>,
    page: Option<Page>,
    page_at: Instant,
    find: Find,
    /// the backdrop: the one before, the one now, when it changed
    bg: (Option<String>, Option<String>, Instant),
    focus_at: Instant,
    /// eased scroll: rows up/down, each row sideways, episodes sideways
    y: f32,
    xs: HashMap<String, f32>,
    last: Instant,
    clip: Option<(String, String)>,
    /// the billboard waited on this title before its trailer
    clip_for: String,
    /// the mouse moved lately (it may move focus; buttons for it show)
    mouse_at: Option<Instant>,
    /// the wheel, gathered until it's a step
    wheel: (f32, f32),
}

const OWNER: &str = "tv:";

fn ease(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

/// a click that runs on TV mode
fn act(me: &WeakEntity<Tv>, f: impl Fn(&mut Tv, &mut Context<Tv>) + 'static) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let me = me.clone();
    move |_, _, cx| {
        cx.stop_propagation();
        let _ = me.update(cx, |t, cx| f(t, cx));
    }
}

/// pointing at something moves focus to it, only when the mouse itself moved
/// (not when the rows slid under a still pointer)
fn point(me: &WeakEntity<Tv>, f: impl Fn(&mut Tv, &mut Context<Tv>) + 'static) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    let me = me.clone();
    move |on, _, cx| {
        if !*on {
            return;
        }
        let _ = me.update(cx, |t, cx| {
            if t.mouse_at.map_or(false, |m| m.elapsed().as_millis() < 160) {
                f(t, cx);
            }
        });
    }
}

impl Tv {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            on: false,
            homes: [None, None, None],
            section: Section::Home,
            section_at: Instant::now(),
            lists: vec![],
            lists_loading: false,
            heroes: vec![],
            hero_i: 0,
            side: false,
            side_i: 1,
            row: 0,
            cols: HashMap::new(),
            page: None,
            page_at: Instant::now(),
            find: Find { kb: Some(0), ..Default::default() },
            bg: (None, None, Instant::now()),
            focus_at: Instant::now(),
            y: -1.,
            xs: HashMap::new(),
            last: Instant::now(),
            clip: None,
            clip_for: String::new(),
            mouse_at: None,
            wheel: (0., 0.),
        }
    }

    pub fn enter(&mut self, homes: [Option<WeakEntity<Home>>; 3], cx: &mut Context<Self>) {
        self.on = true;
        self.homes = homes;
        self.page = None;
        self.side = false;
        self.row = 0;
        self.y = -1.;
        self.section_at = Instant::now();
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
        self.lists_loading = true;
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
                t.lists_loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn rows(&self, cx: &App) -> Vec<Row> {
        let i = match self.section {
            Section::List => {
                return self.lists.iter().map(|r| Row { key: r.key.clone(), title: r.title.clone(), items: r.items.clone(), extra: vec![], wide: false }).collect();
            }
            Section::Search => return vec![],
            Section::Shows => 1,
            Section::Movies => 2,
            Section::Home => 0,
        };
        let Some(h) = self.homes[i].as_ref().and_then(|h| h.upgrade()) else { return vec![] };
        let (_, rows) = h.read(cx).tv();
        rows.iter()
            .filter(|r| !r.items.is_empty())
            .map(|r| Row { key: r.key.clone(), title: r.title.clone(), items: r.items.clone(), extra: r.extra.clone(), wide: r.key == "cw" })
            .collect()
    }

    /// the billboard: the featured titles, or (none yet) the first row's
    fn hero_list(&self, rows: &[Row]) -> Vec<Meta> {
        if !self.heroes.is_empty() {
            return self.heroes.clone();
        }
        rows.iter().find(|r| !r.wide).map(|r| r.items.iter().filter(|m| m.background.is_some()).take(5).cloned().collect()).unwrap_or_default()
    }

    fn focused(&self, rows: &[Row]) -> Option<Meta> {
        if let Some(p) = &self.page {
            return Some(p.meta.clone());
        }
        if self.section == Section::Search {
            return self.find.results.get(self.find.hit).cloned().filter(|_| self.find.kb.is_none()).or_else(|| self.find.results.first().cloned());
        }
        if self.row == 0 {
            let h = self.hero_list(rows);
            return h.get(self.hero_i.min(h.len().saturating_sub(1))).cloned();
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

    fn set_focus(&mut self, row: usize, col: usize, key: Option<String>, cx: &mut Context<Self>) {
        let same = self.row == row && key.as_ref().map_or(true, |k| self.cols.get(k) == Some(&col)) && !self.side;
        self.side = false;
        self.row = row;
        if let Some(k) = key {
            self.cols.insert(k, col);
        }
        if !same {
            self.moved(cx);
        }
    }

    fn play(&mut self, m: &Meta, cx: &mut Context<Self>) {
        self.stop_clip(cx);
        Nav::play(m, cx);
    }

    /// a key (or a controller button, already turned into one)
    pub fn press(&mut self, k: &str, cx: &mut Context<Self>) {
        // keys and buttons hide what's only for the mouse
        self.mouse_at = None;
        if self.page.is_some() {
            return self.press_page(k, cx);
        }
        if self.side {
            match k {
                "up" => self.side_i = self.side_i.saturating_sub(1),
                "down" => self.side_i = (self.side_i + 1).min(SIDE.len() - 1),
                "right" | "escape" | "backspace" => self.side = false,
                "enter" | "space" | "play" => return self.side_pick(self.side_i, cx),
                _ => return,
            }
            return self.moved(cx);
        }
        if self.section == Section::Search {
            return self.press_find(k, cx);
        }
        let rows = self.rows(cx);
        let heroes = self.hero_list(&rows);
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
            "left" | "prev" => {
                if self.row == 0 {
                    if self.hero_i == 0 || heroes.is_empty() {
                        if k == "left" {
                            self.side = true;
                        }
                    } else {
                        self.hero_i -= 1;
                    }
                } else if let Some(r) = rows.get(self.row - 1) {
                    let c = self.cols.entry(r.key.clone()).or_insert(0);
                    if *c == 0 {
                        if k == "left" {
                            self.side = true;
                        }
                    } else {
                        *c -= 1;
                    }
                }
            }
            "right" | "next" => {
                if self.row == 0 {
                    if !heroes.is_empty() {
                        self.hero_i = (self.hero_i + 1) % heroes.len();
                    }
                } else if let Some(r) = rows.get(self.row - 1) {
                    let c = self.cols.entry(r.key.clone()).or_insert(0);
                    *c = (*c + 1).min(r.items.len().saturating_sub(1));
                }
            }
            "enter" => return self.choose(&rows, cx),
            "space" | "play" => {
                if let Some(m) = self.focused(&rows) {
                    return self.play(&m, cx);
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
            // typing goes to search
            k if k.chars().count() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) => {
                self.side_pick(0, cx);
                return self.press_find(k, cx);
            }
            _ => return,
        }
        self.moved(cx);
    }

    /// Enter / a click on what's focused: up next and films play; a show opens
    fn choose(&mut self, rows: &[Row], cx: &mut Context<Self>) {
        let Some(m) = self.focused(rows) else { return };
        let up_next = self.row > 0 && rows.get(self.row - 1).map_or(false, |r| r.wide);
        if m.kind == "series" && !up_next && self.row > 0 {
            return self.open_page(m, cx);
        }
        self.play(&m, cx);
    }

    fn side_pick(&mut self, i: usize, cx: &mut Context<Self>) {
        self.side_i = i;
        let sec = match i {
            0 => Some(Section::Search),
            1 => Some(Section::Home),
            2 => Some(Section::Shows),
            3 => Some(Section::Movies),
            4 => Some(Section::List),
            _ => None,
        };
        self.side = false;
        if let Some(s) = sec {
            if s != self.section {
                self.section = s;
                self.section_at = Instant::now();
                self.row = 0;
                self.y = -1.;
                self.xs.clear();
                if s == Section::List {
                    self.load_lists(cx);
                }
                if s == Section::Search {
                    self.find.kb = Some(self.find.kb.unwrap_or(0));
                } else {
                    self.load_heroes(cx);
                }
            }
            return self.moved(cx);
        }
        // settings, or out: back to the desktop look
        self.leave(cx);
        Nav::tv_off(cx);
        if i == 5 {
            Nav::go(crate::Route::Settings, cx);
        }
    }

    // ---------------------------------------------------------------- search

    fn kb_len() -> usize {
        KEYS.len() + KB_WIDE.len()
    }

    fn type_key(&mut self, key: &str, cx: &mut Context<Self>) {
        match key {
            "space" => self.find.query.push(' '),
            "delete" => {
                self.find.query.pop();
            }
            "clear" => self.find.query.clear(),
            c => self.find.query.push_str(c),
        }
        // search once typing pauses
        self.find.typed_at = Some(Instant::now());
        cx.spawn(async move |this, cx| {
            crate::timer::after(std::time::Duration::from_millis(420)).await;
            let _ = this.update(cx, |t, cx| {
                if t.find.typed_at.map_or(false, |a| a.elapsed().as_millis() >= 400) {
                    t.find.typed_at = None;
                    t.search(cx);
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        let q = self.find.query.trim().to_string();
        if q == self.find.asked {
            return;
        }
        self.find.asked = q.clone();
        self.find.req += 1;
        let my = self.find.req;
        self.find.results.clear();
        self.find.hit = 0;
        if q.chars().count() < 2 {
            self.find.searching = false;
            cx.notify();
            return;
        }
        self.find.searching = true;
        let req_id = format!("tv{my}-{}", std::process::id());
        let mut rx = host::listen("search");
        let rid = req_id.clone();
        cx.spawn(async move |this, cx| {
            let _ = api::call::<Value>("search_start", json!({ "query": q, "reqId": rid })).await;
            while let Some(c) = rx.next().await {
                if c["reqId"].as_str() != Some(req_id.as_str()) {
                    continue;
                }
                let done = c["finished"].as_bool() == Some(true);
                let metas: Vec<Meta> = c["metas"].as_array().into_iter().flatten().filter_map(|m| serde_json::from_value(m.clone()).ok()).collect();
                let alive = this
                    .update(cx, |t, cx| {
                        if t.find.req != my {
                            return false;
                        }
                        for m in metas {
                            if !m.name.is_empty() && m.poster.is_some() && !t.find.results.iter().any(|x| x.id == m.id) {
                                t.find.results.push(m);
                            }
                        }
                        if done {
                            t.find.searching = false;
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if done || !alive {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn press_find(&mut self, k: &str, cx: &mut Context<Self>) {
        // a real keyboard types straight in
        if k.chars().count() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) {
            return self.type_key(&k.to_lowercase(), cx);
        }
        let n = self.find.results.len();
        let per = self.results_cols();
        match (self.find.kb, k) {
            (_, "backspace") => return self.type_key("delete", cx),
            (_, "space") if self.find.kb.is_some() || n == 0 => return self.type_key("space", cx),
            (Some(i), "enter") => {
                let key = if i < KEYS.len() { KEYS[i..i + 1].to_string() } else { KB_WIDE[i - KEYS.len()].to_string() };
                return self.type_key(&key, cx);
            }
            (Some(i), "left") => {
                if i < KEYS.len() && i % KB_COLS == 0 || i == KEYS.len() {
                    self.side = true;
                } else {
                    self.find.kb = Some(i - 1);
                }
            }
            (Some(i), "right") => {
                let last_in_line = if i < KEYS.len() { i % KB_COLS == KB_COLS - 1 } else { i == Self::kb_len() - 1 };
                if last_in_line {
                    if n > 0 {
                        self.find.kb = None;
                    }
                } else {
                    self.find.kb = Some(i + 1);
                }
            }
            (Some(i), "up") => {
                if i >= KEYS.len() {
                    // from the wide keys back into the letters (the last line)
                    let col = (i - KEYS.len()) * 2;
                    self.find.kb = Some(KEYS.len() - KB_COLS + col.min(KB_COLS - 1));
                } else if i >= KB_COLS {
                    self.find.kb = Some(i - KB_COLS);
                }
            }
            (Some(i), "down") => {
                if i + KB_COLS < KEYS.len() {
                    self.find.kb = Some(i + KB_COLS);
                } else if i < KEYS.len() {
                    self.find.kb = Some(KEYS.len() + ((i % KB_COLS) / 2).min(KB_WIDE.len() - 1));
                }
            }
            (Some(_), "escape") => {
                if self.find.query.is_empty() {
                    self.side = true;
                } else {
                    return self.type_key("clear", cx);
                }
            }
            (None, "left") => {
                if self.find.hit % per == 0 {
                    self.find.kb = Some(KB_COLS - 1);
                } else {
                    self.find.hit -= 1;
                }
            }
            (None, "right") => self.find.hit = (self.find.hit + 1).min(n.saturating_sub(1)),
            (None, "up") => self.find.hit = self.find.hit.saturating_sub(per),
            (None, "down") => self.find.hit = (self.find.hit + per).min(n.saturating_sub(1)),
            (None, "escape") => self.find.kb = Some(KB_COLS - 1),
            (None, "enter" | "i" | "info") => {
                if let Some(m) = self.find.results.get(self.find.hit).cloned() {
                    return self.open_page(m, cx);
                }
            }
            (None, "play") => {
                if let Some(m) = self.find.results.get(self.find.hit).cloned() {
                    return self.play(&m, cx);
                }
            }
            _ => return,
        }
        self.moved(cx);
    }

    fn results_cols(&self) -> usize {
        5
    }

    // ----------------------------------------------------------------- a title

    fn open_page(&mut self, meta: Meta, cx: &mut Context<Self>) {
        self.stop_clip(cx);
        self.side = false;
        self.page = Some(Page { meta: meta.clone(), eps: vec![], seasons: vec![], season: 1, seen: vec![], resume: None, line: 0, col: [0; 3], loading: true });
        self.page_at = Instant::now();
        self.focus_at = Instant::now();
        self.xs.remove("eps");
        cx.spawn(async move |this, cx| {
            let full = api::call::<Value>("meta_get", json!({ "kind": meta.kind, "id": meta.id, "hint": Value::Null })).await.unwrap_or_default();
            let item = api::call::<Value>("library_item", json!({ "id": meta.id })).await.unwrap_or_default();
            let eps: Vec<Value> = crate::playback::ordered(&full).into_iter().filter(|e| e["season"].as_i64().unwrap_or(0) > 0).collect();
            let mut seasons: Vec<i64> = eps.iter().filter_map(|e| e["season"].as_i64()).collect();
            seasons.dedup();
            let st = &item["state"];
            let seen: Vec<String> = st["watchedIds"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            let resume = st["video_id"]
                .as_str()
                .filter(|v| !v.is_empty())
                .map(|v| (v.to_string(), st["timeOffset"].as_f64().unwrap_or(0.) / 1000., st["duration"].as_f64().unwrap_or(0.) / 1000.))
                .or_else(|| (meta.kind == "movie" && st["timeOffset"].as_f64().unwrap_or(0.) > 0.).then(|| (meta.id.clone(), st["timeOffset"].as_f64().unwrap_or(0.) / 1000., st["duration"].as_f64().unwrap_or(0.) / 1000.)));
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
                    // the season you're on, at the episode you're on
                    let at = resume.as_ref().and_then(|(v, _, _)| eps.iter().find(|e| e["id"].as_str() == Some(v.as_str())));
                    p.season = at.and_then(|e| e["season"].as_i64()).or(seasons.first().copied()).unwrap_or(1);
                    p.col[1] = seasons.iter().position(|s| *s == p.season).unwrap_or(0);
                    let in_season: Vec<&Value> = eps.iter().filter(|e| e["season"].as_i64() == Some(p.season)).collect();
                    p.col[2] = at.and_then(|a| in_season.iter().position(|e| e["id"] == a["id"])).unwrap_or(0);
                    p.eps = eps;
                    p.seasons = seasons;
                    p.seen = seen;
                    p.resume = resume;
                    p.loading = false;
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn close_page(&mut self, cx: &mut Context<Self>) {
        self.page = None;
        self.moved(cx);
    }

    fn page_play(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.page.as_ref() else { return };
        let m = p.meta.clone();
        match &p.resume {
            Some((v, t, _)) if v.split(':').count() == 3 => {
                let (v, t) = (v.clone(), *t);
                self.stop_clip(cx);
                crate::playback::quick_play(m, Some(v), (t > 30.).then_some(t), cx);
            }
            _ => self.play(&m, cx),
        }
    }

    fn page_episode(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(p) = self.page.as_ref() else { return };
        let in_season: Vec<&Value> = p.eps.iter().filter(|e| e["season"].as_i64() == Some(p.season)).collect();
        if let Some(id) = in_season.get(i).and_then(|e| e["id"].as_str()) {
            let m = p.meta.clone();
            let id = id.to_string();
            self.stop_clip(cx);
            Nav::play_episode(&m, id, cx);
        }
    }

    fn page_season(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(p) = self.page.as_mut() else { return };
        p.col[1] = i;
        if let Some(s) = p.seasons.get(i).copied() {
            if s != p.season {
                p.season = s;
                p.col[2] = 0;
                self.xs.remove("eps");
            }
        }
        cx.notify();
    }

    fn press_page(&mut self, k: &str, cx: &mut Context<Self>) {
        let Some(p) = self.page.as_mut() else { return };
        let series = !p.eps.is_empty();
        let lines = if series { 3 } else { 1 };
        let in_season = p.eps.iter().filter(|e| e["season"].as_i64() == Some(p.season)).count();
        let len = [2, p.seasons.len(), in_season];
        match k {
            "up" => p.line = p.line.saturating_sub(1),
            "down" => p.line = (p.line + 1).min(lines - 1),
            "left" | "prev" => p.col[p.line] = p.col[p.line].saturating_sub(1),
            "right" | "next" => p.col[p.line] = (p.col[p.line] + 1).min(len[p.line].saturating_sub(1)),
            "escape" | "backspace" => return self.close_page(cx),
            "play" | "space" => return self.page_play(cx),
            "enter" => {
                let (line, col) = (p.line, p.col[p.line]);
                return match (line, col) {
                    (0, 1) => {
                        let m = p.meta.clone();
                        Nav::add(&m, cx)
                    }
                    (0, _) => self.page_play(cx),
                    (1, _) => {}
                    (_, c) => self.page_episode(c, cx),
                };
            }
            _ => return,
        }
        let (line, col) = (p.line, p.col[1]);
        if line == 1 {
            self.page_season(col, cx);
        }
        self.focus_at = Instant::now();
        cx.notify();
    }

    // ------------------------------------------------------------------ wheel

    fn wheel(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        self.mouse_at = Some(Instant::now());
        self.wheel.0 += dx;
        self.wheel.1 += dy;
        let step = 70.;
        let mut keys = vec![];
        while self.wheel.1 <= -step {
            self.wheel.1 += step;
            keys.push("down");
        }
        while self.wheel.1 >= step {
            self.wheel.1 -= step;
            keys.push("up");
        }
        while self.wheel.0 <= -step {
            self.wheel.0 += step;
            keys.push("right");
        }
        while self.wheel.0 >= step {
            self.wheel.0 -= step;
            keys.push("left");
        }
        for k in keys {
            // (the wheel never opens the sidebar or leaves the keyboard)
            if k == "left" && !self.side && self.page.is_none() && self.section != Section::Search {
                let rows = self.rows(cx);
                let at_edge = if self.row == 0 { self.hero_i == 0 } else { rows.get(self.row - 1).map_or(true, |r| self.cols.get(&r.key).copied().unwrap_or(0) == 0) };
                if at_edge {
                    continue;
                }
            }
            let keep = self.mouse_at;
            self.press(k, cx);
            self.mouse_at = keep;
        }
    }
}

// ------------------------------------------------------------------ drawing

/// liquid glass: a pale see-through fill, a bright rim, a soft shadow
fn glass(d: gpui::Div, u: f32, on: bool) -> gpui::Div {
    d.bg(if on { theme::white(0.94) } else { theme::white(0.12) })
        .border_1()
        .border_color(if on { theme::white(1.) } else { theme::white(0.2) })
        .shadow(vec![gpui::BoxShadow { color: theme::black(if on { 0.45 } else { 0.25 }), offset: gpui::point(px(0.), px(12. * u)), blur_radius: px(34. * u), spread_radius: px(0.), inset: false }])
}

/// the bright sheen across the top of glass
fn sheen(u: f32) -> gpui::Div {
    div().absolute().top_0().left_0().w_full().h(px(28. * u)).bg(linear_gradient(180., linear_color_stop(theme::white(0.16), 0.), linear_color_stop(theme::white(0.), 1.)))
}

fn glass_btn(id: &str, label: Option<&str>, ic: &str, on: bool, u: f32) -> gpui::Stateful<gpui::Div> {
    let fg: Hsla = if on { gpui::black() } else { theme::white(0.95) };
    glass(div(), u, on)
        .id(SharedString::from(id.to_string()))
        .relative()
        .overflow_hidden()
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
        .cursor_pointer()
        .when(!on, |d| d.hover(|s| s.bg(theme::white(0.24))).child(sheen(u)))
        .child(icon(ic, 26. * u).text_color(fg))
        .children(label.map(|l| l.to_string()))
}

fn round_glass(id: String, ic: &str, size: f32, u: f32) -> gpui::Stateful<gpui::Div> {
    glass(div(), u, false)
        .id(SharedString::from(id))
        .relative()
        .overflow_hidden()
        .size(px(size))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(theme::white(0.26)))
        .child(sheen(u))
        .child(icon(ic, size * 0.5))
}

fn facts(m: &Meta) -> String {
    let mut f: Vec<String> = vec![if m.kind == "series" { "Series".into() } else { "Movie".into() }];
    if let Some(g) = m.genres.as_ref() {
        f.extend(g.iter().take(2).cloned());
    }
    if let Some(y) = m.year() {
        f.push(y);
    }
    if let Some(r) = m.imdb_rating.as_ref().and_then(|r| r.as_str().map(String::from).or(r.as_f64().map(|x| format!("{x:.1}")))).filter(|r| !r.is_empty()) {
        f.push(format!("★ {r}"));
    }
    f.join("  ·  ")
}

fn logo_or_name(m: &Meta, h: f32, max_w: f32) -> AnyElement {
    let name = m.name.clone();
    let words = move || div().max_w(px(max_w)).text_size(px(h * 0.42)).line_height(px(h * 0.46)).line_clamp(2).font_weight(FontWeight::BLACK).child(name.clone()).into_any_element();
    match m.logo.clone() {
        Some(l) => img(crate::net::logo(&sized(&l, "large"), max_w)).h(px(h)).max_w(px(max_w)).object_fit(ObjectFit::Contain).with_fallback(words).into_any_element(),
        None => words(),
    }
}

/// a card's lift (scaled up from its slot's centre), shadow and sheen
fn lifted(slot_w: f32, slot_h: f32, s: f32, lift: f32, on: bool, r: f32, u: f32) -> gpui::Div {
    let (w, h) = (slot_w * s, slot_h * s);
    div()
        .absolute()
        .left(px((slot_w - w) / 2.))
        .top(px((slot_h - h) / 2.))
        .w(px(w))
        .h(px(h))
        .rounded(px(r))
        .overflow_hidden()
        .bg(theme::white(0.06))
        .when(on, |d| d.shadow(vec![gpui::BoxShadow { color: theme::black(0.7 * lift), offset: gpui::point(px(0.), px(26. * u * lift)), blur_radius: px(44. * u), spread_radius: px(0.), inset: false }]))
}

fn light(lift: f32) -> gpui::Div {
    div().absolute().top_0().left_0().size_full().opacity(lift).bg(linear_gradient(135., linear_color_stop(theme::white(0.24), 0.), linear_color_stop(theme::white(0.), 0.45)))
}

/// grey placeholders while a row's on its way
fn skeleton(n: usize, w: f32, h: f32, gap: f32, g: f32, r: f32) -> AnyElement {
    div().pl(px(g)).flex().gap(px(gap)).children((0..n).map(|i| div().flex_none().w(px(w)).h(px(h)).rounded(px(r)).bg(theme::white(0.07 - i as f32 * 0.008)))).into_any_element()
}

fn clock() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

impl Render for Tv {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vs = window.viewport_size();
        let (vw, vh) = (f32::from(vs.width), f32::from(vs.height));
        let u = (vh / 1080.).max(0.5);
        let g = 120. * u;
        let rows = self.rows(cx);
        let dt = self.last.elapsed().as_secs_f32().min(0.05);
        self.last = Instant::now();
        let mut busy = false;
        let me = cx.entity().downgrade();
        let mouse = self.mouse_at.map_or(false, |m| m.elapsed().as_secs() < 4);

        // the backdrop follows focus (a short cross-fade)
        let cur = self.focused(&rows);
        let art = cur.as_ref().and_then(|m| m.background.clone().or(m.poster.clone())).map(|a| crate::net::fit(&sized(&a, "original"), vw));
        if art != self.bg.1 {
            self.bg = (self.bg.1.take(), art.clone(), Instant::now());
        }
        let fade = ease(self.bg.2.elapsed().as_secs_f32() / 0.5);
        busy |= fade < 1.;

        // the billboard's trailer, after a few seconds on it
        let on_hero = self.row == 0 && self.page.is_none() && !self.side && self.section != Section::Search;
        if let Some(m) = cur.as_ref().filter(|_| on_hero) {
            if self.clip_for != m.id && self.focus_at.elapsed().as_secs_f32() > 2.5 {
                self.clip_for = m.id.clone();
                let m = m.clone();
                cx.spawn(async move |this, cx| {
                    let url = api::call::<Option<String>>("trailer_get", json!({ "kind": m.kind, "id": m.id })).await.ok().flatten().filter(|k| !k.is_empty()).map(|k| format!("https://www.youtube.com/watch?v={k}"));
                    let Some(url) = url else { return };
                    let owner = format!("{OWNER}{}", m.id);
                    let _ = this.update(cx, |t, cx| {
                        if t.clip_for == m.id && t.row == 0 && t.on && t.page.is_none() {
                            t.clip = Some((owner.clone(), url.clone()));
                            let v = crate::ui::video::get(cx);
                            v.update(cx, |x, cx| x.play_once(&owner, &url, cx));
                            cx.notify();
                        }
                    });
                })
                .detach();
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

        let mut root = div()
            .id("tv")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .bg(gpui::black())
            .overflow_hidden()
            .text_color(theme::white(0.96))
            .on_mouse_move(cx.listener(|t, _: &gpui::MouseMoveEvent, _, cx| {
                let fresh = t.mouse_at.map_or(true, |m| m.elapsed().as_secs() >= 4);
                t.mouse_at = Some(Instant::now());
                if fresh {
                    cx.notify();
                }
            }))
            .on_scroll_wheel(cx.listener(|t, e: &gpui::ScrollWheelEvent, _, cx| {
                let d = e.delta.pixel_delta(px(40.));
                let (dx, dy) = (f32::from(d.x), f32::from(d.y));
                // shift + wheel: sideways
                if e.modifiers.shift {
                    t.wheel(dy, 0., cx)
                } else {
                    t.wheel(dx, dy, cx)
                }
            }))
            .on_mouse_down(gpui::MouseButton::Right, cx.listener(|t, _: &gpui::MouseDownEvent, _, cx| {
                // right click (on nothing in particular): back
                t.press("escape", cx);
            }));
        // the art (and the trailer through it)
        root = root.children(self.bg.0.clone().filter(|_| fade < 1.).map(|a| img(a).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)));
        let dim = if self.section == Section::Search && self.page.is_none() { 0.35 } else { 1. };
        match &clip {
            Some((p, a)) => {
                for el in crate::ui::video::layers(art.clone(), p, *a, 0.) {
                    root = root.child(div().absolute().top_0().left_0().size_full().opacity(fade * dim).child(el));
                }
            }
            None => {
                root = root.children(art.clone().map(|a| img(a).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).opacity(fade * dim)));
            }
        }
        // shade: the left for words, the bottom for rows (deeper when browsing)
        let deep = if self.row > 0 || self.page.is_some() { 1. } else { 0. };
        root = root
            .child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(90., linear_color_stop(theme::black(0.85), 0.), linear_color_stop(theme::black(0.), 0.68))))
            .child(div().absolute().left_0().bottom_0().w_full().h(px(vh * (0.55 + 0.3 * deep))).bg(linear_gradient(180., linear_color_stop(theme::black(0.), 0.), linear_color_stop(theme::black(0.94), 1.))))
            .child(div().absolute().top_0().left_0().w_full().h(px(160. * u)).bg(linear_gradient(180., linear_color_stop(theme::black(0.55), 0.), linear_color_stop(theme::black(0.), 1.))));

        if self.page.is_some() {
            let a = ease(self.page_at.elapsed().as_secs_f32() / 0.4);
            busy |= a < 1.;
            let el = self.page_view(u, g, vw, vh, dt, &me, &mut busy);
            root = root.child(div().absolute().top_0().left_0().size_full().opacity(a).children(el));
            // back
            root = root.child(round_glass("tv-page-back".into(), "back", 60. * u, u).absolute().top(px(48. * u)).left(px(48. * u)).on_click(act(&me, |t, cx| t.close_page(cx))));
        } else if self.section == Section::Search {
            root = root.child(self.find_view(u, g, vw, vh, &me, &mut busy));
        } else {
            let sa = ease(self.section_at.elapsed().as_secs_f32() / 0.45);
            busy |= sa < 1.;
            // the words: the billboard (with buttons), or the focused title
            if let Some(m) = cur.as_ref() {
                let a = ease(self.focus_at.elapsed().as_secs_f32() / 0.35);
                busy |= a < 1.;
                let top = if self.row == 0 { vh * 0.30 } else { vh * 0.11 };
                let mut words = div()
                    .absolute()
                    .left(px(g))
                    .top(px(top + (1. - a) * 14. * u))
                    .w(px(860. * u))
                    .opacity(a * sa)
                    .flex()
                    .flex_col()
                    .gap(px(18. * u))
                    .child(logo_or_name(m, if self.row == 0 { 190. * u } else { 130. * u }, 620. * u))
                    .child(div().text_size(px(22. * u)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.78)).child(facts(m)))
                    .children(m.description.clone().map(|d| div().max_w(px(760. * u)).text_size(px(23. * u)).line_height(px(33. * u)).line_clamp(if self.row == 0 { 3 } else { 2 }).text_ellipsis().text_color(theme::white(0.86)).child(d)));
                if self.row == 0 {
                    let (m1, m2, m3) = (m.clone(), m.clone(), m.clone());
                    words = words.child(
                        div()
                            .mt(px(10. * u))
                            .flex()
                            .gap(px(16. * u))
                            .child(glass_btn("tv-hero-play", Some("Play"), "play", !self.side, u).on_click(act(&me, move |t, cx| t.play(&m1, cx))))
                            .child(glass_btn("tv-hero-add", None, "plus", false, u).on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                Nav::add(&m2, cx)
                            }))
                            .child(glass_btn("tv-hero-info", None, "info", false, u).on_click(act(&me, move |t, cx| t.open_page(m3.clone(), cx)))),
                    );
                    // which billboard: dots (clickable)
                    let n = self.hero_list(&rows).len();
                    if n > 1 {
                        words = words.child(div().mt(px(18. * u)).flex().items_center().gap(px(4. * u)).children((0..n).map(|i| {
                            let on = i == self.hero_i;
                            div()
                                .id(SharedString::from(format!("tv-dot-{i}")))
                                .p(px(5. * u))
                                .cursor_pointer()
                                .on_click(act(&me, move |t, cx| {
                                    t.hero_i = i;
                                    t.row = 0;
                                    t.moved(cx);
                                }))
                                .child(div().h(px(8. * u)).w(px(if on { 30. } else { 8. } * u)).rounded_full().bg(theme::white(if on { 0.95 } else { 0.35 })))
                        })));
                    }
                }
                root = root.child(words);
                // the mouse: arrows at the billboard's sides
                if self.row == 0 && mouse && self.hero_list(&rows).len() > 1 {
                    root = root
                        .child(round_glass("tv-hero-prev".into(), "chevronLeft", 56. * u, u).absolute().right(px(120. * u)).top(px(vh * 0.42)).opacity(0.85).on_click(act(&me, |t, cx| {
                            let keep = t.mouse_at;
                            t.press("prev", cx);
                            t.mouse_at = keep;
                        })))
                        .child(round_glass("tv-hero-next".into(), "chevron", 56. * u, u).absolute().right(px(48. * u)).top(px(vh * 0.42)).opacity(0.85).on_click(act(&me, |t, cx| {
                            let keep = t.mouse_at;
                            t.press("next", cx);
                            t.mouse_at = keep;
                        })));
                }
            }
            root = root.child(self.rows_view(&rows, u, g, vh, dt, sa, mouse, &me, &mut busy));
        }

        // the time, top right (like the TV's own)
        root = root.child(div().absolute().top(px(52. * u)).right(px(64. * u)).text_size(px(26. * u)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.85)).child(clock()));
        // the sidebar: glass, sliding out from the left
        root = root.child(self.sidebar(u, vh, mouse, &me));
        // what the buttons do (not while the mouse is about)
        if !mouse {
            let hints: &[(&str, &str)] = if self.page.is_some() {
                &[("A", "select"), ("B", "back"), ("▶", "play")]
            } else if self.section == Section::Search {
                &[("A", "type"), ("B", "clear"), ("Y", "details")]
            } else {
                &[("A", "select"), ("Y", "details"), ("▶", "play"), ("B", "menu")]
            };
            root = root.child(div().absolute().bottom(px(36. * u)).right(px(64. * u)).flex().gap(px(26. * u)).opacity(0.7).children(hints.iter().map(|(b, w)| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(10. * u))
                    .text_size(px(17. * u))
                    .text_color(theme::white(0.8))
                    .child(div().size(px(28. * u)).rounded_full().border_1().border_color(theme::white(0.5)).flex().items_center().justify_center().text_size(px(13. * u)).font_weight(FontWeight::BOLD).child(b.to_string()))
                    .child(w.to_string())
            })));
        }
        if busy {
            window.request_animation_frame();
        } else if (on_hero && self.clip_for.is_empty()) || mouse {
            // the trailer's wait, the mouse's buttons going away: look again soon
            cx.spawn(async move |this, cx| {
                crate::timer::after(std::time::Duration::from_millis(500)).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        root
    }
}

impl Tv {
    #[allow(clippy::too_many_arguments)]
    fn rows_view(&mut self, rows: &[Row], u: f32, g: f32, vh: f32, dt: f32, sa: f32, mouse: bool, me: &WeakEntity<Tv>, busy: &mut bool) -> AnyElement {
        let loading = rows.is_empty();
        let heights: Vec<f32> = rows.iter().map(|r| if r.wide { 340. * u } else { 430. * u }).collect();
        let target = if self.row == 0 { vh * 0.84 } else { vh * 0.47 - heights[..(self.row - 1).min(heights.len())].iter().sum::<f32>() };
        if self.y < 0. {
            self.y = target;
        }
        self.y += (target - self.y) * (dt * 11.).min(1.);
        *busy |= (target - self.y).abs() > 0.5;
        let mut col = div().absolute().left_0().top(px(self.y + (1. - sa) * 30. * u)).w_full().opacity(sa).flex().flex_col();
        if loading {
            let msg = if self.section == Section::List && !self.lists_loading { Some("nothing on your lists yet") } else { None };
            col = col.child(match msg {
                Some(m) => div().pl(px(g)).text_size(px(22. * u)).text_color(theme::white(0.6)).child(m).into_any_element(),
                None => skeleton(7, 200. * u, 300. * u, 32. * u, g, 18. * u),
            });
        }
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
            *busy |= (tx - *x).abs() > 0.5;
            let x = *x;
            let lift = if focused_row { ease(self.focus_at.elapsed().as_secs_f32() / 0.22) } else { 0. };
            *busy |= focused_row && lift < 1.;
            let cards = r.items.iter().enumerate().map(|(i, m)| {
                let on = focused_row && i == c;
                let s = if on { 1. + 0.1 * lift } else { 1. };
                let pic = if r.wide { m.background.clone().or(m.poster.clone()) } else { m.poster.clone() };
                let pic = pic.map(|p| crate::net::fit(&sized(&p, if r.wide { "w780" } else { "w342" }), cw * 1.1));
                let mut card = lifted(cw, ch, s, lift, on, 18. * u, u)
                    .child(div().absolute().top_0().left_0().size_full().p(px(14. * u)).flex().items_end().text_size(px(22. * u)).font_weight(FontWeight::BLACK).line_clamp(3).child(m.name.clone()))
                    .children(pic.map(|p| img(p).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)));
                if r.wide {
                    // up next: the logo, what's on, how far in
                    let (prog, sub) = r.extra.get(i).cloned().unwrap_or((None, None));
                    let w = cw * s;
                    let fresh = sub.as_deref().map_or(false, |t| t.ends_with("new episode"));
                    card = card
                        .child(div().absolute().left_0().bottom_0().w_full().h(px(ch * s * 0.62)).bg(linear_gradient(180., linear_color_stop(theme::black(0.), 0.), linear_color_stop(theme::black(0.88), 1.))))
                        .children(m.logo.clone().map(|l| img(crate::net::logo(&sized(&l, "medium"), w * 0.5)).absolute().left(px(18. * u)).bottom(px(48. * u)).h(px(54. * u)).max_w(px(w * 0.55)).object_fit(ObjectFit::Contain)))
                        .children(sub.map(|t| {
                            let t = t.split('\t').filter(|x| !x.is_empty()).collect::<Vec<_>>().join("  ·  ");
                            div().absolute().left(px(18. * u)).bottom(px(18. * u)).text_size(px(17. * u)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.88)).child(t)
                        }))
                        .when(fresh, |d| d.child(div().absolute().top(px(14. * u)).left(px(14. * u)).px(px(12. * u)).py(px(5. * u)).rounded_full().bg(theme::neon()).text_size(px(14. * u)).font_weight(FontWeight::BOLD).child("NEW")))
                        .children(prog.map(|p| div().absolute().left_0().bottom_0().w_full().h(px(5. * u)).bg(theme::white(0.2)).child(div().h_full().w(gpui::relative(p.clamp(0., 1.))).bg(theme::white(0.95)))));
                }
                if on {
                    card = card.child(light(lift));
                }
                let (row, key, key2, mm, mr) = (ri + 1, r.key.clone(), r.key.clone(), m.clone(), m.clone());
                let wide = r.wide;
                div()
                    .id(SharedString::from(format!("tv-{}-{i}", r.key)))
                    .flex_none()
                    .w(px(cw))
                    .flex()
                    .flex_col()
                    .gap(px(14. * u))
                    .cursor_pointer()
                    .on_hover(point(me, move |t, cx| t.set_focus(row, i, Some(key.clone()), cx)))
                    .on_click(act(me, move |t, cx| {
                        t.set_focus(row, i, Some(key2.clone()), cx);
                        if mm.kind == "series" && !wide {
                            t.open_page(mm.clone(), cx);
                        } else {
                            t.play(&mm, cx);
                        }
                    }))
                    .on_mouse_down(gpui::MouseButton::Right, move |e, _, cx| {
                        cx.stop_propagation();
                        Nav::menu(&mr, e.position, cx)
                    })
                    .child(div().relative().w(px(cw)).h(px(ch)).child(card))
                    // the focused poster's name under it
                    .child(div().h(px(28. * u)).px(px(4. * u)).text_size(px(19. * u)).font_weight(FontWeight::SEMIBOLD).line_clamp(1).text_ellipsis().text_color(theme::white(if on && !r.wide { 0.95 } else { 0. })).child(m.name.clone()))
            });
            // the mouse: page a row along
            let n = r.items.len();
            let arrows = (mouse && focused_row && n > 4).then(|| {
                let (k1, k2) = (r.key.clone(), r.key.clone());
                div()
                    .absolute()
                    .top(px(ch / 2. - 28. * u))
                    .left_0()
                    .w_full()
                    .flex()
                    .justify_between()
                    .px(px(30. * u))
                    .child(round_glass(format!("tv-{}-l", r.key), "chevronLeft", 56. * u, u).opacity(if c > 0 { 0.9 } else { 0. }).on_click(act(me, move |t, cx| {
                        let c = t.cols.entry(k1.clone()).or_insert(0);
                        *c = c.saturating_sub(4);
                        t.moved(cx);
                    })))
                    .child(round_glass(format!("tv-{}-r", r.key), "chevron", 56. * u, u).opacity(if c + 1 < n { 0.9 } else { 0. }).on_click(act(me, move |t, cx| {
                        let c = t.cols.entry(k2.clone()).or_insert(0);
                        *c = (*c + 4).min(n.saturating_sub(1));
                        t.moved(cx);
                    })))
            });
            col = col.child(
                div()
                    .h(px(heights[ri]))
                    .flex()
                    .flex_col()
                    .gap(px(18. * u))
                    .opacity(if gone { 0. } else if focused_row || self.row == 0 { 1. } else { 0.8 })
                    .child(div().pl(px(g)).text_size(px(26. * u)).font_weight(FontWeight::BOLD).text_color(theme::white(if focused_row { 0.96 } else { 0.7 })).child(r.title.clone()))
                    .child(div().relative().w_full().h(px(ch + 42. * u)).child(div().absolute().top_0().left(px(g - x)).flex().gap(px(gap)).children(cards)).children(arrows)),
            );
        }
        col.into_any_element()
    }

    fn sidebar(&self, u: f32, vh: f32, mouse: bool, me: &WeakEntity<Tv>) -> AnyElement {
        let open = self.side;
        let cur = match self.section {
            Section::Search => 0,
            Section::Home => 1,
            Section::Shows => 2,
            Section::Movies => 3,
            Section::List => 4,
        };
        let w = if open { 380. * u } else { 84. * u };
        let items = SIDE.iter().enumerate().map(|(i, (ic, label))| {
            let focus = open && i == self.side_i;
            let here = i == cur;
            let fg: Hsla = if focus { gpui::black() } else { theme::white(if here { 0.98 } else { 0.72 }) };
            div()
                .id(SharedString::from(format!("tv-side-{i}")))
                .h(px(62. * u))
                .px(px(20. * u))
                .rounded_full()
                .flex()
                .items_center()
                .gap(px(18. * u))
                .cursor_pointer()
                .when(focus, |d| d.bg(theme::white(0.94)))
                .when(!focus && here && open, |d| d.bg(theme::white(0.12)))
                .when(!focus, |d| d.hover(|s| s.bg(theme::white(0.16))))
                .on_hover(point(me, move |t, cx| {
                    if t.side && t.side_i != i {
                        t.side_i = i;
                        cx.notify();
                    }
                }))
                .on_click(act(me, move |t, cx| t.side_pick(i, cx)))
                .child(if here { crate::ui::icon_fill(ic, 28. * u) } else { icon(ic, 28. * u) }.text_color(fg))
                .when(open, |d| d.child(div().text_size(px(23. * u)).font_weight(if here { FontWeight::BOLD } else { FontWeight::MEDIUM }).text_color(fg).child(label.to_string())))
        });
        let me2 = me.clone();
        let panel = glass(div(), u, false)
            .id("tv-side")
            .relative()
            .overflow_hidden()
            .bg(theme::black(if open { 0.6 } else { 0.3 }))
            .w(px(w))
            .rounded(px(34. * u))
            .p(px(11. * u))
            .flex()
            .flex_col()
            .gap(px(6. * u))
            .child(sheen(u))
            // the mouse opens it by pointing at it, and closes it by leaving
            .on_hover(move |on, _, cx| {
                let _ = me2.update(cx, |t, cx| {
                    let moving = t.mouse_at.map_or(false, |m| m.elapsed().as_millis() < 300);
                    if *on && moving && !t.side {
                        t.side = true;
                        t.side_i = match t.section {
                            Section::Search => 0,
                            Section::Home => 1,
                            Section::Shows => 2,
                            Section::Movies => 3,
                            Section::List => 4,
                        };
                        cx.notify();
                    } else if !*on && t.side && moving {
                        t.side = false;
                        cx.notify();
                    }
                });
            })
            .children(items);
        // collapsed, it shows on the billboard, while the mouse is about and on search
        let show = open || self.row == 0 || mouse || self.section == Section::Search;
        div()
            .absolute()
            .left(px(26. * u))
            .top(px(0.))
            .h(px(vh))
            .flex()
            .items_center()
            .when(self.page.is_none() && show, |d| d.child(panel))
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn page_view(&mut self, u: f32, g: f32, vw: f32, vh: f32, dt: f32, me: &WeakEntity<Tv>, busy: &mut bool) -> Option<AnyElement> {
        let p = self.page.as_ref()?;
        let m = p.meta.clone();
        let series = !p.eps.is_empty();
        let in_season: Vec<Value> = p.eps.iter().filter(|e| e["season"].as_i64() == Some(p.season)).cloned().collect();
        // play: where you are, or the start
        let play_label = match (&p.resume, series) {
            (Some((v, t, _)), true) => {
                let parts: Vec<&str> = v.split(':').collect();
                if parts.len() == 3 { format!("{} S{} E{}", if *t > 30. { "Resume" } else { "Play" }, parts[1], parts[2]) } else { "Play".into() }
            }
            (Some((_, t, _)), false) if *t > 30. => "Resume".into(),
            (_, true) => "Play S1 E1".into(),
            _ => "Play".into(),
        };
        let (line, cols) = (p.line, p.col);
        let on = |l: usize, i: usize| line == l && cols[l] == i;
        // the focused episode's words take the place of the show's
        let ep_focus = (line == 2).then(|| in_season.get(cols[2]).cloned()).flatten();
        let about = ep_focus.as_ref().and_then(|e| e["overview"].as_str().or(e["description"].as_str()).map(String::from)).filter(|s| !s.is_empty()).or(m.description.clone());
        let seasons = p.seasons.clone();
        let season = p.season;
        let seen = p.seen.clone();
        let resume = p.resume.clone();
        let loading = p.loading;
        let mm = m.clone();
        let mut col = div()
            .absolute()
            .left(px(g))
            .top(px(vh * if series { 0.13 } else { 0.38 }))
            .w(px(vw - g * 2.))
            .flex()
            .flex_col()
            .gap(px(20. * u))
            .child(logo_or_name(&m, 160. * u, 620. * u))
            .child(div().text_size(px(22. * u)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.78)).child(match &ep_focus {
                Some(e) => format!("S{} E{}  ·  {}", e["season"].as_i64().unwrap_or(0), e["episode"].as_i64().or(e["number"].as_i64()).unwrap_or(0), e["name"].as_str().or(e["title"].as_str()).unwrap_or("")),
                None => facts(&m),
            }))
            .children(about.map(|d| div().max_w(px(820. * u)).h(px(99. * u)).text_size(px(23. * u)).line_height(px(33. * u)).line_clamp(3).text_ellipsis().text_color(theme::white(0.86)).child(d)))
            .child(
                div()
                    .mt(px(8. * u))
                    .flex()
                    .gap(px(16. * u))
                    .child(glass_btn("tv-p-play", Some(&play_label), "play", on(0, 0), u).on_hover(point(me, |t, cx| t.page_focus(0, 0, cx))).on_click(act(me, |t, cx| t.page_play(cx))))
                    .child(glass_btn("tv-p-add", Some("My List"), "plus", on(0, 1), u).on_hover(point(me, |t, cx| t.page_focus(0, 1, cx))).on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        Nav::add(&mm, cx)
                    })),
            );
        if loading && m.kind == "series" {
            col = col.child(div().mt(px(26. * u)).child(skeleton(5, 380. * u, 214. * u, 30. * u, 0., 16. * u)));
        }
        if series {
            col = col.child(div().mt(px(26. * u)).flex().gap(px(12. * u)).children(seasons.iter().enumerate().map(|(i, s)| {
                let f = on(1, i);
                let here = *s == season;
                div()
                    .id(SharedString::from(format!("tv-season-{i}")))
                    .h(px(50. * u))
                    .px(px(24. * u))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .text_size(px(20. * u))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .when(f, |d| d.bg(theme::white(0.94)).text_color(gpui::black()))
                    .when(!f && here, |d| d.bg(theme::white(0.16)))
                    .when(!f && !here, |d| d.text_color(theme::white(0.6)).hover(|s| s.bg(theme::white(0.1))))
                    .on_click(act(me, move |t, cx| {
                        if let Some(p) = t.page.as_mut() {
                            p.line = 1;
                        }
                        t.page_season(i, cx);
                    }))
                    .child(format!("Season {s}"))
            })));
            let (cw, ch) = (380. * u, 214. * u);
            let gap = 30. * u;
            let c = cols[2];
            let tx = (c as f32 - 1.).max(0.) * (cw + gap);
            let x = self.xs.entry("eps".into()).or_insert(tx);
            *x += (tx - *x) * (dt * 12.).min(1.);
            *busy |= (tx - *x).abs() > 0.5;
            let x = *x;
            let lift = if line == 2 { ease(self.focus_at.elapsed().as_secs_f32() / 0.22) } else { 0. };
            *busy |= line == 2 && lift < 1.;
            let cards = in_season.iter().enumerate().map(|(i, e)| {
                let f = on(2, i);
                let s = if f { 1. + 0.08 * lift } else { 1. };
                let id = e["id"].as_str().unwrap_or("").to_string();
                let watched = seen.iter().any(|x| *x == id);
                let still = e["thumbnail"].as_str().map(|t| crate::net::fit(t, cw * 1.1));
                let n = e["episode"].as_i64().or(e["number"].as_i64()).unwrap_or(0);
                let name = e["name"].as_str().or(e["title"].as_str()).unwrap_or("").to_string();
                // the one you're partway through: how far
                let prog = resume.as_ref().filter(|(v, t, d)| *v == id && *t > 30. && *d > 0.).map(|(_, t, d)| (t / d) as f32);
                div()
                    .id(SharedString::from(format!("tv-ep-{i}")))
                    .flex_none()
                    .w(px(cw))
                    .flex()
                    .flex_col()
                    .gap(px(12. * u))
                    .cursor_pointer()
                    .on_hover(point(me, move |t, cx| t.page_focus(2, i, cx)))
                    .on_click(act(me, move |t, cx| t.page_episode(i, cx)))
                    .child(
                        div().relative().w(px(cw)).h(px(ch)).child(
                            lifted(cw, ch, s, lift, f, 16. * u, u)
                                .children(still.map(|t| img(t).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
                                .when(watched, |d| d.child(div().absolute().top_0().left_0().size_full().bg(theme::black(0.45))).child(div().absolute().top(px(12. * u)).right(px(12. * u)).size(px(34. * u)).rounded_full().bg(theme::white(0.9)).flex().items_center().justify_center().child(icon("check", 22. * u).text_color(gpui::black()))))
                                .children(prog.map(|p| div().absolute().left_0().bottom_0().w_full().h(px(5. * u)).bg(theme::white(0.2)).child(div().h_full().w(gpui::relative(p.clamp(0., 1.))).bg(theme::white(0.95)))))
                                .when(f, |d| d.child(light(lift))),
                        ),
                    )
                    .child(div().text_size(px(17. * u)).font_weight(FontWeight::SEMIBOLD).text_color(theme::white(0.6)).child(format!("Episode {n}")))
                    .child(div().text_size(px(20. * u)).font_weight(FontWeight::SEMIBOLD).line_clamp(1).text_ellipsis().text_color(theme::white(if f { 0.98 } else { 0.8 })).child(name))
            });
            col = col.child(div().relative().w(px(vw)).ml(px(-g)).h(px(ch + 90. * u)).child(div().absolute().top_0().left(px(g - x)).flex().gap(px(gap)).children(cards)));
        }
        Some(col.into_any_element())
    }

    fn page_focus(&mut self, line: usize, i: usize, cx: &mut Context<Self>) {
        let Some(p) = self.page.as_mut() else { return };
        if p.line != line || p.col[line] != i {
            p.line = line;
            p.col[line] = i;
            self.focus_at = Instant::now();
            cx.notify();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn find_view(&mut self, u: f32, g: f32, vw: f32, vh: f32, me: &WeakEntity<Tv>, busy: &mut bool) -> AnyElement {
        let sa = ease(self.section_at.elapsed().as_secs_f32() / 0.45);
        *busy |= sa < 1.;
        let key = 64. * u;
        let kgap = 10. * u;
        let kb_w = key * KB_COLS as f32 + kgap * (KB_COLS as f32 - 1.);
        // what you've typed, in a glass field
        let typed = self.find.query.clone();
        let field = glass(div(), u, false)
            .relative()
            .overflow_hidden()
            .w(px(kb_w))
            .h(px(72. * u))
            .rounded(px(20. * u))
            .px(px(24. * u))
            .flex()
            .items_center()
            .gap(px(14. * u))
            .child(sheen(u))
            .child(icon("search", 26. * u).text_color(theme::white(0.7)))
            .child(if typed.is_empty() {
                div().text_size(px(26. * u)).text_color(theme::white(0.45)).child("Shows, movies…")
            } else {
                div().text_size(px(28. * u)).font_weight(FontWeight::SEMIBOLD).line_clamp(1).text_ellipsis().child(format!("{typed}▏"))
            });
        let keys = KEYS.chars().enumerate().map(|(i, ch)| {
            let f = self.find.kb == Some(i) && !self.side;
            div()
                .id(SharedString::from(format!("tv-key-{i}")))
                .size(px(key))
                .rounded(px(16. * u))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(27. * u))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .when(f, |d| d.bg(theme::white(0.94)).text_color(gpui::black()).shadow(vec![gpui::BoxShadow { color: theme::black(0.5), offset: gpui::point(px(0.), px(10. * u)), blur_radius: px(24. * u), spread_radius: px(0.), inset: false }]))
                .when(!f, |d| d.bg(theme::white(0.08)).hover(|s| s.bg(theme::white(0.18))))
                .on_hover(point(me, move |t, cx| {
                    if t.find.kb != Some(i) {
                        t.find.kb = Some(i);
                        t.side = false;
                        cx.notify();
                    }
                }))
                .on_click(act(me, move |t, cx| {
                    t.find.kb = Some(i);
                    t.type_key(&ch.to_string(), cx);
                }))
                .child(ch.to_string())
        });
        let grid = div().w(px(kb_w)).flex().flex_wrap().gap(px(kgap)).children(keys);
        let wide_w = (kb_w - kgap * 2.) / 3.;
        let wide = div().flex().gap(px(kgap)).children(KB_WIDE.iter().enumerate().map(|(j, w)| {
            let i = KEYS.len() + j;
            let f = self.find.kb == Some(i) && !self.side;
            let label = match *w {
                "space" => "space",
                "delete" => "⌫",
                _ => "clear",
            };
            div()
                .id(SharedString::from(format!("tv-key-{i}")))
                .w(px(wide_w))
                .h(px(key))
                .rounded(px(16. * u))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(21. * u))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .when(f, |d| d.bg(theme::white(0.94)).text_color(gpui::black()))
                .when(!f, |d| d.bg(theme::white(0.08)).text_color(theme::white(0.8)).hover(|s| s.bg(theme::white(0.18))))
                .on_hover(point(me, move |t, cx| {
                    if t.find.kb != Some(i) {
                        t.find.kb = Some(i);
                        cx.notify();
                    }
                }))
                .on_click(act(me, move |t, cx| {
                    t.find.kb = Some(i);
                    t.type_key(w, cx);
                }))
                .child(label)
        }));
        let left = div().absolute().left(px(g + 20. * u)).top(px(vh * 0.16)).flex().flex_col().gap(px(22. * u)).child(div().text_size(px(46. * u)).font_weight(FontWeight::BLACK).child("Search")).child(field).child(div().flex().flex_col().gap(px(kgap)).child(grid).child(wide));
        // the results: posters, five across
        let rx = g + 20. * u + kb_w + 90. * u;
        let avail = vw - rx - 64. * u;
        let per = self.results_cols();
        let pg = 28. * u;
        let pw = ((avail - 28. * u - pg * (per as f32 - 1.)) / per as f32).clamp(110. * u, 210. * u);
        let ph = pw * 1.5;
        let hit = self.find.hit;
        let in_results = self.find.kb.is_none() && !self.side;
        let lift = if in_results { ease(self.focus_at.elapsed().as_secs_f32() / 0.22) } else { 0. };
        *busy |= in_results && lift < 1.;
        // keep the focused line in view
        let line_h = ph + 36. * u + pg;
        let line = hit / per;
        let shift = (line as f32 - 1.).max(0.) * line_h;
        let results = self.find.results.iter().enumerate().map(|(i, m)| {
            let f = in_results && i == hit;
            let s = if f { 1. + 0.1 * lift } else { 1. };
            let pic = m.poster.clone().map(|p| crate::net::fit(&sized(&p, "w342"), pw * 1.1));
            let mm = m.clone();
            let mr = m.clone();
            div()
                .id(SharedString::from(format!("tv-hit-{i}")))
                .flex_none()
                .w(px(pw))
                .flex()
                .flex_col()
                .gap(px(10. * u))
                .cursor_pointer()
                .on_hover(point(me, move |t, cx| {
                    if t.find.kb.is_some() || t.find.hit != i {
                        t.find.kb = None;
                        t.find.hit = i;
                        t.moved(cx);
                    }
                }))
                .on_click(act(me, move |t, cx| t.open_page(mm.clone(), cx)))
                .on_mouse_down(gpui::MouseButton::Right, move |e, _, cx| {
                    cx.stop_propagation();
                    Nav::menu(&mr, e.position, cx)
                })
                .child(
                    div().relative().w(px(pw)).h(px(ph)).child(
                        lifted(pw, ph, s, lift, f, 16. * u, u)
                            .child(div().absolute().top_0().left_0().size_full().p(px(12. * u)).flex().items_end().text_size(px(19. * u)).font_weight(FontWeight::BLACK).line_clamp(3).child(m.name.clone()))
                            .children(pic.map(|p| img(p).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
                            .when(f, |d| d.child(light(lift))),
                    ),
                )
                .child(div().h(px(26. * u)).text_size(px(17. * u)).font_weight(FontWeight::SEMIBOLD).line_clamp(1).text_ellipsis().text_color(theme::white(if f { 0.95 } else { 0.6 })).child(m.name.clone()))
        });
        let status = if self.find.results.is_empty() {
            Some(if self.find.searching {
                "searching…".to_string()
            } else if self.find.asked.chars().count() >= 2 {
                format!("nothing for “{}”", self.find.asked)
            } else {
                "type with the keyboard, your remote or a controller".to_string()
            })
        } else {
            None
        };
        let title = if self.find.results.is_empty() { "Results".to_string() } else { format!("Results  ·  {}", self.find.results.len()) };
        let right = div()
            .absolute()
            .left(px(rx))
            .top(px(vh * 0.16))
            .w(px(avail))
            .h(px(vh * 0.8))
            .overflow_hidden()
            .child(div().text_size(px(26. * u)).font_weight(FontWeight::BOLD).text_color(theme::white(0.7)).mb(px(24. * u)).child(title))
            .children(status.map(|s| div().text_size(px(22. * u)).text_color(theme::white(0.55)).child(s)))
            .child(div().relative().w_full().h(px(vh * 0.7)).child(div().absolute().top(px(-shift + 14. * u)).left(px(14. * u)).w(px(avail - 28. * u)).flex().flex_wrap().gap(px(pg)).children(results)));
        div().absolute().top_0().left_0().size_full().opacity(sa).child(left).child(right).into_any_element()
    }
}
