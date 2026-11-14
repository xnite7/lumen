//! Watch together. Ask to join a friend's couch; they let you in (or not);
//! then your player follows theirs over the live line (a WebSocket through
//! the crew server): same title, same episode, same moment. Small drift is
//! eased out with ±8% speed; only a big gap seeks, aimed ahead by how long a
//! seek takes here, then waits for it to land. Same rules as the webview.
use crate::api::{self, Meta};
use crate::playback;
use crate::Nav;
use futures::{SinkExt, StreamExt};
use gpui::{App, AppContext, Context, Entity, Global};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Default)]
pub struct Together {
    /// people asking to join my couch
    pub inbox: Vec<Value>,
    /// who I've asked: (ask id, host id, host name)
    pub asking: Option<(String, String, String)>,
    /// whose couch I'm on: (host id, name)
    pub following: Option<(String, String)>,
    /// my id while friends are on my couch
    pub hosting: Option<String>,
    pub shared: bool,
    pub can_control: bool,
    line: Option<Line>,
    host_st: Option<HostState>,
    seek_at: Option<Instant>,
    seek_cost: f64,
    nudged: f64,
    stalled_since: Option<Instant>,
    switching: bool,
    steered_at: Option<Instant>,
    sent: Sent,
    /// something a guest tried that only the host can do: (icon, words, when)
    pub denied: Option<(&'static str, String, Instant)>,
    /// told the guest we're waiting on a silent host (once per silence)
    waiting_told: bool,
    /// the host stream this guest last switched to
    sk_applied: Value,
}

#[derive(Clone, Default)]
struct Sent {
    v: String,
    sk: String,
    pa: bool,
    st: bool,
    sh: bool,
    p: f64,
    at: Option<Instant>,
}

#[derive(Clone)]
struct HostState {
    v: String,
    /// the title the host is on (id, type, name)
    m: String,
    ty: String,
    ti: String,
    /// the host's stream (playback::host_tag), so guests play the same file
    sk: Value,
    p: f64,
    pa: bool,
    st: bool,
    got: Instant,
}

/// One WebSocket to the crew server, reopened with backoff while wanted.
struct Line {
    out: mpsc::UnboundedSender<String>,
    rtt: Arc<Mutex<f64>>,
    open: Arc<std::sync::atomic::AtomicBool>,
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for Line {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

fn open_line(host: String) -> (Line, futures::channel::mpsc::UnboundedReceiver<Value>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let (in_tx, in_rx) = futures::channel::mpsc::unbounded::<Value>();
    let rtt = Arc::new(Mutex::new(0.15f64));
    let open = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let (rtt2, open2, stop2) = (rtt.clone(), open.clone(), stop.clone());
    couch::rt::spawn(async move {
        let t0 = Instant::now();
        let mut retry = 0u32;
        while !stop2.load(Ordering::Relaxed) {
            let url = couch::dispatch::invoke("crew_line", json!({ "host": host })).await.ok().and_then(|v| v.as_str().map(|s| s.to_string()));
            let ws = match url {
                Some(u) => tokio_tungstenite::connect_async(u).await.ok().map(|(ws, _)| ws),
                None => None,
            };
            if let Some(ws) = ws {
                retry = 0;
                open2.store(true, Ordering::Relaxed);
                let (mut tx, mut rx) = ws.split();
                let mut ping = tokio::time::interval(Duration::from_secs(5));
                loop {
                    if stop2.load(Ordering::Relaxed) {
                        let _ = tx.close().await;
                        break;
                    }
                    tokio::select! {
                        _ = ping.tick() => {
                            let c = t0.elapsed().as_secs_f64() * 1000.;
                            if tx.send(tokio_tungstenite::tungstenite::Message::Text(json!({ "k": "ping", "c": c }).to_string().into())).await.is_err() { break; }
                        }
                        m = out_rx.recv() => {
                            let Some(m) = m else { break };
                            if tx.send(tokio_tungstenite::tungstenite::Message::Text(m.into())).await.is_err() { break; }
                        }
                        m = rx.next() => {
                            let Some(Ok(m)) = m else { break };
                            let Ok(text) = m.into_text() else { continue };
                            let Ok(d) = serde_json::from_str::<Value>(&text) else { continue };
                            if d["k"] == "pong" {
                                if let Some(c) = d["c"].as_f64() {
                                    let r = (t0.elapsed().as_secs_f64() * 1000. - c) / 1000.;
                                    if r > 0. && r < 5. {
                                        let mut g = rtt2.lock();
                                        *g = *g * 0.7 + r * 0.3;
                                    }
                                }
                                continue;
                            }
                            if in_tx.unbounded_send(d).is_err() { break; }
                        }
                    }
                }
                open2.store(false, Ordering::Relaxed);
            }
            if stop2.load(Ordering::Relaxed) {
                break;
            }
            let wait = (800u64 * 2u64.pow(retry.min(4))).min(10_000);
            retry += 1;
            tokio::time::sleep(Duration::from_millis(wait)).await;
        }
    });
    (Line { out: out_tx, rtt, open, stop }, in_rx)
}

impl Line {
    fn live(&self) -> bool {
        self.open.load(std::sync::atomic::Ordering::Relaxed)
    }
    fn send(&self, v: Value) {
        let _ = self.out.send(v.to_string());
    }
    fn rtt(&self) -> f64 {
        *self.rtt.lock()
    }
}

pub struct TogetherHandle(pub Entity<Together>);
impl Global for TogetherHandle {}

pub fn get(cx: &App) -> Entity<Together> {
    cx.global::<TogetherHandle>().0.clone()
}

pub fn init(cx: &mut App) {
    let t = cx.new(|cx: &mut Context<Together>| {
        // who's asking to join my couch
        cx.spawn(async move |this, cx| loop {
            crate::timer::after(Duration::from_secs(5)).await;
            let r = api::call::<Value>("crew_inbox", json!({})).await;
            let Ok(r) = r else { continue };
            let asks: Vec<Value> = r["asks"].as_array().cloned().unwrap_or_default();
            if this
                .update(cx, |t, cx| {
                    t.inbox = asks;
                    cx.notify();
                })
                .is_err()
            {
                break;
            }
        })
        .detach();
        // the beat: host pushes state, guest re-checks drift
        cx.spawn(async move |this, cx| loop {
            crate::timer::after(Duration::from_millis(100)).await;
            if this.update(cx, |t, cx| t.tick(cx)).is_err() {
                break;
            }
        })
        .detach();
        Together { seek_cost: 1.2, nudged: 1., ..Default::default() }
    });
    cx.set_global(TogetherHandle(t));
}

static TICKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Together {
    fn tick(&mut self, cx: &mut Context<Self>) {
        let n = TICKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.hosting.is_some() {
            self.host_tick(cx);
        }
        if self.following.is_some() && n % 3 == 0 {
            self.apply(cx);
        }
    }

    // ------------------------------------------------------------ host
    fn host_state(&self, cx: &App) -> Value {
        let p = playback::get(cx).read(cx);
        let rtt = self.line.as_ref().map_or(0.15, |l| l.rtt());
        json!({
            "k": "s", "v": p.video_id, "m": p.meta.as_ref().map(|m| m.id.clone()), "ty": p.meta.as_ref().map(|m| m.kind.clone()),
            "ti": p.meta.as_ref().map(|m| m.name.clone()), "p": p.state.pos, "pa": p.state.paused,
            "st": p.state.buffering && !p.state.paused, "sh": self.shared, "lag": rtt / 2. * 1000.,
            "sk": p.stream.as_ref().map(|s| playback::host_tag(s).to_string()),
        })
    }

    fn host_tick(&mut self, cx: &mut Context<Self>) {
        if !playback::get(cx).read(cx).open {
            self.stop_hosting();
            return;
        }
        let Some(line) = &self.line else { return };
        if !line.live() {
            return;
        }
        let st = self.host_state(cx);
        let now = Instant::now();
        let s = &self.sent;
        let since = s.at.map_or(f64::MAX, |a| now.duration_since(a).as_secs_f64());
        let expect = if s.pa || s.st { s.p } else { s.p + since.min(1e6) };
        let changed = st["v"].as_str().unwrap_or("") != s.v
            || st["sk"].as_str().unwrap_or("") != s.sk
            || st["pa"].as_bool() != Some(s.pa)
            || st["st"].as_bool() != Some(s.st)
            || st["sh"].as_bool() != Some(s.sh)
            || (st["p"].as_f64().unwrap_or(0.) - expect).abs() > 0.75;
        if changed || since > 1.0 {
            line.send(st.clone());
            self.sent = Sent {
                v: st["v"].as_str().unwrap_or("").into(),
                pa: st["pa"].as_bool().unwrap_or(true),
                st: st["st"].as_bool().unwrap_or(false),
                sh: self.shared,
                p: st["p"].as_f64().unwrap_or(0.),
                sk: st["sk"].as_str().unwrap_or("").into(),
                at: Some(now),
            };
        }
    }

    pub fn start_hosting(&mut self, me: String, cx: &mut Context<Self>) {
        if self.hosting.as_deref() == Some(me.as_str()) && self.line.is_some() {
            return;
        }
        self.hosting = Some(me.clone());
        self.sent = Sent::default();
        let (line, mut rx) = open_line(me);
        self.line = Some(line);
        // a guest with the remote: the player does what they asked
        cx.spawn(async move |_this, cx| {
            while let Some(d) = rx.next().await {
                if d["k"] != "cmd" {
                    continue;
                }
                let action = d["action"].as_str().or(d["a"].as_str()).unwrap_or("").to_string();
                let pos = d["pos"].as_f64().or(d["p"].as_f64());
                let _ = cx.update(|cx| {
                    let p = playback::get(cx);
                    let p = p.read(cx);
                    match action.as_str() {
                        "pause" => p.cmd("pause", json!({ "value": true })),
                        "play" => p.cmd("pause", json!({ "value": false })),
                        "seek" => {
                            if let Some(t) = pos {
                                p.cmd("seek", json!({ "to": t }))
                            }
                        }
                        _ => {}
                    }
                });
            }
        })
        .detach();
    }

    fn stop_hosting(&mut self) {
        if self.hosting.take().is_some() {
            self.line = None;
        }
    }

    pub fn set_shared(&mut self, on: bool, cx: &mut Context<Self>) {
        self.shared = on;
        couch::rt::spawn(async move {
            let _ = couch::dispatch::invoke("crew_share_control", json!({ "on": on })).await;
        });
        let msg = if on { "everyone on your couch can control playback" } else { "you've got the remote back" };
        cx.defer(move |cx| Nav::toast(msg.into(), cx));
    }

    pub fn answer(&mut self, ask: Value, accept: bool, cx: &mut Context<Self>) {
        let id = ask["id"].as_str().unwrap_or("").to_string();
        self.inbox.retain(|a| a["id"].as_str() != Some(id.as_str()));
        cx.notify();
        let name = ask["fromName"].as_str().unwrap_or("someone").to_string();
        let from = ask["from"].as_str().unwrap_or("").to_string();
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("crew_answer", json!({ "id": id, "accept": accept })).await;
            if accept && r.is_ok() {
                let me = api::call::<Value>("discord_me", json!({})).await.ok().and_then(|v| v["id"].as_str().map(|s| s.to_string()));
                crate::views::crewx::note_together(&from);
                let _ = this.update(cx, |t, cx| {
                    if let Some(me) = me {
                        t.start_hosting(me, cx);
                    }
                    cx.defer(move |cx| Nav::toast(format!("{name} is on your couch"), cx));
                });
            }
        })
        .detach();
    }

