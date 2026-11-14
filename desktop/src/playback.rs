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
    pub aid: String,
    pub sid: String,
    pub ended: bool,
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
    pub sub: String,
    pub status: String,
    /// skip segments from TheIntroDB: { intro: [{start,end}], recap, credits }
    pub segments: Value,
    skipped: Vec<String>,
    pub sleep: Option<Sleep>,
}

pub struct PlaybackHandle(pub Entity<Playback>);
impl Global for PlaybackHandle {}

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
                };
                let _ = this.update(cx, |p, cx| {
                    if p.loading && !s.buffering && s.duration > 0. {
                        p.loading = false;
                    }
                    // auto-skip intros / recaps when asked to
                    let pos = s.pos;
                    if let Some((kind, end)) = p.segment_at(pos) {
                        let set = couch::state::get().core.settings();
                        let auto = (kind == "intro" && set.playback.auto_skip_intro) || (kind == "recap" && set.playback.auto_skip_recap);
                        let key = format!("{kind}{}", end as i64);
                        if auto && !p.skipped.contains(&key) {
                            p.skipped.push(key);
                            p.cmd("seek", json!({ "to": end }));
                        }
                    }
                    p.watch_lag(&s, cx);
                    // reached the end: the next episode by itself
                    let finished = s.ended && !p.state.ended;
                    p.state = s;
                    if finished && p.open {
                        if p.sleep == Some(Sleep::Episode) {
                            // sleep timer: that was the last one tonight
                            p.sleep = None;
                        } else if couch::state::get().core.settings().playback.autoplay_next && crate::together::get(cx).read(cx).following.is_none() {
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
                    cx.notify();
                });
            }
        })
        .detach();
        let mut sub = host::listen("player://sub");
        cx.spawn(async move |this, cx| {
            while let Some(v) = sub.next().await {
                let _ = this.update(cx, |p, cx| {
                    p.sub = clean_cue(v["text"].as_str().unwrap_or(""));
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
                        p.loading = false;
                        p.segments = Value::Null;
                        p.skipped.clear();
                        cx.notify();
                        p.video_id.clone()
                    })
                    .unwrap_or_default();
                if vid.is_empty() || !couch::state::get().core.settings().integrations.tidb_enabled {
                    continue;
                }
                let segs = api::call::<Value>("skip_segments", json!({ "videoId": vid, "durationMs": Value::Null })).await.unwrap_or(Value::Null);
                let _ = this.update(cx, |p, cx| {
                    if p.video_id == vid {
                        p.segments = segs;
                        cx.notify();
                    }
                });
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
    out.trim().to_string()
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

/// Playable-now first, then what fits the connection (best first), the
/// rest smallest first; cams only ever last.
pub fn rank(streams: Vec<Value>, mbps: f64, rt: &Value) -> Vec<Value> {
    let ok: Vec<Value> = streams
        .into_iter()
        .filter(|s| {
            let p = &s["_couch"];
            let k = p["kind"].as_str().unwrap_or("");
            k != "external" && k != "youtube" && !(p["threeD"].as_bool() == Some(true) && p["threeDLayout"].is_null())
        })
        .collect();
    let budget = mbps * 0.7;
    let instant = |s: &Value| {
        let p = &s["_couch"];
        p["cached"].as_bool() != Some(false) && matches!(p["kind"].as_str(), Some("debrid") | Some("http"))
    };
    let cam = |s: &Value| s["_couch"]["cam"].as_bool() == Some(true);
    let num = |s: &Value, k: &str| s["_couch"][k].as_f64().unwrap_or(0.);
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
    fits.into_iter().chain(rest).chain(cams).collect()
}

/// Every addon's streams for one video, gathered for up to `ms`.
pub async fn gather(kind: &str, id: &str, ms: u64) -> Vec<Value> {
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
        if n["released"].as_str().map_or(false, |r| &r[..r.len().min(10)] > today.as_str()) {
            return None;
        }
        Some(n.clone())
    }

    pub fn play_next(&mut self, cx: &mut Context<Self>) {
        let (Some(n), Some(m)) = (self.next_video(), self.meta.clone()) else { return };
        let id = n["id"].as_str().unwrap_or("").to_string();
        cx.defer(move |cx| quick_play(m, Some(id), None, cx));
    }

    pub fn cmd(&self, name: &str, args: Value) {
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
            cx.defer(|cx| crate::Nav::toast("that stream kept lagging — switched to another".into(), cx));
        }
    }

    fn start(&mut self, stream: Value, start_at: Option<f64>, cx: &mut Context<Self>) {
        self.lag = Lag::default();
        self.loading = true;
        self.error.clear();
        self.stream = Some(stream.clone());
        self.state = State { volume: self.state.volume, speed: 1., ..Default::default() };
        let meta = self.full.clone().unwrap_or_else(|| serde_json::to_value(&self.meta).unwrap_or_default());
        let preview = json!({
            "id": meta["id"], "type": meta["type"], "name": meta["name"], "poster": meta["poster"],
            "background": meta["background"], "logo": meta["logo"], "runtime": meta["runtime"],
        });
        let req = json!({ "stream": stream, "meta": preview, "videoId": self.video_id, "startAt": start_at, "episodeTitle": (!self.episode_title.is_empty()).then(|| self.episode_title.clone()) });
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("player_open", json!({ "req": req })).await;
            let _ = this.update(cx, |p, cx| {
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
                    Ok(_) => {}
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
    pb.update(cx, |p, cx| {
        p.open = true;
        p.loading = true;
        p.error.clear();
        p.status = "finding the best stream for your connection…".into();
        p.meta = Some(base.clone());
        p.full = None;
        p.tracks.clear();
        p.sub.clear();
        cx.notify();
    });
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
            let settings = api::call::<Value>("settings_get", json!({})).await.unwrap_or(Value::Null);
            let s = &settings["streams"];
            let (streams, mbps) = futures::join!(gather(full["type"].as_str().unwrap_or("movie"), &vid, 9000), async {
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
            let _ = this.update(cx, |p, cx| {
                if !p.open {
                    return;
                }
                p.full = Some(full);
                p.video_id = vid;
                p.episode_title = ep_title;
                if order.is_empty() {
                    p.loading = false;
                    p.error = "couldn't find a stream for that one".into();
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

#[derive(Default)]
struct Lag {
    playing_since: Option<std::time::Instant>,
    stall_at: Option<std::time::Instant>,
    stalls: u32,
}

const PICKS_KEY: &str = "gpui_stream_picks";
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
