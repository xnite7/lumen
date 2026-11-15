//! couch Wrapped, as **midnite rewind**: your year played back as a late-night
//! broadcast on the couch's own channel. Not a stack of stories: a television
//! in a dark room. It powers on out of a bright line, the tape rewinds to
//! January, and each part of your year is a channel; changing channel folds
//! the picture to a line and opens the next one out of it. Lines land one
//! beat at a time, deadpan, like a bump between shows. The lamp in the corner
//! of the room warms and dims with what's on; the neon only lights for the
//! one thing that matters. It ends on the credits, and the station signs off.
//!
//! Click the right of the screen (or → / space) for the next channel, the
//! left (or ←) to go back, Esc to switch off. Every number comes from your
//! Simkl history (or what couch saw you watch).
use crate::api::{self, sized};
use crate::motion::{self, dur};
use crate::theme::{self, ty};
use chrono::{Datelike, Timelike};
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, Context, FontWeight, Hsla, ObjectFit, Window};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const MONTHS: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

#[derive(Default, Clone)]
struct Year {
    total: usize,
    films: usize,
    shows: usize,
    episodes: i64,
    hours: i64,
    genres: Vec<(String, usize)>,
    months: [usize; 12],
    top_month: usize,
    /// share of the year's watching in each part of the day
    parts: [(&'static str, f32); 4],
    when: &'static str,
    peak_hour: u32,
    tens: Vec<Value>,
    binge: Option<Value>,
    first: Option<String>,
    last: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Ch {
    Loading,
    Empty,
    Intro,
    Hours,
    Count,
    Genres,
    Months,
    When,
    Binge,
    Tens,
    Credits,
}

impl Ch {
    /// how long a channel stays on before the next one (the credits stay)
    fn length(self) -> f32 {
        match self {
            Ch::Intro => 5.2,
            Ch::Hours => 6.4,
            Ch::Count | Ch::Genres | Ch::Months => 6.8,
            Ch::When => 7.2,
            Ch::Binge => 6.2,
            Ch::Tens => 7.0,
            _ => f32::INFINITY,
        }
    }
    /// what the on-screen display calls it
    fn name(self) -> &'static str {
        match self {
            Ch::Intro => "cold open",
            Ch::Hours => "time",
            Ch::Count => "lineup",
            Ch::Genres => "channels",
            Ch::Months => "signal",
            Ch::When => "clock",
            Ch::Binge => "now playing",
            Ch::Tens => "rewatch",
            Ch::Credits => "credits",
            _ => "",
        }
    }
}

pub struct Wrapped {
    pub open: bool,
    year: i32,
    rows: Option<Vec<Value>>,
    y: Year,
    i: usize,
    /// when the channel on now started showing
    t0: Instant,
    /// switched on at (the picture opens out of a line)
    on_at: Instant,
    /// a channel change: when it started
    switch: Option<Instant>,
    /// what was on before the change (shown until the picture is dark)
    from: usize,
    /// switching off: when (the picture folds away, then it's closed)
    off: Option<Instant>,
    /// the room's light, easing toward what each channel wants: (lamp, neon)
    light: (f32, f32),
    last: Instant,
}

impl Wrapped {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        let now = Instant::now();
        let mut w = Self { open: false, year: season_year().unwrap_or(chrono::Local::now().year()), rows: None, y: Year::default(), i: 0, t0: now, on_at: now, switch: None, from: 0, off: None, light: (0., 0.), last: now };
        // a debug build can open on a made-up year, to see it without an account
        if cfg!(debug_assertions) && std::env::var("COUCH_WRAPPED_DEMO").is_ok() {
            w.open = true;
            w.set_rows(demo_rows(w.year));
            w.on_at = now + Duration::from_secs(5);
            w.t0 = w.on_at + Duration::from_secs_f32(dur::CHANNEL * 0.5);
        }
        w
    }

    pub fn show(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.open = true;
        self.i = 0;
        self.rows = None;
        self.switch = None;
        self.off = None;
        self.on_at = now;
        self.t0 = now + Duration::from_secs_f32(dur::CHANNEL * 0.5);
        self.light = (0., 0.);
        let year = self.year;
        cx.spawn(async move |this, cx| {
            let r = api::call::<Vec<Value>>("wrapped_data", json!({ "year": year })).await.unwrap_or_default();
            let _ = this.update(cx, |w, cx| {
                w.set_rows(r);
                w.t0 = Instant::now();
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn set_rows(&mut self, r: Vec<Value>) {
        self.y = year_stats(&r);
        self.rows = Some(r);
    }

    /// Switch off: the picture folds to a line, the line to a dot, then dark.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.off.is_none() {
            self.off = Some(Instant::now());
        }
        cx.notify();
    }

    pub fn step(&mut self, by: i32, cx: &mut Context<Self>) {
        if self.off.is_some() {
            return;
        }
        // mid-change: let it land first
        if self.switch.is_some_and(|s| motion::secs(s) < dur::CHANNEL * 0.5) {
            return;
        }
        let n = self.channels().len() as i32;
        let next = self.i as i32 + by;
        if next >= n {
            return self.close(cx);
        }
        let next = next.max(0) as usize;
        let now = Instant::now();
        if next == self.i {
            // back on the first channel: start it over
            self.t0 = now;
        } else {
            self.from = self.i;
            self.i = next;
            self.switch = Some(now);
            self.t0 = now + Duration::from_secs_f32(dur::CHANNEL * 0.5);
        }
        cx.notify();
    }

    fn channels(&self) -> Vec<Ch> {
        let Some(r) = &self.rows else { return vec![Ch::Loading] };
        if r.is_empty() {
            return vec![Ch::Empty];
        }
        let y = &self.y;
        let mut l = vec![Ch::Intro, Ch::Hours, Ch::Count];
        if !y.genres.is_empty() {
            l.push(Ch::Genres);
        }
        if y.months.iter().any(|m| *m > 0) {
            l.push(Ch::Months);
        }
        l.push(Ch::When);
        if y.binge.as_ref().is_some_and(|b| b["eps"].as_i64().unwrap_or(0) > 2) {
            l.push(Ch::Binge);
        }
        if !y.tens.is_empty() {
            l.push(Ch::Tens);
        }
        l.push(Ch::Credits);
        l
    }

    /// what the room's light should be on this channel (lamp, neon), 0..1
    fn light_for(&self, ch: Ch, t: f32) -> (f32, f32) {
        match ch {
            Ch::Loading | Ch::Empty => (0.35, 0.),
            Ch::Intro => (0.2, if t > 0.9 { 0.8 * motion::neon(t - 0.9) } else { 0. }),
            // the lamp warms as the hours count up
            Ch::Hours => (0.2 + 0.7 * motion::lamp(motion::along(t, dur::BEAT, 2.4)), 0.),
            Ch::Count | Ch::Genres | Ch::Tens => (0.55, 0.08),
            Ch::Months => (0.45, 0.25),
            Ch::When if self.y.when == "night" => (0.12, 0.75),
            Ch::When => (0.8, 0.05),
            Ch::Binge => (0.25, 0.),
            Ch::Credits => (0.3, 0.35),
        }
    }
}

// ------------------------------------------------------------- the year

fn year_stats(r: &[Value]) -> Year {
    let mins = |x: &Value| x["minutes"].as_i64();
    let films = r.iter().filter(|x| x["type"] == "movie" && (x["status"] == "completed" || x["watchedMinutes"].as_i64().unwrap_or(0) > 60)).count();
    let shows: Vec<&Value> = r.iter().filter(|x| x["type"] == "series").collect();
    let episodes: i64 = shows.iter().map(|x| x["eps"].as_i64().unwrap_or(0)).sum();
    let minutes: i64 = r
        .iter()
        .map(|x| {
            if let Some(w) = x["watchedMinutes"].as_i64().filter(|w| *w > 0) {
                w
            } else if x["type"] == "movie" {
                if x["status"] == "completed" { mins(x).unwrap_or(110) } else { 0 }
            } else {
                x["eps"].as_i64().unwrap_or(0) * mins(x).unwrap_or(42)
            }
        })
        .sum();
    let mut g: Vec<(String, usize)> = vec![];
    for x in r {
        for name in x["genres"].as_array().into_iter().flatten().filter_map(|n| n.as_str()) {
            match g.iter_mut().find(|e| e.0 == name) {
                Some(e) => e.1 += 1,
                None => g.push((name.to_string(), 1)),
            }
        }
    }
    g.sort_by(|a, b| b.1.cmp(&a.1));
    g.truncate(5);
    let mut months = [0usize; 12];
    let mut hours = [0usize; 24];
    let mut dated: Vec<(chrono::DateTime<chrono::Local>, String)> = vec![];
    for x in r {
        if let Some(at) = x["at"].as_str().and_then(|a| chrono::DateTime::parse_from_rfc3339(a).ok()) {
            let l = at.with_timezone(&chrono::Local);
            months[l.month0() as usize] += 1;
            hours[l.hour() as usize] += 1;
            dated.push((l, x["name"].as_str().unwrap_or("").to_string()));
        }
    }
    dated.sort_by_key(|d| d.0);
    let top_month = (0..12).max_by_key(|m| (months[*m], 12 - m)).unwrap_or(0);
    let peak_hour = (0..24u32).max_by_key(|h| hours[*h as usize]).unwrap_or(22);
    let slot = |a: usize, b: usize| hours[a..b].iter().sum::<usize>() as f32;
    let all = (slot(0, 24)).max(1.);
    let parts = [("night", slot(0, 6) / all), ("morning", slot(6, 12) / all), ("afternoon", slot(12, 18) / all), ("evening", slot(18, 24) / all)];
    let when = parts.iter().copied().fold(("evening", -1.), |a, p| if p.1 > a.1 { p } else { a }).0;
    let mut tens: Vec<Value> = r.iter().filter(|x| x["userRating"].as_i64().unwrap_or(0) >= 9).cloned().collect();
    tens.sort_by_key(|x| -x["userRating"].as_i64().unwrap_or(0));
    tens.truncate(8);
    let binge = shows.iter().max_by_key(|x| x["eps"].as_i64().unwrap_or(0)).map(|x| (*x).clone());
    Year {
        total: r.len(),
        films,
        shows: shows.len(),
        episodes,
        hours: (minutes as f64 / 60.).round() as i64,
        genres: g,
        months,
        top_month,
        parts,
        when,
        peak_hour,
        tens,
        binge,
        first: dated.first().map(|d| d.1.clone()),
        last: dated.last().filter(|_| dated.len() > 1).map(|d| d.1.clone()),
    }
}

fn when_words(when: &str) -> (&'static str, &'static str) {
    match when {
        "night" => ("a midnite creature", "most of your watching happened after midnight. the couch knows."),
        "afternoon" => ("matinee person", "afternoons on the couch. respectfully, what's your job."),
        "morning" => ("early riser", "you watch in the morning. a rare and powerful creature."),
        _ => ("prime-time regular", "evenings were yours. dinner, then the couch, every time."),
    }
}

fn clock(h: u32) -> String {
    let (n, m) = match h {
        0 => (12, "am"),
        1..=11 => (h, "am"),
        12 => (12, "pm"),
        _ => (h - 12, "pm"),
    };
    format!("{n}:00 {m}")
}

// ------------------------------------------------------------ type, deadpan

/// a small line that sets up the next one ("you spent about")
fn kick(t: &str) -> gpui::Div {
    div().text_size(px(18.)).font_weight(FontWeight::BOLD).text_color(theme::label2()).child(t.to_string())
}
/// the deadpan line under a reveal
fn aside(t: String) -> gpui::Div {
    div().max_w(px(620.)).text_center().text_size(px(19.)).line_height(px(28.)).font_weight(FontWeight::MEDIUM).text_color(theme::label2()).child(t)
}
fn heavy(t: String, size: f32) -> gpui::Div {
    div().text_size(px(size)).line_height(px(size * 0.98)).font_weight(FontWeight::BLACK).child(t)
}
/// a line arriving on beat `n` of the channel
fn on_beat(e: impl IntoElement, t: f32, n: f32) -> gpui::Div {
    let k = motion::lamp(motion::along(t, n * dur::BEAT, dur::LAMP));
    div().relative().top(px((1. - k) * 14.)).opacity(k).child(e)
}

/// Digits on a tape counter: each wheel rolls into place, the higher ones
/// only turning when the one below passes 9. Leading zeros stay, dim.
fn counter(v: f32, places: usize, size: f32, lit: Hsla) -> impl IntoElement {
    let cell = size * 1.02;
    let shown = v.max(0.);
    let first = (0..places).rev().find(|p| shown >= 10f32.powi(*p as i32)).unwrap_or(0);
    div().flex().children((0..places).rev().map(move |p| {
        let unit = 10f32.powi(p as i32);
        let base = (shown / unit).floor() % 10.;
        // the wheel above turns as the one below goes 9 → 0
        let below = shown % unit;
        let carry = if p == 0 { shown.fract() } else { ((below - (unit - 1.)) / 1.).clamp(0., 1.) };
        let off = base + carry;
        let lead = p > first;
        div()
            .w(px(size * 0.62))
            .h(px(cell))
            .overflow_hidden()
            .relative()
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(-off * cell))
                    .w_full()
                    .flex()
                    .flex_col()
                    .text_size(px(size))
                    .line_height(px(cell))
                    .font_weight(FontWeight::BLACK)
                    .text_color(if lead { theme::label4().into() } else { lit })
                    .children((0..11).map(|d| div().h(px(cell)).flex().justify_center().child(format!("{}", d % 10)))),
            )
    }))
}

/// dots between a name and its number, like a TV listing
fn leader() -> impl IntoElement {
    gpui::canvas(
        |_, _, _| (),
        |b, _, window, _| {
            let y = b.origin.y + b.size.height - px(9.);
            let mut x = f32::from(b.origin.x) + 6.;
            let end = f32::from(b.origin.x + b.size.width) - 6.;
            while x < end {
                window.paint_quad(gpui::fill(gpui::Bounds::new(gpui::point(px(x), y), gpui::size(px(3.), px(3.))), theme::white(0.22)).corner_radii(px(1.5)));
                x += 11.;
            }
        },
    )
    .flex_1()
    .h(px(28.))
}

// --------------------------------------------------------------- channels

impl Wrapped {
    fn channel(&self, ch: Ch, t: f32, vw: f32, vh: f32) -> gpui::AnyElement {
        let y = &self.y;
        let yr = self.year;
        let col = || div().flex().flex_col().items_center().gap(px(14.));
        match ch {
            Ch::Loading => col()
                .child(div().size(px(170.)).children(crate::couch_loading_frame().map(|f| img(gpui::ImageSource::Render(f)).size_full().object_fit(ObjectFit::Contain))))
                .child(aside("rewinding your year…".into()))
                .into_any_element(),
            Ch::Empty => col()
                .child(div().size(px(170.)).children(crate::couch_loading_frame().map(|f| img(gpui::ImageSource::Render(f)).size_full().object_fit(ObjectFit::Contain))))
                .child(on_beat(heavy("nothing on tonight.".into(), ty::HERO), t, 0.))
                .child(on_beat(aside(format!("nothing logged for {yr} yet. watch something and come back.")), t, 1.))
                .into_any_element(),

            Ch::Intro => {
                // the year is a neon sign switching on
                let k = if t > 0.9 { motion::neon(t - 0.9) * if t > 0.9 + dur::NEON { motion::hum(t) } else { 1. } } else { 0. };
                let mut tube: Hsla = theme::neon_soft().into();
                tube.a = k.min(1.);
                let mut core: Hsla = gpui::rgb(0xffe3f3).into();
                core.a = (k - 0.2).clamp(0., 1.) * 0.9;
                col()
                    .child(on_beat(kick("midnite™ couch presents"), t, 0.))
                    .child(
                        div()
                            .relative()
                            .child(motion::pool(180., 100., 240., theme::neon_a(0.32 * k)))
                            .child(div().relative().text_size(px(210.)).line_height(px(200.)).font_weight(FontWeight::BLACK).text_color(tube).child(format!("{yr}")))
                            // the bright inside of the tube
                            .child(div().absolute().top_0().left_0().text_size(px(210.)).line_height(px(200.)).font_weight(FontWeight::BLACK).text_color(core).opacity(0.5).child(format!("{yr}"))),
                    )
                    .child(on_beat(aside("your year on the couch, rewound.".into()), t, 4.))
                    .into_any_element()
            }

            Ch::Hours => {
                let k = motion::lamp(motion::along(t, dur::BEAT, 2.4));
                let days = (y.hours as f32 / 24. * 10.).round() / 10.;
                let quip = if y.hours > 300 { "the cushions have your shape now." } else if y.hours > 100 { "solid. very solid." } else { "a gentle year." };
                col()
                    .child(on_beat(kick("you spent about"), t, 0.))
                    .child(on_beat(counter(y.hours as f32 * k, 4.max(y.hours.to_string().len()), ty::DISPLAY * 1.25, theme::text().into()), t, 1.))
                    .child(on_beat(heavy("hours on the couch.".into(), ty::TITLE1), t, 4.5))
                    .child(on_beat(aside(format!("that's {days} days. {quip}")), t, 6.))
                    .into_any_element()
            }

            Ch::Count => {
                let row = |label: &str, n: i64, b: f32| {
                    let k = motion::lamp(motion::along(t, b * dur::BEAT + 0.2, 1.4));
                    on_beat(
                        div()
                            .w(px(620.))
                            .flex()
                            .items_end()
                            .gap(px(14.))
                            .child(div().text_size(px(ty::TITLE2 + 6.)).font_weight(FontWeight::BOLD).text_color(theme::label2()).child(label.to_string()))
                            .child(leader())
                            .child(heavy(format!("{}", (n as f32 * k).round() as i64), 64.)),
                        t,
                        b,
                    )
                };
                let story = match (&y.first, &y.last) {
                    (Some(a), Some(b)) => format!("it started with {a} and (so far) ends with {b}."),
                    (Some(a), None) => format!("it started with {a}."),
                    _ => String::new(),
                };
                col()
                    .gap(px(18.))
                    .child(on_beat(kick("tonight's lineup: your year"), t, 0.))
                    .child(div().h(px(6.)))
                    .child(row("films", y.films as i64, 1.))
                    .child(row("shows", y.shows as i64, 2.))
                    .child(row("episodes", y.episodes, 3.))
                    .child(div().h(px(10.)))
                    .when(!story.is_empty(), |d| d.child(on_beat(aside(story), t, 5.)))
                    .into_any_element()
            }

            Ch::Genres => col()
                .gap(px(10.))
                .child(on_beat(kick("your channels"), t, 0.))
                .child(div().h(px(10.)))
                .children(y.genres.iter().enumerate().map(|(n, (g, c))| {
                    let top = n == 0;
                    on_beat(
                        div()
                            .w(px(700.))
                            .flex()
                            .items_center()
                            .gap(px(22.))
                            .child(div().w(px(64.)).text_size(px(18.)).font_weight(FontWeight::BOLD).text_color(theme::label3()).child(format!("ch {}", n + 1)))
                            .child(div().text_size(px(if top { ty::HERO } else { 34. })).line_height(px(if top { ty::HERO } else { 40. })).font_weight(FontWeight::BLACK).text_color(if top { theme::text() } else { theme::label2() }).child(g.to_lowercase()))
                            .when(top, |d| {
                                // an "on air" light, lit by the lamp
                                d.child(div().px(px(12.)).py(px(5.)).rounded_full().bg(theme::lamp()).text_color(gpui::rgb(0x120a06)).text_size(px(13.)).font_weight(FontWeight::BLACK).shadow(theme::glow_lamp(motion::hum(t))).child("on air"))
                            })
                            .child(div().flex_1())
                            .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child(format!("{c} {}", if *c == 1 { "title" } else { "titles" }))),
                        t,
                        1. + n as f32 * 0.6,
                    )
                }))
                .into_any_element(),

            Ch::Months => {
                let max = (*y.months.iter().max().unwrap_or(&1)).max(1) as f32;
                const SEGS: usize = 12;
                col()
                    .gap(px(12.))
                    .child(on_beat(kick("your busiest month"), t, 0.))
                    .child(on_beat(heavy(MONTHS[y.top_month].into(), ty::HERO), t, 1.))
                    .child(div().h(px(18.)))
                    // a level meter: each month's column of lights climbs to its level
                    .child(div().flex().items_end().gap(px(10.)).children((0..12).map(|m| {
                        let lit = ((y.months[m] as f32 / max) * SEGS as f32).round().max(if y.months[m] > 0 { 1. } else { 0. }) as usize;
                        let top = m == y.top_month;
                        let start = 2. * dur::BEAT + m as f32 * 0.05;
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(4.))
                            .children((0..SEGS).rev().map(move |s| {
                                let on = s < lit && t > start + s as f32 * 0.045;
                                let c: Hsla = if !on {
                                    theme::white(0.05)
                                } else if top {
                                    theme::neon_soft().into()
                                } else {
                                    theme::lamp_a(0.35 + 0.5 * (s as f32 / SEGS as f32))
                                };
                                div().w(px(40.)).h(px(10.)).rounded(px(3.)).bg(c).when(on && top, |d| d.shadow(theme::glow_neon(0.5)))
                            }))
                            .child(div().pt(px(8.)).text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(if top { theme::text() } else { theme::label3() }).child(MONTHS[m][..3].to_string()))
                    })))
                    .into_any_element()
            }

            Ch::When => {
                let (title, line) = when_words(y.when);
                let night = y.when == "night";
                let k = if night && t > 2. * dur::BEAT { motion::neon(t - 2. * dur::BEAT) } else { 1. };
                let mut title_c: Hsla = if night { theme::neon_soft().into() } else { theme::text().into() };
                title_c.a = k.min(1.);
                let vcr = motion::blink(t);
                col()
                    .gap(px(14.))
                    .child(on_beat(kick("you are"), t, 0.))
                    .child(div().relative().when(night, |d| d.child(motion::pool(330., 40., 260., theme::neon_a(0.28 * k)))).child(div().relative().text_size(px(ty::HERO + 16.)).line_height(px(ty::HERO + 16.)).font_weight(FontWeight::BLACK).text_color(title_c).child(title)))
                    .child(on_beat(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .child(div().text_size(px(17.)).font_weight(FontWeight::BOLD).text_color(theme::label3()).child("most watched at"))
                            // an unset VCR clock, blinking
                            .child(div().px(px(16.)).py(px(4.)).rounded(px(10.)).bg(theme::white(0.04)).text_size(px(46.)).font_weight(FontWeight::BLACK).text_color(theme::lamp_a(if vcr { 1. } else { 0.12 })).child(clock(y.peak_hour))),
                        t,
                        4.,
                    ))
                    .child(on_beat(
                        div().flex().gap(px(26.)).children(y.parts.iter().map(|(name, share)| {
                            let me = *name == y.when;
                            div()
                                .flex()
                                .items_baseline()
                                .gap(px(6.))
                                .child(div().text_size(px(22.)).font_weight(FontWeight::BLACK).text_color(if me { theme::text() } else { theme::label3() }).child(format!("{}%", (share * 100.).round() as i64)))
                                .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(if me { theme::label2() } else { theme::label3() }).child(name.to_string()))
                        })),
                        t,
                        5.,
                    ))
                    .child(on_beat(aside(line.into()), t, 6.5))
                    .into_any_element()
            }

            Ch::Binge => {
                let b = y.binge.clone().unwrap_or_default();
                let eps = b["eps"].as_i64().unwrap_or(0);
                let k = motion::lamp(motion::along(t, 2. * dur::BEAT, 1.8));
                let quip = if eps > 30 { "you didn't watch it. you moved in." } else { "one more episode. several times." };
                col()
                    .child(on_beat(kick("most binged"), t, 0.))
                    .child(on_beat(div().max_w(px(vw * 0.8)).text_size(px(ty::HERO + 8.)).line_height(px(ty::HERO + 8.)).font_weight(FontWeight::BLACK).text_center().child(b["name"].as_str().unwrap_or("").to_string()), t, 1.))
                    .child(on_beat(div().flex().items_center().gap(px(14.)).child(counter(eps as f32 * k, 3, 56., theme::lamp().into())).child(heavy("episodes".into(), 34.)), t, 2.))
                    .child(on_beat(aside(quip.into()), t, 5.))
                    .into_any_element()
            }

            Ch::Tens => col()
                .gap(px(26.))
                .child(on_beat(kick("the ones you'd watch again"), t, 0.))
                .child(div().flex().flex_wrap().justify_center().gap(px(16.)).max_w(px(vw * 0.86)).children(y.tens.iter().enumerate().map(|(n, x)| {
                    // dropped onto the pile, one by one, with a little weight
                    let at = 1.6 * dur::BEAT + n as f32 * 0.14;
                    let s = motion::bouncy(t - at);
                    let score = x["userRating"].as_i64().unwrap_or(0);
                    div()
                        .relative()
                        .top(px((1. - s) * -60.))
                        .opacity(motion::out3(motion::along(t, at, 0.25)))
                        .w(px(150.))
                        .h(px(225.))
                        .rounded(px(theme::radius::POSTER))
                        .overflow_hidden()
                        .bg(theme::surface2())
                        .shadow(vec![gpui::BoxShadow { color: theme::black(0.8), offset: gpui::point(px(0.), px(18.)), blur_radius: px(36.), spread_radius: px(-12.), inset: false }])
                        .children(x["poster"].as_str().map(|u| img(sized(u, "w342")).size_full().object_fit(ObjectFit::Cover)))
                        .child(div().absolute().left(px(8.)).bottom(px(8.)).px(px(8.)).py(px(2.)).rounded(px(8.)).bg(theme::lamp()).text_color(gpui::rgb(0x120a06)).text_size(px(14.)).font_weight(FontWeight::BLACK).child(format!("{score}")))
                })))
                .into_any_element(),

            Ch::Credits => self.credits(t, vh).into_any_element(),
        }
    }

    /// The end credits: who did what this year, rolling up the screen, then the
    /// station signs off.
    fn credits(&self, t: f32, vh: f32) -> impl IntoElement {
        let y = &self.y;
        let yr = self.year;
        let mut rows: Vec<(String, String)> = vec![
            ("starring".into(), "you".into()),
            ("hours on the couch".into(), y.hours.to_string()),
            ("films".into(), y.films.to_string()),
            ("shows".into(), y.shows.to_string()),
            ("episodes".into(), y.episodes.to_string()),
        ];
        if let Some((g, _)) = y.genres.first() {
            rows.push(("top genre".into(), g.to_lowercase()));
        }
        rows.push(("busiest month".into(), MONTHS[y.top_month].into()));
        rows.push(("you are".into(), when_words(y.when).0.into()));
        if let Some(b) = y.binge.as_ref().and_then(|b| b["name"].as_str()) {
            rows.push(("most binged".into(), b.into()));
        }
        if let Some(n) = y.tens.first().and_then(|b| b["name"].as_str()) {
            rows.push(("best of the year".into(), n.into()));
        }
        rows.push(("filmed on location".into(), "your couch".into()));
        const ROW: f32 = 58.;
        const HEAD: f32 = 210.;
        const END: f32 = 300.;
        let total = HEAD + rows.len() as f32 * ROW + 120. + END;
        // rolls up at a reading pace, and stops with the sign-off in the middle
        let start = vh * 0.62;
        let stop = vh * 0.5 - (total - END * 0.5);
        let top = (start - (t - 0.6).max(0.) * 54.).max(stop);
        let done = ((stop + 40. - top) / 40.).clamp(0., 1.);
        div().absolute().left_0().top(px(top)).w_full().flex().flex_col().items_center().child(
            div()
                .w(px(760.))
                .flex()
                .flex_col()
                .items_center()
                .child(div().h(px(HEAD)).flex().flex_col().items_center().justify_center().gap(px(10.)).child(kick("a midnite™ couch production")).child(heavy(format!("your {yr}"), ty::HERO)))
                .children(rows.into_iter().map(|(role, name)| {
                    div()
                        .h(px(ROW))
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(26.))
                        .child(div().flex_1().flex().justify_end().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child(role))
                        .child(div().flex_1().text_size(px(26.)).font_weight(FontWeight::BLACK).child(name))
                }))
                .child(div().h(px(120.)))
                .child(
                    div()
                        .h(px(END))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(12.))
                        .child(div().size(px(130.)).children(crate::couch_loading_frame().map(|f| img(gpui::ImageSource::Render(f)).size_full().object_fit(ObjectFit::Contain))))
                        .child(heavy("that's the broadcast.".into(), 40.))
                        .child(aside(format!("midnite™ couch is going off the air. back on in {}.", yr + 1)))
                        .child(div().pt(px(8.)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).opacity(done).child("click to switch off")),
                ),
        )
    }
}

