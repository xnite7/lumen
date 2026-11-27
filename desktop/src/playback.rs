//! Playing something: find the streams, rank them for your connection,
//! hand the best to mpv, fall back down the list when one won't start.
//! The same rules the webview used (quickplay.ts, autopick.ts).
use crate::api::{self, Meta};
use crate::host;
use futures::StreamExt;
use gpui::{App, AppContext, Context, Entity, Global};
use serde_json::{json, Value};
use std::time::Duration;

/// Stop playing: when this episode ends, or at a time (Instant, minutes asked)
#[derive(Clone, Copy, PartialEq)]
pub enum Sleep {
    Episode,
    At(std::time::Instant, i64),
}

#[derive(Default, Clone)]
pub struct State {
    pub pos: f64,
    pub duration: f64,
    pub paused: bool,
    pub buffering: bool,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub cache_ahead: f64,
    #[allow(dead_code)] // (the picked tracks, as mpv reports them)
    pub aid: String,
    #[allow(dead_code)]
    pub sid: String,
    pub ended: bool,
    pub sub_delay: f64,
    /// the picture's shown size (after crops), for fitting it in the window
    pub dw: f64,
    pub dh: f64,
}

#[derive(Default)]
pub struct Playback {
    pub open: bool,
    pub loading: bool,
    pub error: String,
    pub meta: Option<Meta>,
    pub full: Option<Value>,
    pub video_id: String,
    pub episode_title: String,
    pub stream: Option<Value>,
    pub queue: Vec<Value>,
    /// every stream found for what's on, best first (the sources menu)
    pub all: Vec<Value>,
    /// stalls since this stream got going (lag watch)
    lag: Lag,
    last_seek: std::cell::Cell<Option<std::time::Instant>>,
    pub state: State,
    pub tracks: Vec<Value>,
    /// subtitles turned off: lines still arriving are ignored
    pub subs_off: bool,
    pub sub: String,
    pub status: String,
    /// skip segments from TheIntroDB: { intro: [{start,end}], recap, credits }
    pub segments: Value,
    skipped: Vec<String>,
    pub sleep: Option<Sleep>,
    /// since loading began, mpv has shown it's on the new file (no length yet, or buffering):
    /// only then can a "ready" state end the loading screen (old states arrive late)
    fresh: bool,
    /// bumped by every new play and every close: a load that finishes after
    /// you've moved on to something else is dropped, not played
    pub load_gen: u64,
    /// where the current load will start (for watch together, while loading)
    pub start_hint: Option<f64>,
    /// the next episode whose streams are being / were found in the background
    preload_for: Option<String>,
    /// the episode whose credits were already listened for
    ear_asked: String,
    /// the languages this show was last watched in, still being put back (since when)
    langs_pending: Option<std::time::Instant>,
    /// you waved off the next-episode countdown: no autoplay at the end either
    pub no_autonext: bool,
}

pub struct PlaybackHandle(pub Entity<Playback>);
impl Global for PlaybackHandle {}

/// This title is playing right now (full screen or in the corner)?
pub fn playing_title(id: &str, cx: &App) -> bool {
    let base = |s: &str| s.split(':').next().unwrap_or("").to_string();
    let p = get(cx).read(cx);
    p.open && p.error.is_empty() && p.meta.as_ref().map_or(false, |m| base(&m.id) == base(id))
}

pub fn get(cx: &App) -> Entity<Playback> {
    cx.global::<PlaybackHandle>().0.clone()
}

