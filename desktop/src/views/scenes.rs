//! scenes: guess the movie from its most famous scene (ClipCafe's most
//! watched one). A blink first; show more for fewer points; one guess from
//! four titles. The couch server runs every round (which scene, what you've
//! revealed, the score, the boards); this page only shows it.
//!
//!   daily 5   the same five for everyone, easy first then nicher; one go a
//!             day, keeps a streak
//!   endless   starts easy, gets nicher; three misses end the run
use crate::api;
use crate::theme;
use crate::ui::{icon, video};
use gpui::{div, img, prelude::*, px, AnimationExt, Context, Div, FontWeight, ScrollHandle, SharedString, Stateful, Window};
use std::sync::Arc;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// "trailer:" clips always have sound, and stop when you leave the page
const OWNER: &str = "trailer:scenes";

pub struct Scenes {
    home: Option<Value>,
    round: Option<Value>,
    stage: usize,
    picked: Option<usize>,
    result: Option<Value>,
    done: Option<Value>,
    busy: bool,
    error: String,
    board: usize,
    /// this round's video
    clip: String,
    /// the bit of it playing now: which play this is, whether it's on, when
    /// it was let go, and how long it may show (None = to the end)
    play_no: u64,
    on: bool,
    asked: Option<Instant>,
    seen: Option<Instant>,
    limit: Option<f32>,
    clip_failed: bool,
    /// loaded and waiting on its first picture, paused and covered: it's let
    /// go (picture and sound together) once that picture has been drawn
    held: bool,
    frames: u8,
    /// the finished game as a picture to share
    card: Option<(Arc<gpui::Image>, Vec<u8>)>,
    scroll: ScrollHandle,
}

/// the couch mark, as an SVG path (in a 1024 box)
const COUCH: &str = include_str!("couch_path.txt");

fn nice_day(day: &str) -> String {
    let p: Vec<&str> = day.split('-').collect();
    let m = p.get(1).and_then(|m| m.parse::<usize>().ok()).unwrap_or(1).clamp(1, 12);
    format!("{} {} {}", p.get(2).map(|d| d.trim_start_matches('0')).unwrap_or(""), ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][m - 1], p.first().unwrap_or(&""))
}

