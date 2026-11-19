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
    /// who I let onto my couch this time (Discord: "watching with …")
    guests: Vec<String>,
    /// host: who's on my couch right now (from the server)
    pub roster: Vec<Guest>,
    /// host: paused because a guest is buffering (who, since)
    pub held: Option<(String, Instant)>,
    /// host: a hold ran long or I pressed play: no new hold until then
    hold_quiet: Option<Instant>,
    /// host: when the player closed (a short grace before the room ends)
    closed_at: Option<Instant>,
    /// guest: the host left; paused here until I choose (host name)
    pub orphan: Option<String>,
    /// guest: my player has been loading / buffering since
    buf_since: Option<Instant>,
    buf_sent: bool,
    buf_clear: Option<Instant>,
    /// asks I've seen (when) and the ones I waved off
    ask_seen: std::collections::HashMap<String, Instant>,
    ask_hidden: std::collections::HashSet<String>,
}

/// How long a "wants to join" card stays up without an answer.
pub const ASK_SHOWN: Duration = Duration::from_secs(45);

#[derive(Clone, Debug)]
pub struct Guest {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub live: bool,
    pub remote: bool,
    pub buf: bool,
}

/// Discord's "watching with Mari" / "watching with 3 others".
fn company(names: &[String]) -> Option<String> {
    match names.len() {
        0 => None,
        1 => Some(format!("watching with {}", names[0])),
        n => Some(format!("watching with {n} others")),
    }
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
                    for a in &asks {
                        if let Some(id) = a["id"].as_str() {
                            t.ask_seen.entry(id.to_string()).or_insert_with(Instant::now);
                        }
                    }
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
        // still loading (a new episode, another stream): guests hold, on the
        // last moment we know (not the 0:00 a fresh player reports)
        let loading = p.loading || p.state.duration <= 0.;
        let pos = if loading { if p.video_id == self.sent.v { self.sent.p } else { p.start_hint.unwrap_or(0.) } } else { p.state.pos };
        json!({
            "k": "s", "v": p.video_id, "m": p.meta.as_ref().map(|m| m.id.clone()), "ty": p.meta.as_ref().map(|m| m.kind.clone()),
            "ti": p.meta.as_ref().map(|m| m.name.clone()), "p": pos, "pa": p.state.paused && !loading,
            "st": loading || (p.state.buffering && !p.state.paused), "sh": self.shared, "lag": rtt / 2. * 1000.,
            "sk": (!loading).then(|| p.stream.as_ref().map(|s| playback::host_tag(s).to_string())).flatten(),
        })
    }

    /// host: someone's player is behind (loading / buffering): pause for them,
    /// play again once they're ready. Pressing play yourself overrides it.
    fn host_hold(&mut self, cx: &mut Context<Self>) {
        let (paused, loading) = {
            let p = playback::get(cx).read(cx);
            (p.state.paused, p.loading)
        };
        let lag = self.roster.iter().find(|g| g.live && g.buf).map(|g| g.name.clone());
        if let Some((who, at)) = self.held.clone() {
            let over = at.elapsed() > Duration::from_secs(25);
            if lag.is_none() || !paused || over {
                self.held = None;
                // they're ready: carry on (unless I already did, or it ran long)
                if paused && lag.is_none() {
                    playback::get(cx).read(cx).cmd("pause", json!({ "value": false }));
                }
                if over || !paused {
                    self.hold_quiet = Some(Instant::now());
                }
                if over {
                    cx.defer(move |cx| Nav::toast(format!("{who}'s connection is slow, carrying on without waiting"), cx));
                }
                cx.notify();
            }
            return;
        }
        let quiet = self.hold_quiet.map_or(false, |t| t.elapsed() < Duration::from_secs(20));
        if let (Some(who), false, false, false) = (lag, paused, loading, quiet) {
            playback::get(cx).read(cx).cmd("pause", json!({ "value": true }));
            self.held = Some((who, Instant::now()));
            cx.notify();
        }
    }

    pub fn kick(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(l) = &self.line {
            l.send(json!({ "k": "kick", "g": id }));
        }
        let name = self.roster.iter().find(|g| g.id == id).map(|g| g.name.clone()).unwrap_or_default();
        self.roster.retain(|g| g.id != id);
        self.guests.retain(|g| g != &name);
        couch::discord::set_company(company(&self.guests));
        cx.defer(move |cx| Nav::toast(format!("{name} is off your couch"), cx));
        cx.notify();
    }

    /// hand one guest the remote (play / pause / seek), or take it back
    pub fn give_remote(&mut self, id: &str, on: bool, cx: &mut Context<Self>) {
        if let Some(l) = &self.line {
            l.send(json!({ "k": "remote", "g": id, "on": on }));
        }
        if let Some(g) = self.roster.iter_mut().find(|g| g.id == id) {
            g.remote = on;
            let msg = if on { format!("{} has the remote too", g.name) } else { format!("you took the remote back from {}", g.name) };
            cx.defer(move |cx| Nav::toast(msg, cx));
        }
        cx.notify();
    }

    /// the asks worth showing: recent, not waved off, and for what I'm on now
    pub fn asks_now(&self, cx: &App) -> Vec<Value> {
        let p = playback::get(cx).read(cx);
        let on = p.open.then(|| p.meta.as_ref().map(|m| m.id.clone())).flatten();
        self.inbox
            .iter()
            .filter(|a| {
                let id = a["id"].as_str().unwrap_or("");
                let fresh = self.ask_seen.get(id).map_or(true, |t| t.elapsed() < ASK_SHOWN);
                let same = match (a["metaId"].as_str(), &on) {
                    (Some(m), Some(o)) => m == o,
                    (Some(_), None) => false,
                    _ => true,
                };
                fresh && same && !self.ask_hidden.contains(id)
            })
            .cloned()
            .collect()
    }
    /// how much of an ask's time is left (1 → 0)
    pub fn ask_left(&self, id: &str) -> f32 {
        self.ask_seen.get(id).map_or(1., |t| 1. - (t.elapsed().as_secs_f32() / ASK_SHOWN.as_secs_f32()).min(1.))
    }
    pub fn hide_ask(&mut self, id: &str, cx: &mut Context<Self>) {
        self.ask_hidden.insert(id.to_string());
        cx.notify();
    }

    fn host_tick(&mut self, cx: &mut Context<Self>) {
        if !playback::get(cx).read(cx).open {
            // closed for real (not just the next episode loading): end it
            let at = *self.closed_at.get_or_insert_with(Instant::now);
            if at.elapsed() > Duration::from_millis(1500) {
                self.end_hosting();
                cx.notify();
            }
            return;
        }
        self.closed_at = None;
        self.host_hold(cx);
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
                if d["k"] == "guests" {
                    let list: Vec<Guest> = d["list"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|g| Guest {
                            id: g["id"].as_str().unwrap_or("").into(),
                            name: g["name"].as_str().unwrap_or("someone").into(),
                            avatar: g["avatar"].as_str().map(String::from),
                            live: g["live"].as_bool().unwrap_or(false),
                            remote: g["remote"].as_bool().unwrap_or(false),
                            buf: g["buf"].as_bool().unwrap_or(false),
                        })
                        .collect();
                    if _this
                        .update(cx, |t, cx| {
                            t.guests = list.iter().map(|g| g.name.clone()).collect();
                            couch::discord::set_company(company(&t.guests));
                            t.roster = list;
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                    continue;
                }
                if d["k"] == "left" {
                    let name = d["name"].as_str().unwrap_or("someone").to_string();
                    let _ = cx.update(|cx| Nav::toast(format!("{name} left your couch"), cx));
                    continue;
                }
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
            self.guests.clear();
            self.roster.clear();
            self.held = None;
            self.closed_at = None;
            couch::discord::set_company(None);
        }
    }

    /// the host closed the player: guests are told now (they pause and choose)
    fn end_hosting(&mut self) {
        if self.hosting.is_none() {
            return;
        }
        if let Some(l) = &self.line {
            l.send(json!({ "k": "bye" }));
        }
        couch::rt::spawn(async move {
            let _ = couch::dispatch::invoke("crew_end_room", json!({})).await;
        });
        // give the goodbye a moment on the line before it closes
        let line = self.line.take();
        couch::rt::spawn(async move {
            tokio::time::sleep(Duration::from_millis(800)).await;
            drop(line);
        });
        self.stop_hosting();
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
            if let (true, Err(e)) = (accept, &r) {
                let e = e.clone();
                let _ = cx.update(|cx| Nav::toast(e, cx));
            }
            if accept && r.is_ok() {
                let me = api::call::<Value>("discord_me", json!({})).await.ok().and_then(|v| v["id"].as_str().map(|s| s.to_string()));
                crate::views::crewx::note_together(&from);
                let _ = this.update(cx, |t, cx| {
                    if !t.guests.contains(&name) {
                        t.guests.push(name.clone());
                    }
                    couch::discord::set_company(company(&t.guests));
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
            // the host's card runs out a little sooner (ASK_SHOWN)
            let until = Instant::now() + ASK_SHOWN + Duration::from_secs(5);
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
            {
                let id = id.clone();
                couch::rt::spawn(async move {
                    let _ = couch::dispatch::invoke("crew_cancel_ask", json!({ "id": id })).await;
                });
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
            couch::discord::set_company(company(&[name.clone()]));
            let _ = cx.update(|cx| {
                Nav::toast(format!("watching with {name}"), cx);
                playback::quick_play_with(meta, vid, start, prefer, cx);
            });
            let (line, mut rx) = open_line(host.clone());
            let _ = this.update(cx, |t, _| {
                t.orphan = None;
                t.buf_since = None;
                t.buf_sent = false;
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
                let host = host.clone();
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
                                        let _ = this.update(cx, |t, cx| t.host_gone(cx));
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
                if d["k"] == "kicked" {
                    let msg = format!("{name} took you off their couch");
                    let _ = this.update(cx, |t, cx| {
                        t.leave(cx);
                        cx.defer(move |cx| Nav::toast(msg, cx));
                    });
                    break;
                }
                if d["k"] == "hostleft" {
                    let _ = this.update(cx, |t, cx| t.host_gone(cx));
                    break;
                }
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
        // tell the host when I'm behind for more than a moment, so they wait
        // (once the host has paused for me: still behind until a few seconds are loaded)
        let thin = self.buf_sent && s.paused && s.duration > 0. && s.cache_ahead < 3.;
        let behind = open && (loading || self.switching || (s.buffering && !s.paused) || thin);
        if behind {
            let since = *self.buf_since.get_or_insert_with(Instant::now);
            if !self.buf_sent && since.elapsed() > Duration::from_millis(1500) {
                self.buf_sent = true;
                if let Some(l) = &self.line {
                    l.send(json!({ "k": "buf", "on": true }));
                }
            }
            self.buf_clear = None;
        } else {
            self.buf_since = None;
            let clear = *self.buf_clear.get_or_insert_with(Instant::now);
            if self.buf_sent && clear.elapsed() > Duration::from_millis(800) {
                self.buf_sent = false;
                if let Some(l) = &self.line {
                    l.send(json!({ "k": "buf", "on": false }));
                }
            }
        }
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
                let start = Some(at.max(0.));
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
            // (the host only loading: no seeking about; we catch up once they play)
            if h.pa && drift.abs() > 0.4 {
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
        let words = if host.is_empty() { words.to_string() } else { format!("{words} · {host} has the remote") };
        self.denied = Some((icon, words.clone(), Instant::now()));
        cx.notify();
        if !crate::Nav::on_player(cx) {
            cx.defer(move |cx| Nav::toast(words, cx));
        }
    }

    /// the host left: stop on this frame, then the guest picks (on their own, or stop)
    fn host_gone(&mut self, cx: &mut Context<Self>) {
        let Some((_, name)) = self.following.clone() else { return };
        let p = playback::get(cx);
        if p.read(cx).open {
            p.read(cx).cmd("pause", json!({ "value": true }));
            self.leave(cx);
            self.orphan = Some(name.clone());
            if !crate::Nav::on_player(cx) {
                cx.defer(move |cx| Nav::toast(format!("{name} stopped watching"), cx));
            }
        } else {
            self.leave(cx);
            cx.defer(move |cx| Nav::toast(format!("{name} stopped watching"), cx));
        }
        cx.notify();
    }
    /// after the host left: "keep watching on your own"
    pub fn keep_alone(&mut self, cx: &mut Context<Self>) {
        self.orphan = None;
        playback::get(cx).read(cx).cmd("pause", json!({ "value": false }));
        cx.notify();
    }
    /// after the host left: "stop"
    pub fn stop_too(&mut self, cx: &mut Context<Self>) {
        self.orphan = None;
        let p = playback::get(cx);
        p.update(cx, |p, cx| p.close(cx));
        cx.defer(|cx| Nav::back(cx));
        cx.notify();
    }

    /// a guest without the remote: play / pause / seek / next aren't theirs
    pub fn locked(&self) -> bool {
        self.following.is_some() && !self.can_control
    }

    pub fn leave(&mut self, cx: &mut Context<Self>) {
        let f = self.following.take();
        self.orphan = None;
        self.buf_since = None;
        self.buf_sent = false;
        couch::discord::set_company(None);
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