// ------------------------------------------------------------- the screen

impl Render for Wrapped {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        window.request_animation_frame();
        let vp = window.viewport_size();
        let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
        let chans = self.channels();
        let n = chans.len();
        self.i = self.i.min(n - 1);
        let ch = chans[self.i];
        let t = motion::secs(self.t0);
        let since_on = self.on_at.elapsed().as_secs_f32();

        // switching off: fold away, then close
        if let Some(off) = self.off {
            if motion::secs(off) > 0.7 {
                self.open = false;
                self.off = None;
                cx.notify();
                return div().into_any_element();
            }
        }
        // the channel changes on its own when its time is up
        if self.switch.is_none() && self.off.is_none() && t > ch.length() && self.i + 1 < n {
            cx.defer_in(window, |w, _, cx| w.step(1, cx));
        }
        // a change in progress: the old channel until the picture is dark
        let change = self.switch.map(|s| motion::channel(motion::secs(s)));
        if self.switch.is_some_and(|s| motion::secs(s) > dur::CHANNEL) {
            self.switch = None;
        }
        let showing = match &change {
            Some(c) if !c.swap => chans.get(self.from).copied().unwrap_or(ch),
            _ => ch,
        };
        // switched on: the picture opens out of a line (the second half of a change)
        let power = if since_on < dur::CHANNEL * 0.5 { Some(motion::channel(dur::CHANNEL * 0.5 + since_on.max(0.))) } else { None };
        let fold = self.off.map(|o| {
            let k = motion::secs(o) / 0.7;
            (motion::in3((k / 0.55).min(1.)), motion::out3(((k - 0.55) / 0.45).clamp(0., 1.)))
        });