fn commas(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The finished game as a card (1200 x 630, good for Discord).
fn card_svg(d: &Value, name: &str, day: &str) -> String {
    let daily = d["mode"].as_str() == Some("daily");
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut sq = String::new();
    for (i, r) in d["results"].as_array().into_iter().flatten().take(6).enumerate() {
        let r = r.as_i64().unwrap_or(-1);
        let x = 690 + i as i64 * 80;
        if r < 0 {
            sq.push_str(&format!(r##"<rect x="{}" y="252" width="64" height="64" rx="16" fill="none" stroke="#ffffff" stroke-opacity="0.28" stroke-width="4"/>"##, x + 2));
        } else {
            let label = ["blink", "1s", "3s", "5s", "all"][r.min(4) as usize];
            sq.push_str(&format!(
                r##"<rect x="{x}" y="250" width="68" height="68" rx="16" fill="#ffffff" fill-opacity="{:.2}"/><text x="{}" y="291" text-anchor="middle" font-family="Satoshi" font-weight="900" font-size="21" fill="#000000" fill-opacity="0.75">{label}</text>"##,
                1. - r as f32 * 0.18,
                x + 34
            ));
        }
    }
    let mut facts = vec![];
    let streak = d["streak"].as_i64().unwrap_or(0);
    if daily && streak > 0 {
        facts.push(format!("{streak} day streak"));
    }
    if let Some(r) = d["rank"].as_i64() {
        facts.push(format!("#{r} {}", if daily { "today" } else { "all time" }));
    }
    let fx: String = facts.iter().enumerate().map(|(i, t)| format!(r##"<text x="690" y="{}" font-family="Satoshi" font-weight="700" font-size="32" fill="#ffffff" fill-opacity="0.9">{t}</text>"##, 400 + i * 46)).collect();
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
<defs><radialGradient id="g" cx="0.22" cy="0.1" r="1"><stop offset="0" stop-color="#2a2a2e"/><stop offset="0.55" stop-color="#0b0b0c"/><stop offset="1" stop-color="#000000"/></radialGradient></defs>
<rect width="1200" height="630" fill="#000"/>
<rect x="24" y="24" width="1152" height="582" rx="44" fill="url(#g)" stroke="#ffffff" stroke-opacity="0.1" stroke-width="2"/>
<g transform="translate(72,66) scale(0.118) translate(-180.9,-294.8)"><path fill="#ffffff" fill-rule="evenodd" d="{COUCH}"/></g>
<text x="168" y="100" font-family="Satoshi" font-weight="900" font-size="34" fill="#ffffff">scenes</text>
<text x="286" y="100" font-family="Satoshi" font-weight="500" font-size="26" fill="#ffffff" fill-opacity="0.5">{} · {}</text>
<text x="68" y="372" font-family="Satoshi" font-weight="900" font-size="176" fill="#ffffff" letter-spacing="-5">{}</text>
<text x="76" y="432" font-family="Satoshi" font-weight="500" font-size="34" fill="#ffffff" fill-opacity="0.6">points · {} of {} right</text>
<text x="690" y="222" font-family="Satoshi" font-weight="700" font-size="21" fill="#ffffff" fill-opacity="0.5" letter-spacing="2">HOW MUCH I NEEDED TO SEE</text>
{sq}{fx}
<text x="72" y="558" font-family="Satoshi" font-weight="700" font-size="30" fill="#ffffff">{}</text>
<text x="1128" y="558" text-anchor="end" font-family="Satoshi" font-weight="500" font-size="26" fill="#ffffff" fill-opacity="0.45">guess the movie from one scene · midnite couch</text>
</svg>"##,
        if daily { "daily five" } else { "endless run" },
        esc(day),
        commas(d["score"].as_i64().unwrap_or(0)),
        d["right"].as_i64().unwrap_or(0),
        d["rounds"].as_i64().unwrap_or(0),
        esc(name),
    )
}

/// Is the game switched on? (the couch server says; asked at start)
static ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn on() -> bool {
    ON.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn watch(cx: &mut gpui::App) {
    cx.spawn(async move |cx| {
        if let Ok(c) = api::call::<Value>("crew_config", json!({})).await {
            ON.store(c["scenes"].as_bool().unwrap_or(false), std::sync::atomic::Ordering::Relaxed);
            let _ = cx.update(|cx| cx.refresh_windows());
        }
    })
    .detach();
}

async fn game(method: &str, path: &str, body: Value) -> Result<Value, String> {
    api::call::<Value>("crew_social", json!({ "method": method, "path": format!("/v1/game{path}"), "body": body })).await
}

fn pill(id: &'static str, label: impl Into<SharedString>, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .h(px(48.))
        .px(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .rounded(px(12.))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .when(primary, |d| d.bg(gpui::white()).text_color(gpui::black()).hover(|s| s.opacity(0.85)))
        .when(!primary, |d| d.bg(theme::white(0.1)).hover(|s| s.bg(theme::white(0.16))))
        .child(label.into())
}

fn card() -> Div {
    div().w_full().p(px(22.)).rounded(px(18.)).bg(theme::white(0.05)).flex().flex_col().gap(px(12.))
}

// the game's colours
const GOLD: u32 = 0xffc75c;
const GREEN: u32 = 0x5cd68f;
const RED: u32 = 0xfa6e69;
const VIOLET: u32 = 0xab96fa;
const FLAME: u32 = 0xff9442;

/// one square per scene: gold = in a blink, then cooler; hollow = missed
fn dots(results: &Value) -> Div {
    div().flex().gap(px(6.)).children(results.as_array().into_iter().flatten().map(|r| {
        let r = r.as_i64().unwrap_or(-1);
        let d = div().size(px(24.)).rounded(px(6.));
        if r < 0 {
            d.border_2().border_color(theme::white(0.3))
        } else {
            d.bg(gpui::rgb([GOLD, GREEN, 0x4fae78, 0x8b7bd0, 0x777777][(r as usize).min(4)]))
        }
    }))
}

impl Scenes {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // software clips (Mac): a new picture redraws the page
        cx.observe(&video::get(cx), |_, _, cx| cx.notify()).detach();
        let mut me = Self {
            home: None,
            round: None,
            stage: 0,
            picked: None,
            result: None,
            done: None,
            busy: false,
            error: String::new(),
            board: 0,
            clip: String::new(),
            play_no: 0,
            on: false,
            asked: None,
            seen: None,
            limit: None,
            clip_failed: false,
            held: false,
            frames: 0,
            card: None,
            scroll: ScrollHandle::new(),
        };
        me.load(cx);
        me
    }

    pub fn load(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let r = game("GET", "/home", Value::Null).await;
            let _ = this.update(cx, |s, cx| {
                match r {
                    Ok(h) => s.home = Some(h),
                    Err(e) => s.error = e,
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn start(&mut self, daily: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error.clear();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = game("POST", "/start", json!({ "mode": if daily { "daily" } else { "endless" } })).await;
            let _ = this.update(cx, |s, cx| {
                s.busy = false;
                match r {
                    Ok(r) => {
                        s.done = None;
                        s.show(r, cx);
                    }
                    Err(e) => s.error = e,
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// A new scene: fetch its video's address, then play the blink.
    fn show(&mut self, round: Value, cx: &mut Context<Self>) {
        self.stage = round["stage"].as_u64().unwrap_or(0) as usize;
        self.picked = None;
        self.result = None;
        self.clip.clear();
        self.clip_failed = false;
        self.stop(cx);
        let path = round["clip"].as_str().unwrap_or("").to_string();
        self.round = Some(round);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = api::call::<String>("game_clip", json!({ "path": path })).await;
            let _ = this.update(cx, |s, cx| match r {
                Ok(u) => {
                    s.clip = u;
                    s.replay(cx);
                }
                Err(e) => {
                    s.error = e;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.play_no += 1;
        self.on = false;
        video::get(cx).update(cx, |v, cx| v.stop(OWNER, cx));
    }

    /// seconds this stage shows (None = the whole scene)
    fn secs(&self) -> Option<f32> {
        if self.result.is_some() {
            return None;
        }
        let s = self.round.as_ref().and_then(|r| r["stages"][self.stage].as_f64()).unwrap_or(0.) as f32;
        (s > 0.).then_some(s)
    }

    /// Play the part you've earned, from the start; then the screen goes dark.
    /// The clip loads paused and covered; once its first picture has been
    /// drawn it's let go, so picture and sound start at the same instant and
    /// the clock only runs while you can actually see it.
    fn replay(&mut self, cx: &mut Context<Self>) {
        if self.clip.is_empty() {
            return;
        }
        self.play_no += 1;
        let play_no = self.play_no;
        self.on = true;
        self.held = true;
        self.frames = 0;
        self.asked = Some(Instant::now());
        self.seen = None;
        self.limit = self.secs();
        self.clip_failed = false;
        let url = self.clip.clone();
        video::get(cx).update(cx, |v, cx| {
            v.stop(OWNER, cx);
            v.play_once(OWNER, &url, cx);
            v.hold(OWNER);
        });
        cx.notify();
        cx.spawn(async move |this, cx| loop {
            crate::timer::after(Duration::from_millis(8)).await;
            let Ok(go) = this.update(cx, |s, cx| {
                if s.play_no != play_no || !s.on {
                    return false;
                }
                let vid = video::get(cx);
                // it ended (or the page was left)
                if vid.read(cx).owner != OWNER {
                    s.on = false;
                    cx.notify();
                    return false;
                }
                let waited = s.asked.map_or(0., |t| t.elapsed().as_secs_f32());
                if s.held {
                    // its first picture has been on screen (under the cover) for
                    // a couple of frames: go. (After 6 s, go anyway.)
                    if s.frames >= 3 || waited > 6. {
                        s.held = false;
                        s.seen = Some(Instant::now());
                        vid.update(cx, |v, _| v.set_paused(OWNER, false));
                    } else if waited > 20. {
                        vid.update(cx, |v, cx| v.stop(OWNER, cx));
                        s.on = false;
                        s.clip_failed = true;
                        cx.notify();
                        return false;
                    }
                    cx.notify();
                    return true;
                }
                let over = match (s.seen, s.limit) {
                    (Some(t), Some(l)) => t.elapsed().as_secs_f32() >= l,
                    _ => false,
                };
                if over {
                    vid.update(cx, |v, cx| v.stop(OWNER, cx));
                    s.on = false;
                    cx.notify();
                    return false;
                }
                cx.notify();
                true
            }) else {
                break;
            };
            if !go {
                break;
            }
        })
        .detach();
    }

    fn reveal(&mut self, cx: &mut Context<Self>) {
        let Some(g) = self.round.as_ref().and_then(|r| r["game"].as_str()).map(String::from) else { return };
        if self.busy {
            return;
        }
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let r = game("POST", "/reveal", json!({ "game": g })).await;
            let _ = this.update(cx, |s, cx| {
                s.busy = false;
                match r {
                    Ok(v) => {
                        s.stage = v["stage"].as_u64().unwrap_or(s.stage as u64) as usize;
                        s.replay(cx);
                    }
                    Err(e) => s.error = e,
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn answer(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(g) = self.round.as_ref().and_then(|r| r["game"].as_str()).map(String::from) else { return };
        if self.busy || self.result.is_some() {
            return;
        }
        self.busy = true;
        self.picked = Some(i);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = game("POST", "/answer", json!({ "game": g, "pick": i })).await;
            let _ = this.update(cx, |s, cx| {
                s.busy = false;
                match r {
                    Ok(v) => {
                        s.result = Some(v);
                        s.replay(cx); // now the whole scene
                    }
                    Err(e) => {
                        s.picked = None;
                        s.error = e;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn next(&mut self, cx: &mut Context<Self>) {
        let Some(res) = self.result.take() else { return };
        if !res["next"].is_null() {
            self.show(res["next"].clone(), cx);
        } else {
            self.finish(res["done"].clone(), cx);
        }
        cx.notify();
    }

    /// The game is over: the summary, and its card to share.
    fn finish(&mut self, done: Value, cx: &mut Context<Self>) {
        self.stop(cx);
        self.round = None;
        self.card = None;
        let name = couch::state::get().core.db.get::<Value>("discord_user").and_then(|u| u["name"].as_str().map(String::from)).unwrap_or_default();
        let day = nice_day(&chrono::Utc::now().format("%Y-%m-%d").to_string());
        let svg = card_svg(&done, &name, &day);
        self.done = Some(done);
        self.load(cx);
        cx.spawn(async move |this, cx| {
            let png = cx.background_executor().spawn(async move { crate::views::month::render(&svg) }).await;
            let _ = this.update(cx, |s, cx| {
                if let Some(b) = png {
                    s.card = Some((Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, b.clone())), b));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn quit(&mut self, cx: &mut Context<Self>) {
        let g = self.round.as_ref().and_then(|r| r["game"].as_str()).map(String::from);
        self.stop(cx);
        self.round = None;
        self.result = None;
        self.done = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let mut done = Value::Null;
            if let Some(g) = g {
                // today's five is only left (you can carry on later, on any
                // device); an endless run ends, and what you had counts
                if let Ok(v) = game("POST", "/quit", json!({ "game": g })).await {
                    done = v["done"].clone();
                }
            }
            let _ = this.update(cx, |s, cx| {
                if done.is_null() {
                    s.load(cx);
                } else {
                    s.finish(done, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ------------------------------------------------------------ pages
    // One centred column, like the iPhone app: cards in the lobby, then the
    // screen, the titles stacked under it, one big button at the bottom.

    fn lobby(&self, cx: &mut Context<Self>) -> Div {
        let mut col = div().w(px(COL)).flex().flex_col().gap(px(14.));
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .pb(px(6.))
                .child(div().text_size(px(40.)).font_weight(FontWeight::BLACK).child("scenes"))
                .child(div().text_size(px(15.)).text_color(theme::label2()).child("guess the movie from its most famous scene. the less you watch, the more you score.")),
        );
        let Some(h) = &self.home else {
            return col.child(div().text_color(theme::label2()).child(if self.error.is_empty() { "loading…".to_string() } else { self.error.clone() }));
        };
        if h["paused"].as_bool() == Some(true) {
            return col.child(card().child(div().text_color(theme::label2()).child("scenes is taking a break while we sort out where the clips come from. back soon.")));
        }
        let streak = h["streak"].as_i64().unwrap_or(0);
        let mut daily = card().child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().flex().items_center().gap(px(10.)).child(icon("calendar", 20.)).child(div().text_size(px(20.)).font_weight(FontWeight::BLACK).child("daily 5")))
                .child(
                    div()
                        .px(px(12.))
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .bg(theme::white(if streak > 0 { 0.14 } else { 0.06 }))
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(14.))
                        .text_color(if streak > 0 { theme::white(1.) } else { theme::white(0.4) })
                        .child(format!("{streak} day streak")),
                ),
        );
        if h["daily"]["played"].as_bool() == Some(true) {
            let left = h["nextIn"].as_i64().unwrap_or(0);
            daily = daily
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(10.))
                        .child(div().text_size(px(48.)).font_weight(FontWeight::BLACK).child(commas(h["daily"]["score"].as_i64().unwrap_or(0))))
                        .child(div().text_color(theme::label2()).child("points"))
                        .children(h["ranks"]["daily"].as_i64().map(|r| div().ml_auto().font_weight(FontWeight::BOLD).child(format!("#{r} today")))),
                )
                .child(dots(&h["daily"]["results"]))
                .child(div().text_size(px(13.)).text_color(theme::label2()).child(format!("next five in {}h {}m", left / 3600, (left % 3600) / 60)));
        } else if let Some(n) = h["daily"]["resume"]["n"].as_i64() {
            daily = daily
                .child(div().text_color(theme::label2()).child(format!(
                    "you're on scene {n} of {} with {} points. carry on here, or on any of your devices.",
                    h["daily"]["resume"]["total"].as_i64().unwrap_or(5),
                    commas(h["daily"]["resume"]["score"].as_i64().unwrap_or(0))
                )))
                .child(pill("sc-daily", "continue today's five", true).on_click(cx.listener(|s, _, _, cx| s.start(true, cx))));
        } else {
            daily = daily
                .child(div().text_color(theme::label2()).child("five scenes, the same for everyone. easy first, then nicher. one go a day."))
                .child(pill("sc-daily", "play today's five", true).on_click(cx.listener(|s, _, _, cx| s.start(true, cx))));
        }
        let best = h["bestRun"].as_i64().unwrap_or(0);
        let endless = card()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().flex().items_center().gap(px(10.)).child(icon("refresh", 20.)).child(div().text_size(px(20.)).font_weight(FontWeight::BLACK).child("endless")))
                    .children((best > 0).then(|| div().text_color(theme::label2()).child(format!("best {}", commas(best))))),
            )
            .child(div().text_color(theme::label2()).child("play whenever. it starts easy and gets nicher. three misses and the run is over."))
            .child(pill("sc-endless", "start a run", false).on_click(cx.listener(|s, _, _, cx| s.start(false, cx))));
        col = col.child(daily).child(endless);
        if !self.error.is_empty() {
            col = col.child(div().text_color(gpui::rgb(0xff8a8a)).child(self.error.clone()));
        }

        // the boards: a segmented switch, then rows
        let seg = |i: usize, label: &'static str, on: bool| {
            div()
                .id(("sc-tab", i))
                .flex_1()
                .h(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(9.))
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .when(on, |d| d.bg(theme::white(0.18)))
                .when(!on, |d| d.text_color(theme::label2()).hover(|s| s.text_color(theme::text())))
                .child(label)
        };
        let rows: Vec<Value> = h["boards"][if self.board == 0 { "daily" } else { "runs" }].as_array().cloned().unwrap_or_default();
        let me = couch::crew::member_id(&couch::state::get().core);
        col = col.child(div().mt(px(12.)).text_size(px(22.)).font_weight(FontWeight::BLACK).child("leaderboard")).child(
            div()
                .p(px(3.))
                .flex()
                .gap(px(3.))
                .rounded(px(12.))
                .bg(theme::white(0.07))
                .child(seg(0, "today", self.board == 0).on_click(cx.listener(|s, _, _, cx| {
                    s.board = 0;
                    cx.notify();
                })))
                .child(seg(1, "best runs", self.board == 1).on_click(cx.listener(|s, _, _, cx| {
                    s.board = 1;
                    cx.notify();
                }))),
        );
        if rows.is_empty() {
            col = col.child(div().py(px(8.)).text_color(theme::label2()).child(if self.board == 0 { "nobody has played today yet. be first." } else { "no runs yet." }));
        }
        col.children(rows.into_iter().enumerate().map(|(i, r)| {
            let mine = r["id"].as_str() == Some(me.as_str());
            div()
                .w_full()
                .h(px(46.))
                .px(px(12.))
                .flex()
                .items_center()
                .gap(px(12.))
                .rounded(px(12.))
                .when(mine, |d| d.bg(theme::white(0.08)))
                .child(div().w(px(26.)).text_color(if i < 3 { theme::white(1.) } else { theme::white(0.5) }).font_weight(FontWeight::BLACK).child((i + 1).to_string()))
                .child(match r["avatar"].as_str() {
                    Some(a) => img(a.to_string()).size(px(30.)).rounded_full().into_any_element(),
                    None => div().size(px(30.)).rounded_full().bg(theme::surface2()).into_any_element(),
                })
                .child(div().flex_1().min_w_0().truncate().when(mine, |d| d.font_weight(FontWeight::BOLD)).child(r["name"].as_str().unwrap_or("someone").to_string()))
                .child(div().font_weight(FontWeight::BOLD).child(commas(r["score"].as_i64().unwrap_or(0))))
        }))
    }

    fn table(&mut self, r: &Value, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let n = r["n"].as_i64().unwrap_or(1);
        let total = r["total"].as_i64().unwrap_or(0);
        let res = self.result.clone();
        let score = res.as_ref().map_or(&r["score"], |x| &x["score"]).as_i64().unwrap_or(0);
        let lives = res.as_ref().map_or(&r["lives"], |x| &x["lives"]).as_i64();
        let level = ["", "easy", "film fan", "deep cut"][r["level"].as_u64().unwrap_or(1).min(3) as usize];
        let stages: Vec<f64> = r["stages"].as_array().into_iter().flatten().filter_map(|s| s.as_f64()).collect();
        let points: Vec<i64> = r["points"].as_array().into_iter().flatten().filter_map(|s| s.as_i64()).collect();
        let worth = points.get(self.stage).copied().unwrap_or(0);

        // the screen: dark until you play your part of the scene. While the
        // clip is held on its first picture it's drawn (so its window is in
        // place) but covered; the cover comes off the moment it's let go.
        let frame = if self.on { video::get(cx).update(cx, |v, _| v.frame(OWNER, window)) } else { None };
        if self.on && self.held && frame.is_some() {
            self.frames = self.frames.saturating_add(1);
        }
        let (sw, sh) = (COL, COL * 9. / 16.);
        let mut screen = div().relative().w(px(sw)).h(px(sh)).rounded(px(16.)).overflow_hidden().flex().items_center().justify_center();
        let waiting = || div().text_size(px(14.)).text_color(theme::label2()).child("loading the scene…");
        screen = match &frame {
            Some(pic) => {
                let s = screen.children(video::layers(None, pic, 1., 16.));
                if self.held {
                    s.child(div().absolute().top_0().left_0().size_full().bg(gpui::rgb(0x0f0f10)).flex().items_center().justify_center().child(waiting()))
                } else {
                    s
                }
            }
            None => {
                let s = screen.bg(gpui::rgb(0x0f0f10));
                if self.clip_failed {
                    s.child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(10.))
                            .child(div().text_color(theme::label2()).child("couldn't load this scene"))
                            .child(pill("sc-retry", "try again", false).on_click(cx.listener(|s, _, _, cx| s.replay(cx)))),
                    )
                } else if self.on || self.clip.is_empty() {
                    s.child(waiting())
                } else {
                    s.child(
                        div()
                            .id("sc-again")
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(8.))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.8))
                            .on_click(cx.listener(|s, _, _, cx| s.replay(cx)))
                            .child(div().size(px(64.)).rounded_full().bg(theme::white(0.14)).flex().items_center().justify_center().child(icon("play", 26.)))
                            .child(div().text_size(px(13.)).text_color(theme::label2()).child(if res.is_some() { "watch the scene" } else { "play it again" })),
                    )
                }
            }
        };

        let hearts = lives.map(|l| div().flex().gap(px(3.)).children((0..3).map(move |i| if i < l { crate::ui::icon_fill("heart", 12.) } else { icon("heart", 12.).text_color(theme::white(0.35)) })));
        let mut col = div()
            .w(px(COL))
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .relative()
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .id("sc-quit")
                            .size(px(40.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(theme::white(0.1))
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::white(0.18)))
                            .on_click(cx.listener(|s, _, _, cx| s.quit(cx)))
                            .child(icon("close", 16.)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(div().font_weight(FontWeight::BOLD).child(if total > 0 { format!("scene {n} of {total}") } else { format!("scene {n}") }))
                            .child(div().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(gpui::rgb(match r["level"].as_i64().unwrap_or(1) { 2 => GOLD, 3 => VIOLET, _ => GREEN })).child(level)),
                    )
                    .child(div().flex().flex_col().items_end().gap(px(3.)).child(div().text_size(px(20.)).font_weight(FontWeight::BLACK).text_color(gpui::rgb(GOLD)).child(commas(score))).children(hearts)),
            )
            .child(screen);

        // what just happened, said loudly
        if let Some(x) = &res {
            let ok = x["correct"].as_bool().unwrap_or(false);
            let got = x["gained"].as_i64().unwrap_or(0);
            let praise = ["in a blink!", "one second!", "three seconds", "five seconds", "got it"][self.stage.min(4)];
            let line = if ok { if got > 0 { format!("{praise}   +{}", commas(got)) } else { praise.to_string() } } else { "not this time".to_string() };
            col = col.child(
                div()
                    .w_full()
                    .h(px(46.))
                    .rounded(px(12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(10.))
                    .font_weight(FontWeight::BLACK)
                    .text_size(px(17.))
                    .text_color(gpui::black())
                    .bg(gpui::rgb(if ok { GREEN } else { RED }))
                    .child(icon(if ok { "check" } else { "close" }, 18.).text_color(gpui::black()))
                    .child(line)
                    .with_animation(("sc-verdict", n as usize), gpui::Animation::new(std::time::Duration::from_millis(420)).with_easing(gpui::ease_out_quint()), |d, t| {
                        d.opacity(t).mt(px(-10. * (1. - t)))
                    }),
            );
        }

        if res.is_none() {
            col = col
                .child(div().flex().gap(px(6.)).children((0..stages.len()).map(|i| div().flex_1().h(px(4.)).rounded_full().bg(if i <= self.stage { gpui::rgb(GOLD).into() } else { theme::white(0.18) }))))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_size(px(13.))
                        .child(div().text_color(theme::label2()).child(match stages.get(self.stage).copied().unwrap_or(0.) {
                            s if s == 0. => "the whole scene".to_string(),
                            s if s < 1. => "a blink".to_string(),
                            s => format!("the first {} second{}", s as i64, if s == 1. { "" } else { "s" }),
                        }))
                        .child(div().font_weight(FontWeight::BOLD).text_color(gpui::rgb(GOLD)).child(format!("worth {}", commas(worth)))),
                );
        }

        // four titles, one guess
        let right = res.as_ref().and_then(|x| x["answer"]["index"].as_u64()).map(|i| i as usize);
        let gained = res.as_ref().and_then(|x| x["gained"].as_i64()).unwrap_or(0);
        let answered = res.is_some();
        let picked = self.picked;
        col = col.child(div().flex().flex_col().gap(px(8.)).children(r["options"].as_array().into_iter().flatten().enumerate().map(|(i, o)| {
            let is_right = right == Some(i);
            let is_wrong = answered && picked == Some(i) && !is_right;
            div()
                .id(("sc-opt", i))
                .w_full()
                .h(px(54.))
                .px(px(18.))
                .flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(12.))
                .font_weight(FontWeight::BOLD)
                .text_size(px(16.))
                .when(is_right, |d| d.bg(gpui::rgb(GREEN)).text_color(gpui::black()))
                .when(!is_right, |d| d.bg(theme::white(if picked == Some(i) { 0.16 } else { 0.08 })))
                .when(is_wrong, |d| d.border_2().border_color(gpui::rgb(RED)).text_color(gpui::rgb(RED)))
                .when(answered && !is_right && !is_wrong, |d| d.opacity(0.4))
                .when(!answered, |d| d.cursor_pointer().hover(|s| s.bg(theme::white(0.16))).on_click(cx.listener(move |s, _, _, cx| s.answer(i, cx))))
                .child(div().min_w_0().truncate().child(o["title"].as_str().unwrap_or("").to_string()))
                .child(div().flex_1().text_size(px(13.)).opacity(0.6).child(o["year"].as_str().unwrap_or("").to_string()))
                .children((is_right && gained > 0).then(|| div().child(format!("+{}", commas(gained)))))
                .children(is_right.then(|| icon("check", 18.).text_color(gpui::black())))
                .children(is_wrong.then(|| icon("close", 16.)))
        })));

        if !self.error.is_empty() {
            col = col.child(div().text_color(gpui::rgb(0xff8a8a)).child(self.error.clone()));
        }
        if let Some(x) = &res {
            let last = x["next"].is_null();
            col = col.child(div().pt(px(6.)).child(pill("sc-next", if last { "see how you did" } else { "next scene" }, true).on_click(cx.listener(|s, _, _, cx| s.next(cx)))));
        } else if self.stage + 1 < stages.len() {
            let cost = worth - points.get(self.stage + 1).copied().unwrap_or(0);
            col = col.child(div().pt(px(6.)).child(pill("sc-more", format!("show more  (−{cost})"), false).on_click(cx.listener(|s, _, _, cx| s.reveal(cx)))));
        }
        col
    }

    fn summary(&self, d: &Value, cx: &mut Context<Self>) -> Div {
        let daily = d["mode"].as_str() == Some("daily");
        let score = d["score"].as_i64().unwrap_or(0);
        let mut col = div().w(px(COL)).pt(px(10.)).flex().flex_col().items_center().gap(px(14.));
        // (the card is only what gets shared: copy / save. It isn't shown here.)
        col = col
            .child(div().text_color(theme::label2()).font_weight(FontWeight::BOLD).child(if daily { "today's five" } else { "run over" }))
            .child(div().text_size(px(92.)).font_weight(FontWeight::BLACK).text_color(gpui::rgb(GOLD)).child(commas(score)))
            .child(div().text_color(theme::label2()).child(format!("points · {} of {} right", d["right"].as_i64().unwrap_or(0), d["rounds"].as_i64().unwrap_or(0))))
            .child(dots(&d["results"]));
        let chip = |text: String, c: u32| div().px(px(12.)).py(px(6.)).rounded_full().bg(gpui::rgba((c << 8) | 0x2a)).text_color(gpui::rgb(c)).font_weight(FontWeight::BOLD).text_size(px(13.)).child(text);
        let mut chips = div().flex().gap(px(8.));
        if daily && d["streak"].as_i64().unwrap_or(0) > 0 {
            chips = chips.child(chip(format!("{} day streak", d["streak"].as_i64().unwrap_or(0)), FLAME));
        }
        if let Some(r) = d["rank"].as_i64() {
            chips = chips.child(chip(format!("#{r} {}", if daily { "today" } else { "all time" }), GOLD));
        }
        if !daily && score > 0 && score >= d["bestRun"].as_i64().unwrap_or(0) {
            chips = chips.child(chip("your best run".into(), VIOLET));
        }
        col = col.child(chips);
        let mut row = div().w_full().mt(px(8.)).flex().gap(px(10.));
        if self.card.is_some() {
            row = row
                .child(div().flex_1().child(pill("sc-copy", "copy card", false).on_click(cx.listener(|s, _, _, cx| {
                    if let Some((i, _)) = &s.card {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_image(i));
                        crate::Nav::toast("copied. paste it in Discord".into(), cx);
                    }
                }))))
                .child(div().flex_1().child(pill("sc-save", "save", false).on_click(cx.listener(|s, _, _, cx| {
                    let Some((_, b)) = s.card.clone() else { return };
                    let dir = dirs::picture_dir().unwrap_or_else(std::env::temp_dir).join("couch");
                    let name = format!("scenes-{}.png", chrono::Utc::now().format("%Y-%m-%d-%H%M"));
                    let ok = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(&name), b)).is_ok();
                    crate::Nav::toast(if ok { format!("saved to Pictures\\couch\\{name}") } else { "couldn't save it".into() }, cx);
                }))));
        }
        if !daily {
            row = row.child(div().flex_1().child(pill("sc-again-run", "go again", false).on_click(cx.listener(|s, _, _, cx| s.start(false, cx)))));
        }
        col.child(row).child(pill("sc-done", "done", true).on_click(cx.listener(|s, _, _, cx| {
            s.done = None;
            s.card = None;
            cx.notify();
        })))
    }
}

/// the width of the one column everything sits in
const COL: f32 = 620.;

impl Render for Scenes {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = if let Some(r) = self.round.clone() {
            self.table(&r, window, cx)
        } else if let Some(d) = self.done.clone() {
            self.summary(&d, cx)
        } else {
            self.lobby(cx)
        };
        div()
            .id("scenes")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "scenes"))
            .pt(px(96.))
            .px(px(theme::GUTTER))
            .pb(px(80.))
            .flex()
            .flex_col()
            .items_center()
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    /// `cargo test scenes_card -- --nocapture` writes the share card to /tmp
    #[test]
    fn scenes_card() {
        let d = serde_json::json!({ "mode": "daily", "score": 3800, "right": 4, "rounds": 5, "results": [0, 1, -1, 0, 2], "streak": 3, "rank": 2 });
        let svg = super::card_svg(&d, "xnite", "28 Nov 2026");
        assert!(svg.contains("3,800") && svg.contains("3 day streak") && svg.contains("#2 today"));
    }
}