    // ------------------------------------------------------------ guest
    pub fn ask_to_join(&mut self, host: String, name: String, cx: &mut Context<Self>) {
        if self.asking.is_some() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let id = match api::call::<Value>("crew_ask", json!({ "to": host })).await {
                Ok(v) => v["id"].as_str().unwrap_or("").to_string(),
                Err(e) => {
                    let _ = cx.update(|cx| Nav::toast(e, cx));
                    return;
                }
            };
            let _ = this.update(cx, |t, cx| {
                t.asking = Some((id.clone(), host.clone(), name.clone()));
                cx.notify();
            });
            let until = Instant::now() + Duration::from_secs(120);
            while Instant::now() < until {
                crate::timer::after(Duration::from_secs(2)).await;
                let still = this.update(cx, |t, _| t.asking.as_ref().map(|a| a.0.clone()) == Some(id.clone())).unwrap_or(false);
                if !still {
                    return;
                }
                let status = api::call::<Value>("crew_ask_status", json!({ "id": id })).await.ok().and_then(|v| v["status"].as_str().map(|s| s.to_string())).unwrap_or("pending".into());
                if status == "pending" {
                    continue;
                }
                let _ = this.update(cx, |t, cx| {
                    t.asking = None;
                    cx.notify();
                    if status == "accepted" {
                        t.follow(host.clone(), name.clone(), cx);
                    } else {
                        let msg = match status.as_str() {
                            "declined" => format!("{name} wants the couch to themselves"),
                            "ended" => format!("{name} stopped watching"),
                            _ => "that ask expired".into(),
                        };
                        cx.defer(move |cx| Nav::toast(msg, cx));
                    }
                });
                return;
            }
            let _ = this.update(cx, |t, cx| {
                t.asking = None;
                cx.notify();
                let msg = format!("{name} didn't answer");
                cx.defer(move |cx| Nav::toast(msg, cx));
            });
        })
        .detach();
    }

    pub fn cancel_ask(&mut self, cx: &mut Context<Self>) {
        if let Some((id, _, _)) = self.asking.take() {
            couch::rt::spawn(async move {
                let _ = couch::dispatch::invoke("crew_cancel_ask", json!({ "id": id })).await;
            });
        }
        cx.notify();
    }

    fn follow(&mut self, host: String, name: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            // the host's player reports within a second or two
            let mut room = Value::Null;
            for _ in 0..10 {
                room = api::call::<Value>("crew_room", json!({ "host": host })).await.map(|v| v["room"].clone()).unwrap_or(Value::Null);
                if !room.is_null() {
                    break;
                }
                crate::timer::after(Duration::from_secs(1)).await;
            }
            if room.is_null() {
                let _ = cx.update(|cx| Nav::toast(format!("couldn't catch up with {name}"), cx));
                return;
            }
            let meta = Meta { id: room["metaId"].as_str().unwrap_or("").into(), kind: room["type"].as_str().unwrap_or("movie").into(), name: room["title"].as_str().unwrap_or("").into(), ..Default::default() };
            let vid = room["videoId"].as_str().map(|s| s.to_string());
            let start = room["now"].as_f64().map(|n| n + 2.);
            let prefer: Option<Value> = room["sk"].as_str().and_then(|s| serde_json::from_str(s).ok());
            crate::views::crewx::note_together(&host);
            let _ = cx.update(|cx| {
                Nav::toast(format!("watching with {name}"), cx);
                playback::quick_play_with(meta, vid, start, prefer, cx);
            });
            let (line, mut rx) = open_line(host.clone());
            let _ = this.update(cx, |t, _| {
                t.following = Some((host.clone(), name.clone()));
                t.host_st = None;
                t.seek_at = None;
                t.nudged = 1.;
                t.line = Some(line);
                t.waiting_told = false;
                t.take_state(&room);
                // the first load is under way: don't start another until it plays
                t.switching = true;
            });
            cx.spawn({
                let this = this.clone();
                async move |cx| {
                    let until = Instant::now() + Duration::from_secs(25);
                    loop {
                        crate::timer::after(Duration::from_millis(500)).await;
                        let done = cx.update(|cx| {
                            let p = playback::get(cx).read(cx);
                            p.open && !p.loading && p.state.duration > 0.
                        });
                        if done || Instant::now() > until {
                            break;
                        }
                    }
                    let _ = this.update(cx, |t, _| {
                        t.switching = false;
                        t.seek_at = Some(Instant::now());
                    });
                }
            })
            .detach();
            // the room itself, every few seconds: the host stopping ends it, and
            // when the live line goes quiet the room still says where they are
            {
                let (host, name) = (host.clone(), name.clone());
                cx.spawn({
                    let this = this.clone();
                    async move |cx| {
                        let mut gone = 0;
                        loop {
                            crate::timer::after(Duration::from_secs(4)).await;
                            let on = this.update(cx, |t, _| t.following.as_ref().map(|f| f.0.clone()) == Some(host.clone())).unwrap_or(false);
                            if !on {
                                break;
                            }
                            let room = api::call::<Value>("crew_room", json!({ "host": host })).await.map(|v| v["room"].clone());
                            match room {
                                Ok(r) if r.is_null() => {
                                    gone += 1;
                                    if gone >= 2 {
                                        let msg = format!("{name} stopped watching — you're on your own now");
                                        let _ = this.update(cx, |t, cx| {
                                            t.leave(cx);
                                            cx.defer(move |cx| Nav::toast(msg, cx));
                                        });
                                        break;
                                    }
                                }
                                Ok(r) => {
                                    gone = 0;
                                    let _ = this.update(cx, |t, cx| {
                                        let quiet = t.host_st.as_ref().map_or(true, |h| h.got.elapsed() > Duration::from_secs(5));
                                        if quiet {
                                            t.take_state(&r);
                                            t.apply(cx);
                                        }
                                    });
                                }
                                Err(_) => {}
                            }
                        }
                    }
                })
                .detach();
            }
            while let Some(d) = rx.next().await {
                if d["k"] != "s" {
                    continue;
                }
                if this
                    .update(cx, |t, cx| {
                        t.take_state(&d);
                        t.apply(cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn take_state(&mut self, d: &Value) {
        self.host_st = Some(HostState {
            v: d["v"].as_str().or(d["videoId"].as_str()).unwrap_or("").into(),
            m: d["m"].as_str().or(d["metaId"].as_str()).unwrap_or("").into(),
            ty: d["ty"].as_str().or(d["type"].as_str()).unwrap_or("").into(),
            ti: d["ti"].as_str().or(d["title"].as_str()).unwrap_or("").into(),
            sk: d["sk"].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or(Value::Null),
            p: d["p"].as_f64().or(d["now"].as_f64()).unwrap_or(0.),
            pa: d["pa"].as_bool().or(d["paused"].as_bool()).unwrap_or(false),
            st: d["st"].as_bool().or(d["stalled"].as_bool()).unwrap_or(false),
            got: Instant::now(),
        });
        self.can_control = d["sh"].as_bool().or(d["shared"].as_bool()).unwrap_or(false);
    }

    fn host_now(&self) -> f64 {
        let Some(h) = &self.host_st else { return 0. };
        if h.pa || h.st {
            return h.p;
        }
        h.p + h.got.elapsed().as_secs_f64() + self.line.as_ref().map_or(0.15, |l| l.rtt()) / 2.
    }

    fn nudge(&mut self, rate: f64, cx: &App) {
        if (rate - self.nudged).abs() < 0.005 {
            return;
        }
        self.nudged = rate;
        playback::get(cx).read(cx).cmd("speed", json!({ "value": rate }));
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        let Some(h) = self.host_st.clone() else { return };
        if self.following.is_none() {
            return;
        }
        let pbe = playback::get(cx);
        let (open, loading, video_id, s, meta) = {
            let p = pbe.read(cx);
            (p.open, p.loading, p.video_id.clone(), p.state.clone(), p.meta.clone())
        };
        if !open {
            // (still opening the first time / the next one: not a "close")
            if !self.switching {
                self.leave(cx);
            }
            return;
        }
        // the host moved on (next episode, another episode, another title):
        // come along, to the moment they're at (not your own saved spot)
        if !h.v.is_empty() && h.v != video_id && !loading && !self.switching {
            let same = meta.as_ref().map_or(false, |m| h.m.is_empty() || m.id == h.m);
            let m = match meta {
                Some(m) if same => m,
                _ => Meta { id: h.m.clone(), kind: if h.ty.is_empty() { "movie".into() } else { h.ty.clone() }, name: h.ti.clone(), ..Default::default() },
            };
            if !m.id.is_empty() {
                self.switching = true;
                self.nudge(1., cx);
                let v = h.v.clone();
                let at = if h.pa { h.p } else { self.host_now() + self.seek_cost };
                let start = (at > 3.).then_some(at);
                let prefer = (!h.sk.is_null()).then(|| h.sk.clone());
                cx.defer(move |cx| playback::quick_play_with(m, Some(v), start, prefer, cx));
                cx.spawn(async move |this, cx| {
                    // until the new one is actually playing (or 20 s)
                    let until = Instant::now() + Duration::from_secs(20);
                    loop {
                        crate::timer::after(Duration::from_millis(500)).await;
                        let done = cx.update(|cx| {
                            let p = playback::get(cx).read(cx);
                            !p.loading && p.state.duration > 0.
                        });
                        if done || Instant::now() > until {
                            break;
                        }
                    }
                    let _ = this.update(cx, |t, _| {
                        t.switching = false;
                        t.seek_at = Some(Instant::now());
                    });
                })
                .detach();
            }
            return;
        }
        if loading || self.switching || self.steered_at.map_or(false, |a| a.elapsed() < Duration::from_millis(1500)) {
            return;
        }
        // the host moved to another stream of the same thing: the same file as them
        if !h.sk.is_null() && !pbe.read(cx).on_host_stream(&h.sk) && self.sk_applied != h.sk {
            self.sk_applied = h.sk.clone();
            if let Some(s) = pbe.read(cx).host_choice(&h.sk) {
                let at = self.host_now() + self.seek_cost;
                self.switching = true;
                pbe.update(cx, |p, cx| p.use_stream(s, Some(at.max(0.)), cx));
                cx.spawn(async move |this, cx| {
                    crate::timer::after(Duration::from_secs(6)).await;
                    let _ = this.update(cx, |t, _| {
                        t.switching = false;
                        t.seek_at = Some(Instant::now());
                    });
                })
                .detach();
                return;
            }
        }
        let p = pbe.read(cx);
        // a seek still landing: let it
        if let Some(at) = self.seek_at {
            let e = at.elapsed();
            if e < Duration::from_secs(10) && (s.buffering || e < Duration::from_millis(700)) {
                return;
            }
            if e < Duration::from_secs(12) && !s.buffering && !s.paused {
                self.seek_cost = (self.seek_cost * 0.6 + e.as_secs_f64() * 0.4).clamp(0.3, 6.);
                self.seek_at = None;
            }
        }
        // nothing from the host for a while (their connection dropped?): hold
        // here rather than run off on our own
        if h.got.elapsed() > Duration::from_secs(12) {
            if !s.paused {
                p.cmd("pause", json!({ "value": true }));
            }
            if !self.waiting_told {
                self.waiting_told = true;
                let name = self.following.as_ref().map(|f| f.1.clone()).unwrap_or_default();
                cx.defer(move |cx| Nav::toast(format!("waiting for {name}…"), cx));
            }
            return;
        }
        self.waiting_told = false;
        let target = self.host_now();
        let drift = target - s.pos;
        if !h.st {
            self.stalled_since = None;
        }
        let stopped = h.pa || (h.st && self.stalled_since.get_or_insert_with(Instant::now).elapsed() > Duration::from_millis(900));
        if stopped {
            let _ = p;
            self.nudge(1., cx);
            let p = pbe.read(cx);
            if !s.paused {
                p.cmd("pause", json!({ "value": true }));
            }
            // paused: land on their exact frame; just loading (subtitles, a
            // hiccup): wait for them without jumping around unless far off
            if drift.abs() > if h.pa { 0.4 } else { 2.0 } {
                self.seek_at = Some(Instant::now());
                p.cmd("seek", json!({ "to": target }));
            }
            return;
        }
        if h.st {
            return;
        }
        if s.paused {
            if drift.abs() > 0.6 {
                self.seek_at = Some(Instant::now());
                p.cmd("seek", json!({ "to": target + self.seek_cost.min(1.5) }));
            }
            p.cmd("pause", json!({ "value": false }));
            return;
        }
        if s.buffering {
            return;
        }
        // a jump (they skipped): seek; small drift: ease it out with speed
        if drift.abs() > 2.5 {
            let _ = p;
            self.nudge(1., cx);
            self.seek_at = Some(Instant::now());
            pbe.read(cx).cmd("seek", json!({ "to": target + self.seek_cost }));
        } else if drift.abs() > 0.25 {
            // a gentle, steady catch-up (two fixed steps), not a rate that wobbles every second
            let step = if drift.abs() > 1. { 0.08 } else { 0.04 };
            self.nudge(1. + step * drift.signum(), cx);
        } else if drift.abs() < 0.08 {
            self.nudge(1., cx);
        }
    }

    /// Guest with the remote: send pause / play / seek to the host.
    pub fn steer(&mut self, action: &str, pos: Option<f64>) -> bool {
        let Some((host, _)) = &self.following else { return false };
        if !self.can_control {
            return false;
        }
        self.steered_at = Some(Instant::now());
        match &self.line {
            Some(l) if l.live() => l.send(json!({ "k": "cmd", "a": action, "action": action, "p": pos, "pos": pos })),
            _ => {
                let (host, action) = (host.clone(), action.to_string());
                couch::rt::spawn(async move {
                    let _ = couch::dispatch::invoke("crew_room_cmd", json!({ "host": host, "action": action, "pos": pos })).await;
                });
            }
        }
        true
    }

    /// Something a guest tried that only the host can do: say so (a crossed-out
    /// symbol over the player, or a toast elsewhere).
    pub fn deny(&mut self, what: &str, cx: &mut Context<Self>) {
        let (icon, words) = match what {
            "pause" => ("pause", "guests can't pause"),
            "play" => ("play", "guests can't press play"),
            "seek" => ("forward10", "guests can't skip around"),
            "speed" => ("gauge", "guests can't change the speed"),
            "episode" => ("film", "only the host changes episodes"),
            _ => ("film", "only the host picks what's on"),
        };
        let host = self.following.as_ref().map(|f| f.1.clone()).unwrap_or_default();
        let words = if host.is_empty() { words.to_string() } else { format!("{words} — {host} has the remote") };
        self.denied = Some((icon, words.clone(), Instant::now()));
        cx.notify();
        if !crate::Nav::on_player(cx) {
            cx.defer(move |cx| Nav::toast(words, cx));
        }
    }

    pub fn leave(&mut self, cx: &mut Context<Self>) {
        let f = self.following.take();
        self.line = None;
        self.host_st = None;
        self.can_control = false;
        if self.nudged != 1. {
            self.nudge(1., cx);
        }
        if let Some((host, _)) = f {
            couch::rt::spawn(async move {
                let _ = couch::dispatch::invoke("crew_leave", json!({ "host": host })).await;
            });
        }
        cx.notify();
    }
}