        // the room's light eases toward what this channel wants
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        let want = self.light_for(showing, if showing == ch { t } else { 99. });
        let ease = 1. - (-dt / 0.45).exp();
        self.light.0 += (want.0 - self.light.0) * ease;
        // neon follows its flicker exactly (it's a switch, not a dimmer)
        self.light.1 += (want.1 - self.light.1) * if want.1 > self.light.1 { 1. } else { ease };
        let (lamp, neon) = self.light;

        // the playhead: where on the year's tape this moment is
        let prog = if ch.length().is_finite() { (t / ch.length()).clamp(0., 1.) } else { 1. };
        let head = if matches!(ch, Ch::Loading | Ch::Empty) { 0. } else { ((self.i as f32 + prog) / n as f32).clamp(0., 1.) };
        let rewinding = ch == Ch::Intro && t < 2.2 || ch == Ch::Loading;
        let day = if rewinding { 364. * (1. - motion::in_out3(motion::along(t, 0.2, 2.))) } else { head * 364. };
        let date = chrono::NaiveDate::from_yo_opt(self.year, 1).map(|d| d + chrono::Duration::days(day as i64)).map(|d| d.format("%b %d").to_string().to_lowercase()).unwrap_or_default();

        let body_t = if showing == ch { t } else { 99. };
        let body = self.channel(showing, body_t.max(0.), vw, vh);
        let binge_art = (showing == Ch::Binge).then(|| self.y.binge.as_ref().and_then(|b| b["background"].as_str().or(b["poster"].as_str())).map(|u| sized(u, "w1280"))).flatten();
        let clock_t = since_on;
        let snow = change.as_ref().map_or(0., |c| c.snow).max(power.as_ref().map_or(0., |c| c.snow));