pub fn init(cx: &mut App) {
    let pb = cx.new(|cx: &mut Context<Playback>| {
        // mirror the player's state
        let mut st = host::listen("player://state");
        cx.spawn(async move |this, cx| {
            while let Some(v) = st.next().await {
                let s = State {
                    pos: v["pos"].as_f64().unwrap_or(0.),
                    duration: v["duration"].as_f64().unwrap_or(0.),
                    paused: v["paused"].as_bool().unwrap_or(true),
                    buffering: v["buffering"].as_bool().unwrap_or(false),
                    volume: v["volume"].as_f64().unwrap_or(100.),
                    muted: v["muted"].as_bool().unwrap_or(false),
                    speed: v["speed"].as_f64().unwrap_or(1.),
                    cache_ahead: v["cacheAhead"].as_f64().unwrap_or(0.),
                    aid: v["aid"].as_str().map(|s| s.to_string()).unwrap_or_else(|| v["aid"].to_string()),
                    sid: v["sid"].as_str().map(|s| s.to_string()).unwrap_or_else(|| v["sid"].to_string()),
                    ended: v["ended"].as_bool().unwrap_or(false),
                    sub_delay: v["subDelay"].as_f64().unwrap_or(0.),
                    dw: v["video"]["dw"].as_f64().unwrap_or(0.),
                    dh: v["video"]["dh"].as_f64().unwrap_or(0.),
                };
                let _ = this.update(cx, |p, cx| {
                    if p.loading && (s.duration <= 0. || s.buffering) {
                        p.fresh = true;
                    }
                    if p.loading && p.fresh && !s.buffering && s.duration > 0. {
                        p.loading = false;
                    }
                    // auto-skip intros / recaps when asked to
                    let pos = s.pos;
                    if let Some((kind, end)) = p.segment_at(pos) {
                        let set = couch::state::get().core.settings();
                        // intros: never in films, never the first episode of a season
                        // (you hear it once), every episode after that to the season's end
                        let ep = p.video_id.rsplit(':').next().and_then(|e| e.parse::<i64>().ok());
                        let later_ep = p.meta.as_ref().map_or(false, |m| m.kind == "series") && p.video_id.split(':').count() >= 3 && ep.map_or(false, |e| e >= 2);
                        let auto = (kind == "intro" && later_ep) || (kind == "recap" && set.playback.auto_skip_recap);
                        let key = format!("{kind}{}", end as i64);
                        if auto && !p.skipped.contains(&key) {
                            p.skipped.push(key);
                            p.cmd("seek", json!({ "to": end }));
                        }
                    }
                    p.watch_lag(&s, cx);
                    // the last ten minutes, and no credits known: listen for them (once)
                    if p.open && !p.loading && s.duration > 900. && s.duration - s.pos < 600. && p.video_id.split(':').count() == 3 && p.ear_asked != p.video_id {
                        let no_credits = p.segments["credits"].as_array().map_or(true, |a| a.is_empty());
                        p.ear_asked = p.video_id.clone();
                        if no_credits {
                            let vid = p.video_id.clone();
                            cx.spawn(async move |this, cx| {
                                let r = api::call::<Value>("ear_find", json!({ "kind": "credits" })).await.unwrap_or(Value::Null);
                                if r.is_object() {
                                    let _ = this.update(cx, |p, cx| p.add_segment("credits", &vid, r, cx));
                                }
                            })
                            .detach();
                        }
                    }
                    // halfway through: find the next episode's streams now, so it starts at once
                    if p.open && !p.loading && s.duration > 0. && s.pos / s.duration >= 0.5 && crate::together::get(cx).read(cx).following.is_none() {
                        if let Some(n) = p.next_video() {
                            let nid = n["id"].as_str().unwrap_or("").to_string();
                            if !nid.is_empty() && p.preload_for.as_deref() != Some(nid.as_str()) {
                                p.preload_for = Some(nid.clone());
                                let g = p.load_gen;
                                let rt = n.get("runtime").cloned().or_else(|| p.full.as_ref().map(|f| f["runtime"].clone())).unwrap_or(Value::Null);
                                cx.spawn(async move |this, cx| {
                                    let order = ranked("series".into(), nid.clone(), rt, None).await;
                                    // still on the same thing (not closed, not moved on): keep it
                                    let ok = this.update(cx, |p, _| p.open && p.load_gen == g).unwrap_or(false);
                                    if ok && !order.is_empty() {
                                        *PRELOAD.lock() = Some((nid, order, std::time::Instant::now()));
                                    }
                                })
                                .detach();
                            }
                        }
                    }
                    // reached the end: the next episode by itself
                    let finished = s.ended && !p.state.ended;
                    p.state = s;
                    if finished && p.open {
                        if p.sleep == Some(Sleep::Episode) {
                            // sleep timer: that was the last one tonight
                            p.sleep = None;
                        } else if couch::state::get().core.settings().playback.autoplay_next && !p.no_autonext && crate::together::get(cx).read(cx).following.is_none() {
                            // (on someone's couch the host's next episode brings you along)
                            p.play_next(cx);
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
        let mut tr = host::listen("player://tracks");
        cx.spawn(async move |this, cx| {
            while let Some(v) = tr.next().await {
                let _ = this.update(cx, |p, cx| {
                    p.tracks = v.as_array().cloned().unwrap_or_default();
                    p.apply_langs(cx);
                    cx.notify();
                });
            }
        })
        .detach();
        let mut sub = host::listen("player://sub");
        cx.spawn(async move |this, cx| {
            while let Some(v) = sub.next().await {
                let _ = this.update(cx, |p, cx| {
                    p.sub = if p.subs_off { String::new() } else { clean_cue(v["text"].as_str().unwrap_or("")) };
                    cx.notify();
                });
            }
        })
        .detach();
        let mut loaded = host::listen("player://loaded");
        cx.spawn(async move |this, cx| {
            while loaded.next().await.is_some() {
                let vid = this
                    .update(cx, |p, cx| {
                        if p.stream.is_none() {
                            return String::new();
                        }
                        p.loading = false;
                        p.segments = Value::Null;
                        p.skipped.clear();
                        cx.notify();
                        p.video_id.clone()
                    })
                    .unwrap_or_default();
                // headphone surround: set up for this film's channels once its sound is going
                if crate::views::player::headphones_on() {
                    couch::rt::spawn(async {
                        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                        let _ = couch::dispatch::invoke("player_cmd", json!({ "cmd": "headphones", "args": { "value": true } })).await;
                    });
                }
                if vid.is_empty() || !couch::state::get().core.settings().integrations.tidb_enabled {
                    continue;
                }
                // with the file's length, TheIntroDB matches the right cut (a
                // director's cut / extended episode has its intro elsewhere)
                let mut dur = 0.;
                for _ in 0..30 {
                    dur = this.update(cx, |p, _| if p.video_id == vid { p.state.duration } else { -1. }).unwrap_or(-1.);
                    if dur != 0. {
                        break;
                    }
                    crate::timer::after(std::time::Duration::from_millis(500)).await;
                }
                if dur < 0. {
                    continue;
                }
                let dms = (dur > 0.).then(|| (dur * 1000.) as u64);
                let mut segs = api::call::<Value>("skip_segments", json!({ "videoId": vid, "durationMs": dms })).await.unwrap_or(Value::Null);
                // nothing for this length: what's known for the episode at all
                let empty = segs.as_object().map_or(true, |o| o.values().all(|l| l.as_array().map_or(true, |a| a.is_empty())));
                if empty && dms.is_some() {
                    segs = api::call::<Value>("skip_segments", json!({ "videoId": vid, "durationMs": Value::Null })).await.unwrap_or(Value::Null);
                }
                let no_intro = segs["intro"].as_array().map_or(true, |a| a.is_empty());
                let early = this
                    .update(cx, |p, cx| {
                        if p.video_id == vid {
                            p.segments = segs;
                            cx.notify();
                        }
                        p.state.pos < 300.
                    })
                    .unwrap_or(false);
                // TheIntroDB doesn't know this one: listen for the intro (from the start
                // of an episode, while it's still ahead)
                if no_intro && early && vid.split(':').count() == 3 {
                    let v2 = vid.clone();
                    let this2 = this.clone();
                    cx.spawn(async move |cx| {
                        let r = api::call::<Value>("ear_find", json!({ "kind": "intro" })).await.unwrap_or(Value::Null);
                        if r.is_object() {
                            let _ = this2.update(cx, |p, cx| p.add_segment("intro", &v2, r, cx));
                        }
                    })
                    .detach();
                }
            }
        })
        .detach();
        // a stream that dies in the first seconds: try the next one
        for ev in ["player://error", "player://end"] {
            let mut rx = host::listen(ev);
            cx.spawn(async move |this, cx| {
                while let Some(v) = rx.next().await {
                    let msg = v["message"].as_str().or(v["reason"].as_str()).unwrap_or("").to_string();
                    if ev == "player://end" && !msg.to_lowercase().contains("error") {
                        continue;
                    }
                    let _ = this.update(cx, |p, cx| {
                        if !p.open {
                            return;
                        }
                        if p.state.pos < 5. && !p.queue.is_empty() {
                            p.try_next(cx);
                        } else {
                            p.loading = false;
                            p.error = if msg.is_empty() { "the tv got scared. that stream stopped.".into() } else { msg };
                        }
                        cx.notify();
                    });
                }
            })
            .detach();
        }
        Playback { state: State { volume: 100., speed: 1., ..Default::default() }, ..Default::default() }
    });
    cx.set_global(PlaybackHandle(pb));
}

fn clean_cue(t: &str) -> String {
    // ASS override tags and HTML-ish markup out; \N line breaks in
    let mut out = String::new();
    let mut depth = 0;
    for ch in t.replace("\\N", "\n").replace("\\n", "\n").chars() {
        match ch {
            '{' | '<' => depth += 1,
            '}' | '>' if depth > 0 => depth -= 1,
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let mut text = fix_mojibake(out.trim());
    // hide [sound descriptions]: [MUSIC], (DOOR SLAMS), and "JOHN:" speaker names
    if couch::state::get().core.settings().subtitles.hide_sdh {
        text = strip_sdh(&text);
    }
    rtl_visual(&text)
}

fn strip_sdh(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for ch in s.chars() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' if depth > 0 => depth -= 1,
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.lines()
        .map(|l| {
            let l = l.trim();
            // a speaker's name in capitals before a colon
            match l.split_once(':') {
                Some((who, rest)) if who.len() <= 24 && !who.is_empty() && who.chars().all(|c| c.is_uppercase() || c.is_whitespace() || c == '.' || c == '\'' || c == '-') => rest.trim().to_string(),
                _ => l.to_string(),
            }
        })
        .map(|l| l.trim_start_matches(['-', ' ']).to_string())
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Arabic, Persian, Hebrew lines, ready for a renderer that only lays text
/// out left to right (couch's draws every run left to right, so Arabic came
/// out back to front and in pieces): the letters are put in their joined
/// forms, the line is put in the order it's seen in (right to left, numbers
/// and Latin words kept the right way round), and it's held left to right
/// so nothing turns it round again.
pub fn rtl_visual(s: &str) -> String {
    let rtl = |c: char| matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF);
    if !s.chars().any(rtl) {
        return s.to_string();
    }
    s.lines()
        .map(|line| {
            if !line.chars().any(rtl) {
                return line.to_string();
            }
            let shaped = ar_reshaper::reshape_line(line);
            let bi = unicode_bidi::BidiInfo::new(&shaped, Some(unicode_bidi::Level::rtl()));
            let Some(para) = bi.paragraphs.first() else { return shaped };
            let seen: String = bi.reorder_line(para, para.range.clone()).into_owned();
            // brackets face the other way once the line is turned
            let seen: String = seen
                .chars()
                .map(|c| match c {
                    '(' => ')',
                    ')' => '(',
                    '[' => ']',
                    ']' => '[',
                    '«' => '»',
                    '»' => '«',
                    c => c,
                })
                .collect();
            format!("\u{202D}{seen}\u{202C}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Text that was UTF-8 but read as Windows-1252 ("â™ª" for ♪, "Ã©" for é):
/// put it back together. Anything that doesn't decode cleanly is left alone.
pub fn fix_mojibake(s: &str) -> String {
    if !s.chars().any(|c| ('\u{C0}'..='\u{FF}').contains(&c)) {
        return s.to_string();
    }
    const HIGH: [(char, u8); 27] = [
        ('€', 0x80), ('‚', 0x82), ('ƒ', 0x83), ('„', 0x84), ('…', 0x85), ('†', 0x86), ('‡', 0x87), ('ˆ', 0x88), ('‰', 0x89), ('Š', 0x8A), ('‹', 0x8B), ('Œ', 0x8C), ('Ž', 0x8E),
        ('‘', 0x91), ('’', 0x92), ('“', 0x93), ('”', 0x94), ('•', 0x95), ('–', 0x96), ('—', 0x97), ('˜', 0x98), ('™', 0x99), ('š', 0x9A), ('›', 0x9B), ('œ', 0x9C), ('ž', 0x9E), ('Ÿ', 0x9F),
    ];
    let mut bytes = Vec::with_capacity(s.len());
    for c in s.chars() {
        let u = c as u32;
        if u < 0x100 {
            bytes.push(u as u8);
        } else if let Some((_, b)) = HIGH.iter().find(|(h, _)| *h == c) {
            bytes.push(*b);
        } else {
            return s.to_string();
        }
    }
    match String::from_utf8(bytes.clone()) {
        Ok(t) => t,
        // not UTF-8 either: Arabic in Windows-1256 read as Western ("ÇáÓáÇã"):
        // mostly letters from the top half of the code page
        Err(_) => {
            let high = bytes.iter().filter(|b| **b >= 0xC0).count();
            let letters = bytes.iter().filter(|b| b.is_ascii_alphabetic() || **b >= 0xC0).count();
            if letters >= 4 && high * 10 >= letters * 8 {
                let (t, _, bad) = encoding_rs::WINDOWS_1256.decode(&bytes);
                if !bad {
                    return t.into_owned();
                }
            }
            s.to_string()
        }
    }
}

/// Episodes in watching order (season 0 extras last).
pub fn ordered(full: &Value) -> Vec<Value> {
    let mut v = full["videos"].as_array().cloned().unwrap_or_default();
    v.sort_by_key(|x| {
        let s = x["season"].as_i64().filter(|s| *s > 0).unwrap_or(9999);
        let e = x["episode"].as_i64().or(x["number"].as_i64()).unwrap_or(0);
        (s, e)
    });
    v
}

pub fn episode_name(v: &Value) -> String {
    v["title"].as_str().or(v["name"].as_str()).map(|s| s.to_string()).unwrap_or_else(|| format!("Episode {}", v["episode"].as_i64().or(v["number"].as_i64()).unwrap_or(0)))
}

fn runtime_secs(rt: &Value) -> f64 {
    if let Some(n) = rt.as_f64() {
        return n * 60.;
    }
    let s = rt.as_str().unwrap_or("").to_lowercase();
    let num = |t: &str| t.trim().parse::<f64>().ok();
    if let Some(i) = s.find('h') {
        let h = num(&s[..i]).unwrap_or(0.);
        let m = s[i + 1..].trim().trim_end_matches("min").trim_end_matches('m');
        return (h * 60. + num(m).unwrap_or(0.)) * 60.;
    }
    num(s.trim_end_matches("min")).map(|m| m * 60.).unwrap_or(0.)
}

/// Megabits per second this stream needs to play without stalling.
fn need_mbps(s: &Value, rt: &Value) -> f64 {
    let p = &s["_couch"];
    let secs = match runtime_secs(rt) {
        x if x > 0. => x,
        _ => 7200.,
    };
    if let Some(b) = p["sizeBytes"].as_f64() {
        return b * 8. / secs / 1e6;
    }
    match p["res"].as_str().unwrap_or("Unknown") {
        "4K" => 40.,
        "1080p" => 12.,
        "720p" => 5.,
        "SD" => 2.,
        _ => 8.,
    }
}

/// An audio track that isn't the film's own sound: a commentary, or audio
/// description for the blind (Some(what to call it)).
pub fn track_extra(t: &Value) -> Option<&'static str> {
    let title = t["title"].as_str().unwrap_or("").to_lowercase();
    if t["commentary"] == true || title.contains("commentary") || title.contains("comment") || title.contains("director's") || title.contains("directors") || title.contains("cast and crew") || title.contains("cast & crew") {
        return Some("commentary");
    }
    if t["visualImpaired"] == true || title.contains("audio description") || title.contains("descriptive") || title.contains("described") || title.contains(" ad ") || title.ends_with(" ad") {
        return Some("audio description");
    }
    if title.contains("isolated score") || title.contains("music only") {
        return Some("music only");
    }
    None
}

/// A bonus file, not the film: a featurette, interview, trailer, sample...
/// (packs often carry these, and a small one can look like the "best fit").
pub fn is_extra(s: &Value) -> bool {
    let t = [&s["name"], &s["title"], &s["description"]]
        .iter()
        .filter_map(|v| v.as_str())
        .chain(s.pointer("/behaviorHints/filename").and_then(|v| v.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    const WORDS: [&str; 16] = [
        "featurette", "behind the scenes", "behind.the.scenes", "behind-the-scenes", "interview", "trailer", "teaser", "sample",
        "making of", "making.of", "deleted scene", "deleted.scene", "bonus", "extras", "b-roll", "press conference",
    ];
    WORDS.iter().any(|w| t.contains(w))
}

/// Playable-now first, then what fits the connection (best first), the
/// rest smallest first; cams only ever last, bonus files after them.
pub fn rank(streams: Vec<Value>, mbps: f64, rt: &Value) -> Vec<Value> {
    let ok: Vec<Value> = streams
        .into_iter()
        .filter(|s| {
            let p = &s["_couch"];
            let k = p["kind"].as_str().unwrap_or("");
            k != "external" && !(p["threeD"].as_bool() == Some(true) && p["threeDLayout"].is_null())
        })
        .collect();
    let budget = mbps * 0.7;
    let instant = |s: &Value| {
        let p = &s["_couch"];
        p["cached"].as_bool() != Some(false) && matches!(p["kind"].as_str(), Some("debrid") | Some("http"))
    };
    let cam = |s: &Value| s["_couch"]["cam"].as_bool() == Some(true);
    let num = |s: &Value, k: &str| s["_couch"][k].as_f64().unwrap_or(0.);
    let (extras, ok): (Vec<Value>, Vec<Value>) = ok.into_iter().partition(is_extra);
    let (cams, real): (Vec<Value>, Vec<Value>) = ok.into_iter().partition(|s| cam(s));
    let (mut fits, mut rest): (Vec<Value>, Vec<Value>) =
        real.into_iter().partition(|s| need_mbps(s, rt) <= budget && s["_couch"]["threeD"].as_bool() != Some(true));
    fits.sort_by(|a, b| {
        (instant(b) as i32 - instant(a) as i32)
            .cmp(&0)
            .then((!b["_couch"]["source"].is_null() as i32).cmp(&(!a["_couch"]["source"].is_null() as i32)))
            .then(num(b, "resRank").partial_cmp(&num(a, "resRank")).unwrap_or(std::cmp::Ordering::Equal))
            .then(num(b, "score").partial_cmp(&num(a, "score")).unwrap_or(std::cmp::Ordering::Equal))
    });
    rest.sort_by(|a, b| {
        (instant(b) as i32).cmp(&(instant(a) as i32)).then(need_mbps(a, rt).partial_cmp(&need_mbps(b, rt)).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut cams = cams;
    cams.sort_by(|a, b| num(b, "score").partial_cmp(&num(a, "score")).unwrap_or(std::cmp::Ordering::Equal));
    fits.into_iter().chain(rest).chain(cams).chain(extras).collect()
}

/// Starting on a lower quality than what was found, because the connection
/// measured slow right now: say so (and where the better ones are).
fn why_lower(order: &[Value], first: &Value, mbps: f64, rt: &Value) -> Option<String> {
    let r = |s: &Value| s["_couch"]["resRank"].as_f64().unwrap_or(0.);
    let best = order.iter().filter(|s| !is_extra(s) && s["_couch"]["cam"].as_bool() != Some(true)).max_by(|a, b| r(a).partial_cmp(&r(b)).unwrap_or(std::cmp::Ordering::Equal))?;
    if r(best) <= r(first) || r(first) == 0. || need_mbps(best, rt) <= mbps * 0.7 {
        return None;
    }
    let res = |s: &Value| s["_couch"]["res"].as_str().unwrap_or("?").to_string();
    Some(format!(
        "playing {} because your connection measured {:.0} Mbps just now ({} needs about {:.0}). something else using the internet? pick {} from streams",
        res(first),
        mbps,
        res(best),
        need_mbps(best, rt),
        res(best)
    ))
}

/// Every addon's streams for one video, gathered for up to `ms`.
/// what the sources answered in the last search (shown when nothing was found)
static ASKED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
pub fn asked() -> String {
    let a = ASKED.lock().unwrap();
    if a.is_empty() { "no source answered in time".into() } else { a.join(" · ") }
}

pub async fn gather(kind: &str, id: &str, ms: u64) -> Vec<Value> {
    ASKED.lock().unwrap().clear();
    let req = format!("g{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
    let mut rx = host::listen("streams");
    let _ = api::call::<Value>("streams_fetch", json!({ "kind": kind, "id": id, "reqId": req })).await;
    let mut got = vec![];
    let deadline = crate::timer::after(Duration::from_millis(ms));
    futures::pin_mut!(deadline);
    loop {
        let next = rx.next();
        futures::pin_mut!(next);
        match futures::future::select(next, &mut deadline).await {
            futures::future::Either::Left((Some(c), _)) => {
                if c["reqId"].as_str() != Some(req.as_str()) {
                    continue;
                }
                if c["finished"].as_bool() == Some(true) {
                    break;
                }
                // what each source said, for the "couldn't find a stream" message
                if let Some(name) = c["addonName"].as_str().filter(|n| !n.is_empty()) {
                    let said = match (c["error"].as_str(), c["streams"].as_array().map_or(0, |a| a.len())) {
                        (Some(e), _) => format!("{name}: {}", e.chars().take(70).collect::<String>()),
                        (None, n) => format!("{name}: {n}"),
                    };
                    ASKED.lock().unwrap().push(said);
                }
                got.extend(c["streams"].as_array().cloned().unwrap_or_default());
            }
            _ => break,
        }
    }
    got
}

impl Playback {
    /// 0 = off, -1 = end of this episode, n = in n minutes
    pub fn set_sleep(&mut self, v: i64, cx: &mut Context<Self>) {
        self.sleep = match v {
            0 => None,
            -1 => Some(Sleep::Episode),
            n => Some(Sleep::At(std::time::Instant::now() + Duration::from_secs(n as u64 * 60), n)),
        };
        if let Some(Sleep::At(at, _)) = self.sleep {
            cx.spawn(async move |this, cx| {
                crate::timer::at(at).await;
                let _ = this.update(cx, |p, cx| {
                    if matches!(p.sleep, Some(Sleep::At(a, _)) if a == at) && p.open {
                        p.sleep = None;
                        p.cmd("pause", json!({ "value": true }));
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    /// Inside an intro / recap / credits right now? (kind, where it ends)
    pub fn segment_at(&self, pos: f64) -> Option<(String, f64)> {
        let o = self.segments.as_object()?;
        for (kind, list) in o {
            for seg in list.as_array().into_iter().flatten() {
                let a = seg["start"].as_f64().unwrap_or(0.);
                let b = seg["end"].as_f64().unwrap_or(self.state.duration);
                if pos >= a && pos < b - 1. {
                    return Some((kind.clone(), b));
                }
            }
        }
        None
    }

    /// The episode after this one (not specials after a finale, not unaired).
    pub fn next_video(&self) -> Option<Value> {
        let f = self.full.as_ref()?;
        if f["type"] != "series" {
            return None;
        }
        let list = ordered(f);
        let i = list.iter().position(|v| v["id"].as_str() == Some(self.video_id.as_str()))?;
        let n = list.get(i + 1)?;
        if n["season"].as_i64().unwrap_or(0) == 0 && list[i]["season"].as_i64().unwrap_or(0) != 0 {
            return None;
        }
        let today = couch::integrations::tmdb::today_str();
        // not out yet, or no date at all (announced, not aired): no next
        match n["released"].as_str().or(n["firstAired"].as_str()).filter(|r| !r.is_empty()) {
            Some(r) if &r[..r.len().min(10)] <= today.as_str() => Some(n.clone()),
            _ => None,
        }
    }

    pub fn play_next(&mut self, cx: &mut Context<Self>) {
        let (Some(n), Some(m)) = (self.next_video(), self.meta.clone()) else { return };
        let id = n["id"].as_str().unwrap_or("").to_string();
        cx.defer(move |cx| quick_play(m, Some(id), None, cx));
    }

    /// Pick an audio / subtitle track (None: subtitles off). The tick moves
    /// right away; mpv doesn't always report the change back.
    /// Put back the audio and subtitle languages this show (or film) was last
    /// watched in. Runs as tracks arrive (addon subtitles come a few seconds
    /// after the film), for up to 20 s after it opens.
    fn apply_langs(&mut self, _cx: &mut Context<Self>) {
        let Some(since) = self.langs_pending else { return };
        if since.elapsed().as_secs() > 20 {
            self.langs_pending = None;
            return;
        }
        // started on a commentary (or audio description) track: the film's own
        // sound in the same language instead
        {
            let lang = |t: &Value| t["lang"].as_str().unwrap_or("").to_lowercase();
            let list: Vec<&Value> = self.tracks.iter().filter(|t| t["type"] == "audio").collect();
            if let Some(on) = list.iter().find(|t| t["selected"] == true).filter(|t| track_extra(t).is_some()) {
                let l = lang(on);
                let better = list.iter().filter(|t| track_extra(t).is_none()).max_by_key(|t| (lang(t) == l) as u8 * 2 + (t["default"] == true) as u8).and_then(|t| t["id"].as_i64());
                if let Some(id) = better {
                    self.cmd("aid", json!({ "value": id }));
                }
            }
        }
        let Some(key) = self.meta.as_ref().map(|m| m.id.clone()) else { return };
        let all = couch::state::get().core.db.get::<serde_json::Map<String, Value>>(LANGS_KEY).unwrap_or_default();
        let Some(want) = all.get(&key).cloned() else {
            self.langs_pending = None;
            return;
        };
        let mut done = true;
        let lang = |t: &Value| t["lang"].as_str().unwrap_or("").to_lowercase();
        // audio
        if let Some(a) = want["audio"].as_str().filter(|a| !a.is_empty()) {
            let list: Vec<&Value> = self.tracks.iter().filter(|t| t["type"] == "audio").collect();
            let on = list.iter().find(|t| t["selected"] == true).map(|t| lang(t));
            if on.as_deref() != Some(a) {
                let title = want["audioTitle"].as_str();
                let pick = list.iter().filter(|t| lang(t) == a).max_by_key(|t| (t["title"].as_str() == title) as u8).and_then(|t| t["id"].as_i64());
                if let Some(id) = pick {
                    self.cmd("aid", json!({ "value": id }));
                }
            }
        }
        // subtitles: off, or a language (built-in or not, as it was)
        match want["sub"].as_str() {
            Some("off") => {
                if self.tracks.iter().any(|t| t["type"] == "sub" && t["selected"] == true) {
                    self.subs_off = true;
                    self.sub.clear();
                    self.cmd("sid", json!({ "value": "no" }));
                }
            }
            Some(sl) if !sl.is_empty() => {
                let list: Vec<&Value> = self.tracks.iter().filter(|t| t["type"] == "sub").collect();
                let on = list.iter().find(|t| t["selected"] == true);
                if on.map(|t| lang(t)).as_deref() != Some(sl) {
                    let ext = want["subExternal"].as_bool();
                    let pick = list
                        .iter()
                        .filter(|t| lang(t) == sl && t["forced"] != true)
                        .max_by_key(|t| (Some(t["external"].as_bool().unwrap_or(false)) == ext) as u8)
                        .and_then(|t| t["id"].as_i64());
                    match pick {
                        Some(id) => {
                            self.subs_off = false;
                            self.cmd("sid", json!({ "value": id }));
                        }
                        // not here yet (addon subtitles come a little later)
                        None => done = false,
                    }
                }
            }
            _ => {}
        }
        if done {
            self.langs_pending = None;
        }
    }

    /// a part found by ear (when the episode's still the one playing)
    fn add_segment(&mut self, kind: &str, vid: &str, seg: Value, cx: &mut Context<Self>) {
        if self.video_id != vid {
            return;
        }
        if !self.segments.is_object() {
            self.segments = json!({});
        }
        let empty = self.segments[kind].as_array().map_or(true, |a| a.is_empty());
        if empty {
            self.segments[kind] = json!([seg]);
            cx.notify();
        }
    }

    pub fn pick_track(&mut self, kind: &str, id: Option<i64>, cx: &mut Context<Self>) {
        // remembered for this show: its next episodes start the same way
        if let Some(key) = self.meta.as_ref().map(|m| m.id.clone()) {
            let db = &couch::state::get().core.db;
            let mut all = db.get::<serde_json::Map<String, Value>>(LANGS_KEY).unwrap_or_default();
            let mut e = all.get(&key).cloned().unwrap_or_else(|| json!({}));
            let t = id.and_then(|i| self.tracks.iter().find(|t| t["type"] == kind && t["id"].as_i64() == Some(i)).cloned());
            if kind == "audio" {
                if let Some(t) = &t {
                    e["audio"] = json!(t["lang"].as_str().unwrap_or("").to_lowercase());
                    e["audioTitle"] = t["title"].clone();
                }
            } else {
                match &t {
                    Some(t) => {
                        e["sub"] = json!(t["lang"].as_str().unwrap_or("").to_lowercase());
                        e["subExternal"] = t["external"].clone();
                    }
                    None => e["sub"] = json!("off"),
                }
            }
            all.insert(key, e);
            let _ = db.set(LANGS_KEY, &all);
            // you chose: nothing is put back over it now
            self.langs_pending = None;
        }
        for t in self.tracks.iter_mut().filter(|t| t["type"] == kind) {
            t["selected"] = json!(id.is_some() && t["id"].as_i64() == id);
        }
        if kind != "audio" {
            self.subs_off = id.is_none();
            if self.subs_off {
                self.sub.clear();
            }
        }
        let cmd = if kind == "audio" { "aid" } else { "sid" };
        self.cmd(cmd, match id {
            Some(i) => json!({ "value": i }),
            None => json!({ "value": "no" }),
        });
        // another audio track may have other channels: headphone surround follows
        if kind == "audio" && crate::views::player::headphones_on() {
            couch::rt::spawn(async {
                tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                let _ = couch::dispatch::invoke("player_cmd", json!({ "cmd": "headphones", "args": { "value": true } })).await;
            });
        }
        cx.notify();
    }

    pub fn cmd(&self, name: &str, args: Value) {
        // volume: the newest value only, sent one after another. Each move of
        // the slider used to fire its own request; they raced and arrived out
        // of order, so the volume jumped back and forth
        if name == "volume" {
            static WANT: parking_lot::Mutex<(Option<f64>, bool)> = parking_lot::Mutex::new((None, false));
            let mut w = WANT.lock();
            w.0 = args["value"].as_f64();
            if w.1 {
                return;
            }
            w.1 = true;
            drop(w);
            couch::rt::spawn(async {
                loop {
                    let v = {
                        let mut w = WANT.lock();
                        match w.0.take() {
                            Some(v) => v,
                            None => {
                                w.1 = false;
                                break;
                            }
                        }
                    };
                    let _ = couch::dispatch::invoke("player_cmd", json!({ "cmd": "volume", "args": { "value": v } })).await;
                }
            });
            return;
        }
        let name = name.to_string();
        if name == "seek" {
            self.last_seek.set(Some(std::time::Instant::now()));
        }
        couch::rt::spawn(async move {
            let _ = couch::dispatch::invoke("player_cmd", json!({ "cmd": name, "args": args })).await;
        });
    }

    fn try_next(&mut self, cx: &mut Context<Self>) -> bool {
        if self.queue.is_empty() {
            return false;
        }
        let next = self.queue.remove(0);
        self.status = "that one wouldn't start, trying another…".into();
        self.start(next, None, cx);
        true
    }

    /// On someone's couch, this file isn't the one the host plays (its length
    /// is way off: a different film by the same name, a cut, a sample): the next.
    pub fn not_this_file(&mut self, at: Option<f64>, cx: &mut Context<Self>) -> bool {
        if self.queue.is_empty() {
            return false;
        }
        let next = self.queue.remove(0);
        self.status = "that file didn't match the host's, trying another…".into();
        self.start(next, at, cx);
        true
    }

    /// Switch to this stream (the sources menu), from where you are. The pick
    /// is remembered for this episode/film, and goes first next time.
    pub fn pick(&mut self, stream: Value, cx: &mut Context<Self>) {
        let key = stream_key(&stream);
        let mut picks = couch::state::get().core.db.get::<serde_json::Map<String, Value>>(PICKS_KEY).unwrap_or_default();
        picks.insert(self.video_id.clone(), json!(key));
        if picks.len() > 400 {
            if let Some(k) = picks.keys().next().cloned() {
                picks.remove(&k);
            }
        }
        let _ = couch::state::get().core.db.set(PICKS_KEY, &picks);
        let at = (self.state.pos > 5.).then_some(self.state.pos);
        self.queue = self.all.iter().filter(|s| stream_key(s) != key).cloned().collect();
        self.status = "switching streams…".into();
        self.start(stream, at, cx);
    }

    /// Switch to this stream without remembering it (following the host).
    pub fn use_stream(&mut self, stream: Value, at: Option<f64>, cx: &mut Context<Self>) {
        let key = stream_key(&stream);
        self.queue = self.all.iter().filter(|s| stream_key(s) != key).cloned().collect();
        self.status = "switching to the host's stream…".into();
        self.start(stream, at, cx);
    }

    /// The host's stream among the ones found here (or theirs as it is).
    pub fn host_choice(&self, tag: &Value) -> Option<Value> {
        self.all.iter().find(|s| same_as_host(s, tag)).cloned().or_else(|| host_stream(tag))
    }

    /// Is this the stream the host is on?
    pub fn on_host_stream(&self, tag: &Value) -> bool {
        self.stream.as_ref().map_or(true, |s| same_as_host(s, tag))
    }

    /// The stream keeps stalling (not just a seek or a pause): count it bad
    /// and move on to the next one, from the same moment.
    fn watch_lag(&mut self, s: &State, cx: &mut Context<Self>) {
        let now = std::time::Instant::now();
        let seeking = self.last_seek.get().map_or(false, |t| t.elapsed().as_secs_f32() < 3.);
        if self.lag.playing_since.is_none() {
            if !s.buffering && !s.paused && s.pos > 0.5 && !self.loading {
                self.lag.playing_since = Some(now);
            }
            return;
        }
        if s.paused || seeking || self.queue.is_empty() {
            self.lag.stall_at = None;
            return;
        }
        if s.buffering && !self.state.buffering {
            self.lag.stall_at = Some(now);
            self.lag.stalls += 1;
        }
        if !s.buffering {
            self.lag.stall_at = None;
        }
        let young = self.lag.playing_since.map_or(false, |t| t.elapsed().as_secs() < 180);
        let long = self.lag.stall_at.map_or(false, |t| t.elapsed().as_secs_f32() > 8.);
        if long || (young && self.lag.stalls >= 4) {
            if let Some(cur) = self.stream.clone() {
                mark_bad(&cur);
            }
            let at = (s.pos > 5.).then_some(s.pos);
            let next = self.queue.remove(0);
            self.status = "that stream kept stalling, trying another…".into();
            self.start(next, at, cx);
            cx.defer(|cx| crate::Nav::toast("that stream kept lagging, so couch switched to another".into(), cx));
        }
    }

    fn start(&mut self, stream: Value, start_at: Option<f64>, cx: &mut Context<Self>) {
        self.lag = Lag::default();
        self.loading = true;
        self.fresh = false;
        self.error.clear();
        self.stream = Some(stream.clone());
        self.state = State { volume: self.state.volume, speed: 1., ..Default::default() };
        let meta = self.full.clone().unwrap_or_else(|| serde_json::to_value(&self.meta).unwrap_or_default());
        let preview = json!({
            "id": meta["id"], "type": meta["type"], "name": meta["name"], "poster": meta["poster"],
            "background": meta["background"], "logo": meta["logo"], "runtime": meta["runtime"],
        });
        let req = json!({ "stream": stream, "meta": preview, "videoId": self.video_id, "startAt": start_at, "episodeTitle": (!self.episode_title.is_empty()).then(|| self.episode_title.clone()) });
        let g = self.load_gen;
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("player_open", json!({ "req": req })).await;
            let _ = this.update(cx, |p, cx| {
                // moved on while it opened: this one mustn't play
                if p.load_gen != g {
                    if !p.open {
                        couch::rt::spawn(async {
                            let _ = couch::dispatch::invoke("player_stop", json!({})).await;
                        });
                    }
                    return;
                }
                match r {
                    Ok(v) if v["mode"] == "external" => {
                        if let Some(u) = v["url"].as_str() {
                            cx.open_url(u);
                        }
                        p.close(cx);
                    }
                    Ok(v) if v["mode"] == "youtube" || v["mode"] == "web" => {
                        p.loading = false;
                        p.error = "this stream needs the old player. pick another one.".into();
                    }
                    Ok(_) => {
                        // night mode stays on from film to film (and from last time)
                        if crate::views::player::night_on() {
                            p.cmd("night", json!({ "value": true }));
                        }
                    }
                    Err(e) => {
                        if !p.try_next(cx) {
                            p.loading = false;
                            p.error = e;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.load_gen += 1;
        // left the player: the next episode found in the background goes too
        *PRELOAD.lock() = None;
        self.preload_for = None;
        self.open = false;
        self.loading = false;
        self.error.clear();
        self.queue.clear();
        self.sub.clear();
        couch::rt::spawn(async {
            let _ = couch::dispatch::invoke("player_stop", json!({})).await;
        });
        cx.notify();
    }
}

/// Play straight away: couch finds the streams, ranks them and starts the
/// best. Shows pick up at the episode you're on.
pub fn quick_play(base: Meta, video_id: Option<String>, start_at: Option<f64>, cx: &mut App) {
    quick_play_with(base, video_id, start_at, None, cx)
}

/// `prefer`: a stream to start with if it's among the ones found (on someone's
/// couch: the host's own stream, so you're both on the same file).
pub fn quick_play_with(base: Meta, video_id: Option<String>, start_at: Option<f64>, prefer: Option<Value>, cx: &mut App) {
    let pb = get(cx);
    // already playing (the corner picture, the page behind it): back to it,
    // not a fresh search that drops the film you're watching
    {
        let p = pb.read(cx);
        let same_title = p.meta.as_ref().map_or(false, |m| m.id == base.id);
        let same_video = video_id.as_ref().map_or(true, |v| *v == p.video_id);
        if p.open && !p.loading && p.error.is_empty() && same_title && same_video && p.state.duration > 0. && !p.state.ended && prefer.is_none() && start_at.is_none() {
            crate::Nav::go(crate::Route::Player, cx);
            return;
        }
    }
    let g = pb.update(cx, |p, cx| {
        p.load_gen += 1;
        p.open = true;
        p.loading = true;
        p.fresh = false;
        p.stream = None;
        p.start_hint = start_at;
        p.preload_for = None;
        p.no_autonext = false;
        p.langs_pending = Some(std::time::Instant::now());
        // the episode is known now: watch together tells guests at once
        p.video_id = video_id.clone().unwrap_or_default();
        p.state = State { volume: p.state.volume, muted: p.state.muted, speed: 1., ..Default::default() };
        p.error.clear();
        p.status = "finding the best stream for your connection…".into();
        p.meta = Some(base.clone());
        p.full = None;
        p.tracks.clear();
        p.subs_off = false;
        p.sub.clear();
        cx.notify();
        p.load_gen
    });
    crate::ui::video::stop_all(cx);
    crate::Nav::go(crate::Route::Player, cx);
    let kind = base.kind.clone();
    pb.update(cx, |_, cx| {
        cx.spawn(async move |this, cx| {
            let full = api::call::<Value>("meta_get", json!({ "kind": kind, "id": base.id, "hint": Value::Null })).await.unwrap_or_else(|_| serde_json::to_value(&base).unwrap_or_default());
            let mut vid = full["id"].as_str().unwrap_or(&base.id).to_string();
            let mut ep_title = String::new();
            let mut rt = full["runtime"].clone();
            let eps: Vec<Value> = ordered(&full).into_iter().filter(|v| v["season"].as_i64().unwrap_or(0) > 0).collect();
            if let Some(want) = video_id.clone().filter(|v| v != &vid) {
                if let Some(v) = eps.iter().find(|x| x["id"].as_str() == Some(want.as_str())) {
                    ep_title = episode_name(v);
                    rt = v.get("runtime").cloned().unwrap_or(rt);
                }
                vid = want;
            } else if full["type"] == "series" {
                let item = api::call::<Value>("library_item", json!({ "id": vid })).await.unwrap_or(Value::Null);
                let at = item["state"]["video_id"].as_str().map(|s| s.to_string());
                if let Some(v) = at.as_ref().and_then(|a| eps.iter().find(|x| x["id"].as_str() == Some(a.as_str()))).or(eps.first()) {
                    vid = v["id"].as_str().unwrap_or("").to_string();
                    ep_title = episode_name(v);
                    rt = v.get("runtime").cloned().unwrap_or(rt);
                }
            }
            // saved on this computer: that file (no internet needed); else the
            // next episode found while you watched the last one; else a search
            let saved = api::call::<Option<String>>("download_local", json!({ "videoId": vid })).await.ok().flatten();
            let order = match (saved, take_preload(&vid)) {
                (Some(path), _) if prefer.is_none() => vec![json!({ "url": path, "name": "on this computer", "title": "downloaded", "_addonName": "couch" })],
                (_, Some(o)) if prefer.is_none() => o,
                _ => ranked(full["type"].as_str().unwrap_or("movie").to_string(), vid.clone(), rt.clone(), prefer.clone()).await,
            };
            let mut order = order;
            let note = match order.first() {
                Some(first) if prefer.is_none() => {
                    let mbps = api::call::<f64>("net_speed", json!({ "fresh": false })).await.unwrap_or(25.);
                    why_lower(&order, first, mbps, &rt)
                }
                _ => None,
            };
            let _ = this.update(cx, |p, cx| {
                if !p.open || p.load_gen != g {
                    return;
                }
                if let Some(n) = note {
                    crate::Nav::toast(n, cx);
                }
                p.full = Some(full);
                p.video_id = vid;
                p.episode_title = ep_title;
                if order.is_empty() {
                    p.loading = false;
                    p.error = format!("couldn't find a stream for that one ({})", asked());
                    cx.notify();
                    return;
                }
                p.all = order.clone();
                let first = order.remove(0);
                p.queue = order;
                p.status = "starting…".into();
                p.start(first, start_at, cx);
            });
        })
        .detach();
    });
}


/// Save a film (or one episode) to this computer: the best stream that can
/// be saved (an instant / web one; torrents play from their swarm).
pub fn download(base: Meta, video_id: Option<String>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let full = api::call::<Value>("meta_get", json!({ "kind": base.kind, "id": base.id, "hint": Value::Null })).await.unwrap_or_else(|_| serde_json::to_value(&base).unwrap_or_default());
        let vid = video_id.clone().unwrap_or_else(|| full["id"].as_str().unwrap_or(&base.id).to_string());
        let ep = ordered(&full).into_iter().find(|v| v["id"].as_str() == Some(vid.as_str()));
        let label = match &ep {
            Some(v) => format!("{} S{:02}E{:02} {}", base.name, v["season"].as_i64().unwrap_or(0), v["episode"].as_i64().or(v["number"].as_i64()).unwrap_or(0), episode_name(v)),
            None => format!("{} ({})", base.name, full["year"].as_str().map(String::from).or(full["releaseInfo"].as_str().map(String::from)).unwrap_or_default()),
        };
        let rt = ep.as_ref().and_then(|v| v.get("runtime").cloned()).unwrap_or(full["runtime"].clone());
        let order = ranked(full["type"].as_str().unwrap_or("movie").to_string(), vid.clone(), rt, None).await;
        let pick = order.into_iter().find(|s| s["url"].as_str().is_some_and(|u| u.starts_with("http")));
        let msg = match pick {
            None => "nothing saveable found for that one (only instant / web streams can be saved)".to_string(),
            Some(stream) => {
                let meta = json!({ "id": base.id, "type": base.kind, "name": base.name, "poster": base.poster, "background": base.background, "logo": base.logo });
                match api::call::<Value>("download_start", json!({ "stream": stream, "meta": meta, "videoId": vid, "label": label })).await {
                    Ok(_) => format!("saving {label} to watch offline (My Lists → Downloads)"),
                    Err(e) => e,
                }
            }
        };
        let _ = cx.update(|cx| crate::Nav::toast(msg, cx));
    })
    .detach();
}

/// Every stream for this film / episode, best first (your pick, or the
/// host's, at the front; ones that lagged before at the back).
async fn ranked(kind: String, vid: String, rt: Value, prefer: Option<Value>) -> Vec<Value> {
            let settings = api::call::<Value>("settings_get", json!({})).await.unwrap_or(Value::Null);
            let s = &settings["streams"];
            let (streams, mbps) = futures::join!(gather(&kind, &vid, 9000), async {
                api::call::<f64>("net_speed", json!({ "fresh": false })).await.unwrap_or(25.)
            });
            let max_gb = s["maxSizeGb"].as_f64().unwrap_or(0.);
            let ok: Vec<Value> = streams
                .into_iter()
                .filter(|x| {
                    let p = &x["_couch"];
                    if s["hideCam"].as_bool() == Some(true) && p["cam"].as_bool() == Some(true) {
                        return false;
                    }
                    if s["hide3d"].as_bool() == Some(true) && p["threeD"].as_bool() == Some(true) {
                        return false;
                    }
                    if max_gb > 0. && p["sizeBytes"].as_f64().map_or(false, |b| b > max_gb * 1e9) {
                        return false;
                    }
                    true
                })
                .collect();
            let mut order = rank(ok, mbps, &rt);
            // "prefer HDR" off (an SDR screen): HDR / Dolby Vision files only if nothing else
            if s["preferHdr"].as_bool() == Some(false) {
                order.sort_by_key(|x| x["_couch"]["hdr"].as_array().map_or(false, |h| !h.is_empty()));
            }
            // ones that lagged before go last; your own pick (or the host's) first
            let bad = couch::state::get().core.db.get::<serde_json::Map<String, Value>>(BAD_KEY).unwrap_or_default();
            let (good, lagged): (Vec<Value>, Vec<Value>) = order.into_iter().partition(|s| !bad.contains_key(&stream_key(s)));
            order = good.into_iter().chain(lagged).collect();
            let mine = couch::state::get().core.db.get::<serde_json::Map<String, Value>>(PICKS_KEY).unwrap_or_default().get(&vid).and_then(|v| v.as_str().map(String::from));
            if let Some(h) = prefer.as_ref() {
                match order.iter().position(|s| same_as_host(s, h)) {
                    Some(i) => {
                        let s = order.remove(i);
                        order.insert(0, s);
                    }
                    // not among mine: the host's stream itself, as they play it
                    None => {
                        if let Some(s) = host_stream(h) {
                            order.insert(0, s);
                        }
                    }
                }
            } else if let Some(k) = mine {
                if let Some(i) = order.iter().position(|s| stream_key(s) == k) {
                    let s = order.remove(i);
                    order.insert(0, s);
                }
            }
    order
}

/// On a title's page, before Play: its streams are looked for already, so
/// Play starts at once, and the page can say when there's nothing.
/// (title id | video id) → None while looking, Some(found any)
static WARM: parking_lot::Mutex<Option<std::collections::HashMap<String, Option<bool>>>> = parking_lot::Mutex::new(None);

pub fn warm_state(id: &str, video: Option<&str>) -> Option<bool> {
    WARM.lock().as_ref().and_then(|m| m.get(&format!("{id}|{}", video.unwrap_or(""))).copied().flatten())
}

pub fn warm(base: Meta, video_id: Option<String>, cx: &mut App) {
    let key = format!("{}|{}", base.id, video_id.clone().unwrap_or_default());
    {
        let mut w = WARM.lock();
        let w = w.get_or_insert_with(Default::default);
        if w.contains_key(&key) {
            return;
        }
        w.insert(key.clone(), None);
    }
    // something's playing: its next episode's search isn't pushed out
    if get(cx).read(cx).open {
        WARM.lock().get_or_insert_with(Default::default).remove(&key);
        return;
    }
    cx.spawn(async move |cx| {
        let full = api::call::<Value>("meta_get", json!({ "kind": base.kind, "id": base.id, "hint": Value::Null })).await.unwrap_or_else(|_| serde_json::to_value(&base).unwrap_or_default());
        let mut vid = full["id"].as_str().unwrap_or(&base.id).to_string();
        let mut rt = full["runtime"].clone();
        let eps: Vec<Value> = ordered(&full).into_iter().filter(|v| v["season"].as_i64().unwrap_or(0) > 0).collect();
        if let Some(want) = video_id.clone().filter(|v| v != &vid) {
            if let Some(v) = eps.iter().find(|x| x["id"].as_str() == Some(want.as_str())) {
                rt = v.get("runtime").cloned().unwrap_or(rt);
            }
            vid = want;
        } else if full["type"] == "series" {
            let item = api::call::<Value>("library_item", json!({ "id": vid })).await.unwrap_or(Value::Null);
            let at = item["state"]["video_id"].as_str().map(|s| s.to_string());
            if let Some(v) = at.as_ref().and_then(|a| eps.iter().find(|x| x["id"].as_str() == Some(a.as_str()))).or(eps.first()) {
                vid = v["id"].as_str().unwrap_or("").to_string();
                rt = v.get("runtime").cloned().unwrap_or(rt);
            }
        }
        let order = ranked(full["type"].as_str().unwrap_or("movie").to_string(), vid.clone(), rt, None).await;
        let found = !order.is_empty();
        WARM.lock().get_or_insert_with(Default::default).insert(key, Some(found));
        if found {
            let playing = cx.update(|cx| get(cx).read(cx).open);
            if !playing {
                *PRELOAD.lock() = Some((vid, order, std::time::Instant::now()));
            }
        }
        let _ = cx.update(|cx| cx.refresh_windows());
    })
    .detach();
}

/// The next episode's streams, found in the background halfway through this
/// one: (video id, streams, when). Dropped when you close the player.
static PRELOAD: parking_lot::Mutex<Option<(String, Vec<Value>, std::time::Instant)>> = parking_lot::Mutex::new(None);

fn take_preload(vid: &str) -> Option<Vec<Value>> {
    let mut g = PRELOAD.lock();
    match g.take() {
        Some((v, o, at)) if v == vid && at.elapsed().as_secs() < 1800 && !o.is_empty() => Some(o),
        other => {
            *g = other;
            None
        }
    }
}

#[derive(Default)]
struct Lag {
    playing_since: Option<std::time::Instant>,
    stall_at: Option<std::time::Instant>,
    stalls: u32,
}

const PICKS_KEY: &str = "gpui_stream_picks";
/// per show / film: { audio, audioTitle, sub ("off" or a language), subExternal }
const LANGS_KEY: &str = "gpui_langs";
const BAD_KEY: &str = "gpui_bad_streams";

/// One stream, the same whichever addon listed it: its torrent (and file), or its address.
pub fn stream_key(s: &Value) -> String {
    match (s["infoHash"].as_str(), s["url"].as_str()) {
        (Some(h), _) => format!("bt:{}:{}", h.to_lowercase(), s["fileIdx"].as_i64().unwrap_or(-1)),
        (None, Some(u)) => format!("url:{u}"),
        _ => String::new(),
    }
}

/// A stream that kept stalling: tried last for a week.
fn mark_bad(s: &Value) {
    let k = stream_key(s);
    if k.is_empty() {
        return;
    }
    let db = &couch::state::get().core.db;
    let mut bad = db.get::<serde_json::Map<String, Value>>(BAD_KEY).unwrap_or_default();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    bad.retain(|_, t| t.as_u64().map_or(false, |t| now.saturating_sub(t) < 7 * 86400));
    bad.insert(k, json!(now));
    let _ = db.set(BAD_KEY, &bad);
}

/// What the host tells guests about their stream (small: it rides the live line).
pub fn host_tag(s: &Value) -> Value {
    let title: String = s["title"].as_str().or(s["name"].as_str()).unwrap_or("").chars().take(80).collect();
    match (s["infoHash"].as_str(), s["url"].as_str()) {
        (Some(h), _) => json!({ "h": h.to_lowercase(), "f": s["fileIdx"], "a": s["_addonName"], "t": title }),
        (None, Some(u)) if u.len() < 480 => json!({ "u": u, "a": s["_addonName"], "t": title }),
        _ => json!({ "a": s["_addonName"], "t": title }),
    }
}

fn same_as_host(s: &Value, h: &Value) -> bool {
    if let Some(hh) = h["h"].as_str() {
        let f = h["f"].as_i64();
        return s["infoHash"].as_str().map_or(false, |x| x.eq_ignore_ascii_case(hh)) && (f.is_none() || s["fileIdx"].as_i64() == f);
    }
    if let Some(u) = h["u"].as_str() {
        if s["url"].as_str() == Some(u) {
            return true;
        }
    }
    // the same release from the same addon (a debrid link differs per account)
    !h["t"].as_str().unwrap_or("").is_empty() && s["_addonName"] == h["a"] && s["title"].as_str().or(s["name"].as_str()).map(|t| t.chars().take(80).collect::<String>()) == h["t"].as_str().map(String::from)
}

fn host_stream(h: &Value) -> Option<Value> {
    if let Some(hh) = h["h"].as_str() {
        let mut s = json!({ "infoHash": hh, "title": h["t"], "name": h["a"] });
        if let Some(f) = h["f"].as_i64() {
            s["fileIdx"] = json!(f);
        }
        return Some(s);
    }
    h["u"].as_str().map(|u| json!({ "url": u, "title": h["t"], "name": h["a"] }))
}
