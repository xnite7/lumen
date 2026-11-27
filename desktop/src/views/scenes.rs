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
use gpui::{div, img, prelude::*, px, Context, Div, FontWeight, ScrollHandle, SharedString, Stateful, Window};
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
    /// its first picture showed, and how long it may show (None = to the end)
    play_no: u64,
    on: bool,
    asked: Option<Instant>,
    seen: Option<Instant>,
    limit: Option<f32>,
    clip_failed: bool,
    scroll: ScrollHandle,
}

async fn game(method: &str, path: &str, body: Value) -> Result<Value, String> {
    api::call::<Value>("crew_social", json!({ "method": method, "path": format!("/v1/game{path}"), "body": body })).await
}

fn pill(id: &'static str, label: impl Into<SharedString>, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(46.))
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

/// one square per scene: brighter = got it sooner, hollow = missed
fn dots(results: &Value) -> Div {
    div().flex().gap(px(6.)).children(results.as_array().into_iter().flatten().map(|r| {
        let r = r.as_i64().unwrap_or(-1);
        let d = div().size(px(24.)).rounded(px(6.));
        if r < 0 {
            d.border_2().border_color(theme::white(0.3))
        } else {
            d.bg(theme::white(1. - r as f32 * 0.18))
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
        // (a blink is 0.1 s on paper; a GPU window needs a moment to appear)
        (s > 0.).then_some(s.max(0.2))
    }

    /// Play the part you've earned, from the start; then the screen goes dark.
    fn replay(&mut self, cx: &mut Context<Self>) {
        if self.clip.is_empty() {
            return;
        }
        self.play_no += 1;
        let play_no = self.play_no;
        self.on = true;
        self.asked = Some(Instant::now());
        self.seen = None;
        self.limit = self.secs();
        self.clip_failed = false;
        let url = self.clip.clone();
        video::get(cx).update(cx, |v, cx| {
            v.stop(OWNER, cx);
            v.play_once(OWNER, &url, cx);
        });
        cx.notify();
        cx.spawn(async move |this, cx| loop {
            crate::timer::after(Duration::from_millis(16)).await;
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
                let over = match (s.seen, s.limit) {
                    (Some(t), Some(l)) => t.elapsed().as_secs_f32() >= l,
                    _ => false,
                };
                // nothing to see after 20 s: the scene didn't load
                let stuck = s.seen.is_none() && s.asked.map_or(false, |t| t.elapsed().as_secs() >= 20);
                if over || stuck {
                    vid.update(cx, |v, cx| v.stop(OWNER, cx));
                    s.on = false;
                    s.clip_failed = stuck;
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
            self.stop(cx);
            self.round = None;
            self.done = Some(res["done"].clone());
            self.load(cx);
        }
        cx.notify();
    }

    fn quit(&mut self, cx: &mut Context<Self>) {
        let g = self.round.as_ref().and_then(|r| r["game"].as_str()).map(String::from);
        self.stop(cx);
        self.round = None;
        self.result = None;
        self.done = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            if let Some(g) = g {
                let _ = game("POST", "/quit", json!({ "game": g })).await;
            }
            let _ = this.update(cx, |s, cx| s.load(cx));
        })
        .detach();
    }

    // ------------------------------------------------------------ pages

    fn lobby(&self, cx: &mut Context<Self>) -> Div {
        let mut col = div().w_full().max_w(px(760.)).flex().flex_col().gap(px(16.));
        let Some(h) = &self.home else {
            return col.child(div().text_color(theme::label2()).child(if self.error.is_empty() { "loading…".to_string() } else { self.error.clone() }));
        };
        let streak = h["streak"].as_i64().unwrap_or(0);
        let mut daily = card().child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_size(px(20.)).font_weight(FontWeight::BLACK).child("daily 5"))
                .child(div().font_weight(FontWeight::BOLD).text_color(if streak > 0 { theme::white(1.) } else { theme::white(0.4) }).child(format!("{streak} day streak"))),
        );
        if h["daily"]["played"].as_bool() == Some(true) {
            let left = h["nextIn"].as_i64().unwrap_or(0);
            daily = daily
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(10.))
                        .child(div().text_size(px(44.)).font_weight(FontWeight::BLACK).child(h["daily"]["score"].as_i64().unwrap_or(0).to_string()))
                        .child(div().text_color(theme::label2()).child("points"))
                        .children(h["ranks"]["daily"].as_i64().map(|r| div().ml_auto().font_weight(FontWeight::BOLD).child(format!("#{r} today")))),
                )
                .child(dots(&h["daily"]["results"]))
                .child(div().text_size(px(13.)).text_color(theme::label2()).child(format!("next five in {}h {}m", left / 3600, (left % 3600) / 60)));
        } else {
            daily = daily
                .child(div().text_color(theme::label2()).child("five scenes, the same for everyone. easy first, then nicher. one go a day."))
                .child(div().flex().child(pill("sc-daily", "play today's five", true).on_click(cx.listener(|s, _, _, cx| s.start(true, cx)))));
        }
        let best = h["bestRun"].as_i64().unwrap_or(0);
        let endless = card()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(20.)).font_weight(FontWeight::BLACK).child("endless"))
                    .children((best > 0).then(|| div().text_color(theme::label2()).child(format!("best {best}")))),
            )
            .child(div().text_color(theme::label2()).child("play whenever. it starts easy and gets nicher. three misses and the run is over."))
            .child(div().flex().child(pill("sc-endless", "start a run", false).on_click(cx.listener(|s, _, _, cx| s.start(false, cx)))));
        col = col.child(daily).child(endless);
        if !self.error.is_empty() {
            col = col.child(div().text_color(gpui::rgb(0xff8a8a)).child(self.error.clone()));
        }

        // the boards
        let tab = |i: usize, label: &'static str, on: bool| {
            div()
                .id(("sc-tab", i))
                .px(px(16.))
                .h(px(34.))
                .flex()
                .items_center()
                .rounded_full()
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .when(on, |d| d.bg(gpui::white()).text_color(gpui::black()))
                .when(!on, |d| d.bg(theme::white(0.08)).hover(|s| s.bg(theme::white(0.14))))
                .child(label)
        };
        let rows = &h["boards"][if self.board == 0 { "daily" } else { "runs" }];
        let rows: Vec<Value> = rows.as_array().cloned().unwrap_or_default();
        let me = couch::crew::member_id(&couch::state::get().core);
        col = col
            .child(div().mt(px(10.)).text_size(px(22.)).font_weight(FontWeight::BLACK).child("leaderboard"))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(tab(0, "today", self.board == 0).on_click(cx.listener(|s, _, _, cx| {
                        s.board = 0;
                        cx.notify();
                    })))
                    .child(tab(1, "best runs", self.board == 1).on_click(cx.listener(|s, _, _, cx| {
                        s.board = 1;
                        cx.notify();
                    }))),
            );
        if rows.is_empty() {
            col = col.child(div().text_color(theme::label2()).child(if self.board == 0 { "nobody has played today yet. be first." } else { "no runs yet." }));
        }
        col.children(rows.into_iter().enumerate().map(|(i, r)| {
            let mine = r["id"].as_str() == Some(me.as_str());
            div()
                .w_full()
                .h(px(44.))
                .px(px(12.))
                .flex()
                .items_center()
                .gap(px(12.))
                .rounded(px(10.))
                .when(mine, |d| d.bg(theme::white(0.08)))
                .child(div().w(px(28.)).text_color(if i < 3 { theme::white(1.) } else { theme::white(0.5) }).font_weight(FontWeight::BLACK).child((i + 1).to_string()))
                .child(match r["avatar"].as_str() {
                    Some(a) => img(a.to_string()).size(px(28.)).rounded_full().into_any_element(),
                    None => div().size(px(28.)).rounded_full().bg(theme::surface2()).into_any_element(),
                })
                .child(div().flex_1().min_w_0().truncate().when(mine, |d| d.font_weight(FontWeight::BOLD)).child(r["name"].as_str().unwrap_or("someone").to_string()))
                .child(div().font_weight(FontWeight::BOLD).child(r["score"].as_i64().unwrap_or(0).to_string()))
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

        // the screen: dark until you play your part of the scene
        let frame = if self.on { video::get(cx).update(cx, |v, _| v.frame(OWNER, window)) } else { None };
        if self.on && self.seen.is_none() && frame.is_some() {
            self.seen = Some(Instant::now());
        }
        let mut screen = div().relative().w(px(760.)).h(px(428.)).rounded(px(16.)).overflow_hidden().flex().items_center().justify_center();
        screen = match &frame {
            Some(pic) => screen.children(video::layers(None, pic, 1., 16.)),
            None => {
                let s = screen.bg(theme::white(0.05));
                if self.clip_failed {
                    s.child(div().text_color(theme::label2()).child("couldn't load this scene"))
                } else if self.on || self.clip.is_empty() {
                    s.child(div().text_color(theme::label2()).child("loading the scene…"))
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

        let mut col = div()
            .w(px(760.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .id("sc-quit")
                            .px(px(14.))
                            .h(px(34.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .rounded_full()
                            .bg(theme::white(0.08))
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::white(0.14)))
                            .on_click(cx.listener(|s, _, _, cx| s.quit(cx)))
                            .child(icon("close", 14.))
                            .child("quit"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(div().font_weight(FontWeight::BOLD).child(if total > 0 { format!("scene {n} of {total}") } else { format!("scene {n}") }))
                            .child(div().text_size(px(12.)).text_color(theme::label2()).child(level)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .child(div().text_size(px(20.)).font_weight(FontWeight::BLACK).child(score.to_string()))
                            .children(lives.map(|l| div().text_size(px(12.)).text_color(theme::label2()).child(format!("{l} {} left", if l == 1 { "life" } else { "lives" })))),
                    ),
            )
            .child(screen);

        if res.is_none() {
            col = col
                .child(div().flex().gap(px(6.)).children((0..stages.len()).map(|i| div().flex_1().h(px(4.)).rounded_full().bg(if i <= self.stage { theme::white(1.) } else { theme::white(0.18) }))))
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
                        .child(div().font_weight(FontWeight::BOLD).child(format!("worth {worth}"))),
                );
        }

        // four titles, one guess
        let right = res.as_ref().and_then(|x| x["answer"]["index"].as_u64()).map(|i| i as usize);
        let gained = res.as_ref().and_then(|x| x["gained"].as_i64()).unwrap_or(0);
        let answered = res.is_some();
        let picked = self.picked;
        col = col.child(div().flex().flex_wrap().gap(px(10.)).children(r["options"].as_array().into_iter().flatten().enumerate().map(|(i, o)| {
            let is_right = right == Some(i);
            let is_wrong = answered && picked == Some(i) && !is_right;
            div()
                .id(("sc-opt", i))
                .w(px(375.))
                .min_h(px(56.))
                .px(px(18.))
                .flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(12.))
                .font_weight(FontWeight::BOLD)
                .when(is_right, |d| d.bg(gpui::white()).text_color(gpui::black()))
                .when(!is_right, |d| d.bg(theme::white(if picked == Some(i) { 0.16 } else { 0.08 })))
                .when(is_wrong, |d| d.border_2().border_color(theme::white(0.6)))
                .when(answered && !is_right && !is_wrong, |d| d.opacity(0.4))
                .when(!answered, |d| d.cursor_pointer().hover(|s| s.bg(theme::white(0.16))).on_click(cx.listener(move |s, _, _, cx| s.answer(i, cx))))
                .child(div().flex_1().min_w_0().truncate().child(o["title"].as_str().unwrap_or("").to_string()))
                .child(div().text_size(px(13.)).opacity(0.6).child(o["year"].as_str().unwrap_or("").to_string()))
                .children((is_right && gained > 0).then(|| div().child(format!("+{gained}"))))
                .children(is_right.then(|| icon("check", 18.).text_color(gpui::black())))
                .children(is_wrong.then(|| icon("close", 16.)))
        })));

        if !self.error.is_empty() {
            col = col.child(div().text_color(gpui::rgb(0xff8a8a)).child(self.error.clone()));
        }
        if let Some(x) = &res {
            let last = x["next"].is_null();
            col = col.child(div().flex().child(pill("sc-next", if last { "see how you did" } else { "next scene" }, true).on_click(cx.listener(|s, _, _, cx| s.next(cx)))));
        } else if self.stage + 1 < stages.len() {
            let cost = worth - points.get(self.stage + 1).copied().unwrap_or(0);
            col = col.child(div().flex().child(pill("sc-more", format!("show more  (−{cost})"), false).on_click(cx.listener(|s, _, _, cx| s.reveal(cx)))));
        }
        col
    }

    fn summary(&self, d: &Value, cx: &mut Context<Self>) -> Div {
        let daily = d["mode"].as_str() == Some("daily");
        let score = d["score"].as_i64().unwrap_or(0);
        let mut col = div()
            .w_full()
            .max_w(px(520.))
            .pt(px(30.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(14.))
            .child(div().text_color(theme::label2()).font_weight(FontWeight::BOLD).child(if daily { "today's five" } else { "run over" }))
            .child(div().text_size(px(84.)).font_weight(FontWeight::BLACK).child(score.to_string()))
            .child(div().text_color(theme::label2()).child(format!("{} of {} right", d["right"].as_i64().unwrap_or(0), d["rounds"].as_i64().unwrap_or(0))));
        if daily {
            col = col.child(dots(&d["results"])).child(div().font_weight(FontWeight::BOLD).child(format!("{} day streak", d["streak"].as_i64().unwrap_or(0))));
        } else if score > 0 && score >= d["bestRun"].as_i64().unwrap_or(0) {
            col = col.child(div().font_weight(FontWeight::BOLD).child("your best run"));
        }
        if let Some(r) = d["rank"].as_i64() {
            col = col.child(div().font_weight(FontWeight::SEMIBOLD).child(format!("#{r} on the leaderboard")));
        }
        let mut row = div().mt(px(14.)).flex().gap(px(10.));
        if !daily {
            row = row.child(pill("sc-again-run", "go again", false).on_click(cx.listener(|s, _, _, cx| s.start(false, cx))));
        }
        col.child(row.child(pill("sc-done", "done", true).on_click(cx.listener(|s, _, _, cx| {
            s.done = None;
            cx.notify();
        }))))
    }
}

impl Render for Scenes {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = if let Some(r) = self.round.clone() {
            self.table(&r, window, cx)
        } else if let Some(d) = self.done.clone() {
            self.summary(&d, cx)
        } else {
            self.lobby(cx)
        };
        let playing = self.round.is_some();
        div()
            .id("scenes")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "scenes"))
            .pt(px(100.))
            .px(px(theme::GUTTER))
            .pb(px(80.))
            .flex()
            .flex_col()
            .gap(px(18.))
            .when(!playing, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_size(px(40.)).font_weight(FontWeight::BLACK).child("scenes"))
                        .child(div().text_size(px(15.)).text_color(theme::label2()).child("guess the movie from its most famous scene. the less you watch, the more you score.")),
                )
            })
            .child(body)
    }
}