        div()
            .id("wrapped")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .overflow_hidden()
            .bg(gpui::rgb(0x050304))
            .font_family(theme::FONT)
            .text_color(theme::text())
            // the room: a lamp low in one corner, a neon sign's spill high in the other
            .child(motion::pool(vw * 0.08, vh * 1.02, vh * 0.85, theme::lamp_a(0.42 * lamp)))
            .child(motion::pool(vw * 0.94, vh * -0.04, vh * 0.7, theme::neon_a(0.34 * neon)))
            // what's playing, behind the words (most binged)
            .children(binge_art.map(|u| {
                let z = 1. + 0.08 * (1. - motion::out3(motion::along(t, 0., 9.)));
                div()
                    .absolute()
                    .left(px(-(vw * (z - 1.)) / 2.))
                    .top(px(-(vh * (z - 1.)) / 2.))
                    .w(px(vw * z))
                    .h(px(vh * z))
                    .opacity(0.34 * motion::lamp(motion::along(t, 0., 1.2)))
                    .child(img(u).size_full().object_fit(ObjectFit::Cover))
            }))
            .when(showing == Ch::Binge, |d| d.child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(180., linear_color_stop(theme::black(0.2), 0.), linear_color_stop(theme::black(0.85), 1.)))))
            // the picture
            .child(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().child(body))
            // the broadcast's texture
            .child(motion::scanlines(clock_t, 0.16))
            .child(motion::grain(clock_t, 0.18 + snow))
            // on-screen display, top left: the deck's state and the date on the tape
            .child(
                div()
                    .absolute()
                    .top(px(30.))
                    .left(px(theme::GUTTER))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .text_size(px(16.))
                    .font_weight(FontWeight::BLACK)
                    .child(div().size(px(9.)).rounded_full().bg(if rewinding { theme::lamp_a(if motion::blink(clock_t * 2.) { 1. } else { 0.2 }) } else { theme::neon_a(0.9) }).when(!rewinding, |d| d.shadow(theme::glow_neon(0.6))))
                    .child(if rewinding { "rew" } else { "play" })
                    .child(div().w(px(80.)).text_color(theme::label2()).child(date)),
            )
            // top right: the channel
            .child(
                div()
                    .absolute()
                    .top(px(30.))
                    .right(px(theme::GUTTER + 56.))
                    .flex()
                    .items_baseline()
                    .gap(px(10.))
                    .when(!matches!(ch, Ch::Loading | Ch::Empty), |d| {
                        d.child(div().text_size(px(16.)).font_weight(FontWeight::BLACK).child(format!("ch {:02}", self.i + 1))).child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label2()).child(showing.name()))
                    }),
            )
            // the tape: the whole year, the playhead riding along it
            .child(
                div()
                    .absolute()
                    .bottom(px(30.))
                    .left(px(theme::GUTTER))
                    .right(px(theme::GUTTER))
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .child(div().text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(theme::label3()).child("jan"))
                    .child(
                        div()
                            .flex_1()
                            .h(px(14.))
                            .relative()
                            .child(div().absolute().left_0().right_0().top(px(6.)).h(px(2.)).rounded_full().bg(theme::white(0.12)))
                            .child(div().absolute().left_0().top(px(6.)).h(px(2.)).w(gpui::relative(head)).rounded_full().bg(theme::lamp_a(0.75)))
                            .children((1..n).map(|k| div().absolute().left(gpui::relative(k as f32 / n as f32)).top(px(3.)).w(px(2.)).h(px(8.)).rounded_full().bg(theme::white(0.22))))
                            .child(div().absolute().left(gpui::relative(head)).top(px(1.)).ml(px(-6.)).size(px(12.)).rounded_full().bg(theme::lamp()).shadow(theme::glow_lamp(0.9))),
                    )
                    .child(div().text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(theme::label3()).child("dec")),
            )
            // the station's bug in the corner: the couch, boiling
            .child(div().absolute().right(px(theme::GUTTER)).bottom(px(62.)).size(px(56.)).opacity(0.32).children(crate::couch_loading_frame().map(|f| img(gpui::ImageSource::Render(f)).size_full().object_fit(ObjectFit::Contain))))
            // click left = back, right = next (the credits: anywhere switches off)
            .child(div().id("wr-prev").absolute().top(px(80.)).left_0().w(gpui::relative(0.3)).bottom(px(80.)).cursor_pointer().on_click(cx.listener(|w, _, _, cx| w.step(-1, cx))))
            .child(div().id("wr-next").absolute().top(px(80.)).right_0().w(gpui::relative(0.7)).bottom(px(80.)).cursor_pointer().on_click(cx.listener(|w, _, _, cx| w.step(1, cx))))
            .child(
                div()
                    .id("wr-close")
                    .absolute()
                    .top(px(22.))
                    .right(px(theme::GUTTER))
                    .size(px(40.))
                    .rounded_full()
                    .bg(theme::white(0.08))
                    .hover(|s| s.bg(theme::white(0.16)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|w, _, _, cx| w.close(cx)))
                    .child(crate::ui::icon("close", 18.)),
            )
            // a channel change, or the set warming up: the picture folds to a line
            .children(change.as_ref().or(power.as_ref()).map(|c| motion::shutters(c, vh, theme::text().into())))
            // switching off: fold to a line, the line to a dot, dark
            .children(fold.map(|(shut, dot)| {
                let c = motion::Channel { open: 1. - shut, line: if dot > 0. { 0. } else { shut }, swap: false, snow: 0. };
                let w = vw * (1. - dot);
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .child(motion::shutters(&c, vh, theme::text().into()))
                    .when(dot > 0., |d| {
                        d.bg(gpui::black()).child(
                            div()
                                .absolute()
                                .left(px((vw - w.max(6.)) / 2.))
                                .top(px(vh / 2. - 1.5))
                                .w(px(w.max(6.)))
                                .h(px(3.))
                                .rounded_full()
                                .bg(theme::white(1. - dot * dot))
                                .shadow(vec![gpui::BoxShadow { color: theme::white(0.8 * (1. - dot)), offset: gpui::point(px(0.), px(0.)), blur_radius: px(20.), spread_radius: px(2.), inset: false }]),
                        )
                    })
            }))
            .into_any_element()
    }
}

