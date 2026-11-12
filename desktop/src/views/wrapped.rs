//! couch Wrapped: your year as a story. Click the right side (or →) for the
//! next card, the left side (or ←) to go back, Esc to leave. Every number
//! comes from your Simkl history (or what couch saw you watch).
use crate::api::{self, sized};
use crate::theme;
use chrono::{Datelike, Timelike};
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, Context, FontWeight, ObjectFit, Window};
use serde_json::{json, Value};
use std::time::Instant;

const SLIDE_MS: f32 = 6500.;
const MONTHS: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

#[derive(Default)]
struct Year {
    total: usize,
    films: usize,
    shows: usize,
    episodes: i64,
    hours: i64,
    genres: Vec<(String, usize)>,
    months: [usize; 12],
    top_month: usize,
    when: &'static str,
    tens: Vec<Value>,
    binge: Option<Value>,
}

pub struct Wrapped {
    pub open: bool,
    year: i32,
    rows: Option<Vec<Value>>,
    i: usize,
    t0: Instant,
}

impl Wrapped {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { open: false, year: chrono::Local::now().year(), rows: None, i: 0, t0: Instant::now() }
    }

    pub fn show(&mut self, cx: &mut Context<Self>) {
        self.open = true;
        self.i = 0;
        self.rows = None;
        self.t0 = Instant::now();
        let year = self.year;
        cx.spawn(async move |this, cx| {
            let r = api::call::<Vec<Value>>("wrapped_data", json!({ "year": year })).await.unwrap_or_default();
            let _ = this.update(cx, |w, cx| {
                w.rows = Some(r);
                w.t0 = Instant::now();
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn step(&mut self, by: i32, cx: &mut Context<Self>) {
        let n = self.slides().len() as i32;
        let next = self.i as i32 + by;
        if next >= n {
            self.open = false;
        } else {
            self.i = next.max(0) as usize;
            self.t0 = Instant::now();
        }
        cx.notify();
    }

    fn year_stats(&self) -> Year {
        let r = self.rows.clone().unwrap_or_default();
        let mins = |x: &Value| x["minutes"].as_i64();
        let films: Vec<&Value> = r.iter().filter(|x| x["type"] == "movie" && (x["status"] == "completed" || x["watchedMinutes"].as_i64().unwrap_or(0) > 60)).collect();
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
        for x in &r {
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
        for x in &r {
            if let Some(at) = x["at"].as_str().and_then(|a| chrono::DateTime::parse_from_rfc3339(a).ok()) {
                let l = at.with_timezone(&chrono::Local);
                months[l.month0() as usize] += 1;
                hours[l.hour() as usize] += 1;
            }
        }
        let top_month = (0..12).max_by_key(|m| months[*m]).unwrap_or(0);
        let slot = |a: usize, b: usize| hours[a..b].iter().sum::<usize>();
        let parts = [("night", slot(0, 6)), ("morning", slot(6, 12)), ("afternoon", slot(12, 18)), ("evening", slot(18, 24))];
        let when = parts.iter().max_by_key(|p| p.1).map(|p| p.0).unwrap_or("evening");
        let tens: Vec<Value> = r.iter().filter(|x| x["userRating"].as_i64().unwrap_or(0) >= 9).take(12).cloned().collect();
        let binge = shows.iter().max_by_key(|x| x["eps"].as_i64().unwrap_or(0)).map(|x| (*x).clone());
        Year { total: r.len(), films: films.len(), shows: shows.len(), episodes, hours: (minutes as f64 / 60.).round() as i64, genres: g, months, top_month, when, tens, binge }
    }

    fn slides(&self) -> Vec<&'static str> {
        let Some(r) = &self.rows else { return vec!["loading"] };
        if r.is_empty() {
            return vec!["empty"];
        }
        let y = self.year_stats();
        let mut l = vec!["intro", "hours", "count"];
        if !y.genres.is_empty() {
            l.push("genres");
        }
        if y.months.iter().any(|m| *m > 0) {
            l.push("months");
        }
        l.push("when");
        if y.binge.as_ref().map_or(false, |b| b["eps"].as_i64().unwrap_or(0) > 2) {
            l.push("binge");
        }
        if !y.tens.is_empty() {
            l.push("tens");
        }
        l.push("outro");
        l
    }
}

fn big(t: String) -> gpui::Div {
    div().text_size(px(120.)).line_height(px(118.)).font_weight(FontWeight::BLACK).child(t)
}
fn line(t: String) -> gpui::Div {
    div().text_size(px(30.)).font_weight(FontWeight::BLACK).child(t)
}
fn sub(t: String) -> gpui::Div {
    div().max_w(px(640.)).text_size(px(19.)).line_height(px(27.)).text_color(theme::white(0.75)).child(t)
}

impl Render for Wrapped {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        window.request_animation_frame();
        let slides = self.slides();
        let slide = slides.get(self.i).copied().unwrap_or("intro");
        let ms = self.t0.elapsed().as_secs_f32() * 1000.;
        let auto = slide != "loading" && slide != "empty";
        if auto && ms > SLIDE_MS && self.i + 1 < slides.len() {
            cx.defer_in(window, |w, _, cx| w.step(1, cx));
        }
        let k = (ms / 700.).min(1.); // entrance
        let ease = 1. - (1. - k).powi(3);
        let y = self.year_stats();
        let count = |n: i64| ((n as f32) * (ms / 1400.).min(1.)).round() as i64;
        // a slow-moving glow, a different colour per card
        let palettes = [(0xe0309a, 0xffb25c), (0xffb25c, 0x7a3b5e), (0x9b6bff, 0xe0309a), (0x4cc9f0, 0x9b6bff), (0xff6ec4, 0xffd84a)];
        let (c1, c2) = palettes[self.i % palettes.len()];
        let mut g1: gpui::Hsla = gpui::rgb(c1).into();
        g1.a = 0.55;
        let mut g2: gpui::Hsla = gpui::rgb(c2).into();
        g2.a = 0.45;
        let drift = (self.t0.elapsed().as_secs_f32() * 0.25).sin() * 40.;
        let yr = self.year;

        let body: gpui::AnyElement = match slide {
            "loading" => div().text_size(px(22.)).text_color(theme::label2()).child("adding up your year…").into_any_element(),
            "empty" => div().flex().flex_col().gap(px(14.)).child(line(format!("a quiet {yr} on the couch."))).child(sub("watch a few things and come back — your year fills in as you go.".into())).into_any_element(),
            "intro" => div().flex().flex_col().gap(px(10.)).child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).text_color(theme::white(0.7)).child("midnite™ couch")).child(big(format!("{yr}"))).child(line("your year on the couch.".into())).into_any_element(),
            "hours" => div().flex().flex_col().gap(px(10.)).child(sub("you spent".into())).child(big(format!("{}", count(y.hours)))).child(line("hours on the couch.".into())).child(sub(format!("that's {} full days of watching.", (y.hours as f32 / 24.).round() as i64))).into_any_element(),
            "count" => div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(big(format!("{}", count(y.total as i64))))
                .child(line("titles.".into()))
                .child(sub(format!("{} films, {} shows, {} episodes.", y.films, y.shows, y.episodes)))
                .into_any_element(),
            "genres" => div()
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(line("your genres".into()))
                .children(y.genres.iter().enumerate().map(|(n, (g, c))| {
                    let a = ((ms - n as f32 * 180.) / 500.).clamp(0., 1.);
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(18.))
                        .opacity(a)
                        .child(div().w(px(50.)).text_size(px(40.)).font_weight(FontWeight::BLACK).text_color(theme::white(0.5)).child(format!("{}", n + 1)))
                        .child(div().text_size(px(if n == 0 { 64. } else { 40. })).font_weight(FontWeight::BLACK).child(g.to_lowercase()))
                        .child(div().text_size(px(16.)).text_color(theme::white(0.6)).child(format!("{c}")))
                }))
                .into_any_element(),
            "months" => {
                let max = *y.months.iter().max().unwrap_or(&1) as f32;
                div()
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .child(line(format!("{} was your month.", MONTHS[y.top_month])))
                    .child(div().flex().items_end().gap(px(10.)).h(px(260.)).children((0..12).map(|m| {
                        let h = (y.months[m] as f32 / max.max(1.)) * 240. * ((ms - m as f32 * 60.) / 600.).clamp(0., 1.);
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(6.))
                            .child(div().w(px(38.)).h(px(h.max(4.))).rounded(px(8.)).bg(if m == y.top_month { theme::text().into() } else { theme::white(0.3) }))
                            .child(div().text_size(px(12.)).text_color(theme::white(0.6)).child(MONTHS[m][..3].to_string()))
                    })))
                    .into_any_element()
            }
            "when" => {
                let (t, s) = match y.when {
                    "night" => ("a midnite creature", "most of your watching happened after midnight. the couch knows."),
                    "afternoon" => ("matinee person", "afternoons on the couch. respectfully, what's your job."),
                    "morning" => ("early riser", "you watch in the morning. a rare and powerful creature."),
                    _ => ("prime-time regular", "evenings were yours. dinner, then the couch, every time."),
                };
                div().flex().flex_col().gap(px(12.)).child(sub("you are".into())).child(div().text_size(px(72.)).line_height(px(74.)).font_weight(FontWeight::BLACK).child(t)).child(sub(s.into())).into_any_element()
            }
            "binge" => {
                let b = y.binge.clone().unwrap_or_default();
                div()
                    .flex()
                    .items_center()
                    .gap(px(36.))
                    .child(div().w(px(240.)).h(px(360.)).rounded(px(16.)).overflow_hidden().bg(theme::surface2()).children(b["poster"].as_str().map(|u| img(sized(u, "w500")).size_full().object_fit(ObjectFit::Cover))))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(sub("your binge of the year".into()))
                            .child(div().text_size(px(56.)).line_height(px(58.)).font_weight(FontWeight::BLACK).max_w(px(560.)).child(b["name"].as_str().unwrap_or("").to_string()))
                            .child(line(format!("{} episodes.", count(b["eps"].as_i64().unwrap_or(0))))),
                    )
                    .into_any_element()
            }
            "tens" => div()
                .flex()
                .flex_col()
                .gap(px(18.))
                .child(line("your 9s and 10s".into()))
                .child(div().flex().flex_wrap().gap(px(12.)).max_w(px(900.)).children(y.tens.iter().enumerate().map(|(n, t)| {
                    let a = ((ms - n as f32 * 120.) / 400.).clamp(0., 1.);
                    div().w(px(130.)).h(px(195.)).rounded(px(10.)).overflow_hidden().bg(theme::surface2()).opacity(a).children(t["poster"].as_str().map(|u| img(sized(u, "w342")).size_full().object_fit(ObjectFit::Cover)))
                })))
                .into_any_element(),
            _ => div().flex().flex_col().gap(px(12.)).child(big("see you".into())).child(line(format!("on the couch in {}.", yr + 1))).child(sub("click to close".into())).into_any_element(),
        };

        let n = slides.len();
        let i = self.i;
        let prog = (ms / SLIDE_MS).min(1.);
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::rgb(0x0b070a))
            .child(div().absolute().left(px(-200. + drift)).top(px(-260.)).size(px(900.)).rounded_full().bg(g1).opacity(0.7))
            .child(div().absolute().right(px(-260. - drift)).bottom(px(-300.)).size(px(1000.)).rounded_full().bg(g2).opacity(0.6))
            .child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(180., linear_color_stop(theme::black(0.35), 0.), linear_color_stop(theme::black(0.75), 1.))))
            // progress segments
            .child(div().absolute().top(px(18.)).left(px(theme::GUTTER)).right(px(theme::GUTTER + 160.)).flex().gap(px(6.)).children((0..n).map(|k| {
                let f = if k < i { 1. } else if k == i { prog } else { 0. };
                div().flex_1().h(px(4.)).rounded_full().bg(theme::white(0.25)).child(div().h_full().rounded_full().w(gpui::relative(f)).bg(theme::text()))
            })))
            .child(div().absolute().left(px(theme::GUTTER * 2.)).top(px(150. + (1. - ease) * 40.)).opacity(ease).child(body))
            // click left = back, right = next
            .child(div().id("wr-prev").absolute().top(px(40.)).left_0().w(gpui::relative(0.3)).h_full().on_click(cx.listener(|w, _, _, cx| w.step(-1, cx))))
            .child(div().id("wr-next").absolute().top(px(40.)).right_0().w(gpui::relative(0.7)).h_full().on_click(cx.listener(|w, _, _, cx| w.step(1, cx))))
            .child(
                div()
                    .id("wr-close")
                    .absolute()
                    .top(px(44.))
                    .right(px(theme::GUTTER))
                    .size(px(40.))
                    .rounded_full()
                    .bg(theme::white(0.14))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|w, _, _, cx| {
                        w.open = false;
                        cx.notify();
                    }))
                    .child(crate::ui::icon("close", 18.)),
            )
            .into_any_element()
    }
}