/// Wrapped is for a whole year: it shows up in December (this year) and
/// January (the year just ended), and not the rest of the time.
pub fn season_year() -> Option<i32> {
    let now = chrono::Local::now();
    match now.month() {
        12 => Some(now.year()),
        1 => Some(now.year() - 1),
        _ => None,
    }
}

/// A made-up year (debug builds, COUCH_WRAPPED_DEMO=1): enough to see every channel.
fn demo_rows(year: i32) -> Vec<Value> {
    let names = ["The Long Summer", "Radio Silence", "Night Swim", "Sunflower Static", "Glasshouse", "Afterglow", "Low Tide", "Paper Moons", "The Quiet Hour", "Velvet Static", "Northbound", "Second Shift", "Hollow Bay", "Kingfisher"];
    let genres = [["Drama", "Mystery"], ["Comedy", "Drama"], ["Drama", "Thriller"], ["Sci-Fi", "Drama"], ["Animation", "Comedy"], ["Thriller", "Mystery"]];
    names
        .iter()
        .enumerate()
        .map(|(k, n)| {
            let series = k % 3 != 0;
            let at = chrono::NaiveDate::from_ymd_opt(year, (k as u32 * 5) % 12 + 1, 3 + k as u32).and_then(|d| d.and_hms_opt(((k * 7) % 24) as u32, 12, 0)).map(|d| d.and_utc().to_rfc3339()).unwrap_or_default();
            json!({
                "id": format!("tt{k}"), "type": if series { "series" } else { "movie" }, "name": n,
                "genres": genres[k % genres.len()], "at": at, "status": "completed",
                "userRating": if k % 3 == 0 { 10 } else if k % 3 == 1 { 9 } else { 7 },
                "eps": if series { Some(6 + k * 3) } else { None }, "minutes": if series { 45 } else { 118 },
            })
        })
        .collect()
}
