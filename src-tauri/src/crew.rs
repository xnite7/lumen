//! The midnite crew: "on the couch" presence shared with friends through a
//! tiny Cloudflare Worker (see /worker).
//!
//! Always on: what you're playing (Simkl's title and poster when it knows
//! it, otherwise the title's own metadata -- never a file name), under your
//! Discord name and avatar. The server forgets you two minutes after the last
//! heartbeat, so closing the app is enough to disappear.

use crate::core::Core;
use crate::discord::Known;
use crate::error::{AppError, AppResult};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const HEARTBEAT: Duration = Duration::from_secs(45);

struct Sent {
    at: Instant,
    key: String,
}
static LAST: Mutex<Option<Sent>> = Mutex::new(None);

/// The crew server (built in; COUCH_CREW_URL at build time overrides it).
pub const CREW_URL: &str = match option_env!("COUCH_CREW_URL") {
    Some(u) => u,
    None => "https://midnite-couch.xnite7.workers.dev",
};

/// You on the couch: your Discord id (the server vouches for it).
pub fn member_id(core: &Core) -> String {
    core.db
        .get::<Value>("discord_user")
        .and_then(|u| u.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .unwrap_or_default()
}

/// (url, "Bearer <session>") once the server has checked you're in the
/// midnite Discord server. No codes, no invite links.
fn server(core: &Core) -> Option<(String, String)> {
    let session = core.db.secret("crew_session")?;
    let exp = core.db.get::<Value>("crew_session_exp").and_then(|v| v.as_u64()).unwrap_or(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    (exp > now + 60).then(|| (CREW_URL.trim_end_matches('/').to_string(), format!("Bearer {session}")))
}

/// Trade your Discord sign-in for a couch session (the server checks you're
/// in the midnite server with Discord itself).
pub async fn login(core: &Core) -> AppResult<()> {
    if server(core).is_some() {
        return Ok(());
    }
    let tok = crate::integrations::discord_auth::access_token(core).await.ok_or_else(|| AppError::msg("sign in with Discord first"))?;
    let r = core
        .http
        .client()
        .post(format!("{}/v1/auth", CREW_URL.trim_end_matches('/')))
        .json(&json!({ "token": tok }))
        .timeout(Duration::from_secs(12))
        .send()
        .await
        .map_err(|e| AppError::msg(format!("the crew server didn't answer: {e}")))?;
    let st = r.status().as_u16();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if st == 403 {
        let _ = core.db.set("discord_member", &json!(false));
        return Err(AppError::msg("not in the midnite server"));
    }
    let session = v.get("session").and_then(|s| s.as_str()).ok_or_else(|| AppError::msg(v.get("error").and_then(|e| e.as_str()).unwrap_or("the crew server said no").to_string()))?;
    core.db.set_secret("crew_session", Some(session))?;
    core.db.set("crew_session_exp", &json!(v.get("exp").and_then(|e| e.as_u64()).unwrap_or(0)))?;
    Ok(())
}

/// Your Discord name and avatar are who you are on the couch.
fn who(core: &Core) -> (String, Option<String>) {
    let d = core.db.get::<Value>("discord_user");
    let name = d
        .as_ref()
        .and_then(|u| u.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "someone".into());
    let avatar = d.as_ref().and_then(|u| u.get("avatar").and_then(|v| v.as_str()).map(|s| s.to_string()));
    (name, avatar)
}

/// Called from the player tracker on every tick: sends when something
/// changed, otherwise a heartbeat every 45 s.
pub fn playing(core: &Core, known: &Known, meta: &Value, video_id: &str, pos: f64, duration: f64, paused: bool) {
    host_tick(core, known, meta, video_id, pos, paused);
    let Some((url, code)) = server(core) else { return };
    // gatekeeper mode: you're on the couch, but what you're watching is yours
    let gate = core.settings.read().integrations.crew_gatekeeper;
    let key = format!("{}|{}|{}|{gate}", known.title, known.episode.clone().unwrap_or_default(), paused);
    {
        let mut last = LAST.lock();
        if let Some(l) = last.as_ref() {
            if l.key == key && l.at.elapsed() < HEARTBEAT {
                return;
            }
        }
        *last = Some(Sent { at: Instant::now(), key });
    }
    let (name, avatar) = who(core);
    let body = json!({
        "id": member_id(core),
        "name": name,
        "avatar": avatar,
        "watching": if gate { Value::Null } else { json!({
            "title": known.title,
            "year": known.year,
            "poster": known.poster,
            "episode": known.episode,
            "link": known.link,
            "metaId": meta.get("id"),
            "type": meta.get("type"),
            "background": meta.get("background"),
        }) },
        "paused": paused,
        "pos": pos,
        "duration": duration,
    });
    let client = core.http.client();
    tauri::async_runtime::spawn(async move {
        let _ = client.put(format!("{url}/v1/presence")).header("authorization", code).json(&body).timeout(Duration::from_secs(8)).send().await;
    });
}

/// Playback ended: leave the couch row.
pub fn stopped(core: &Core) {
    *LAST.lock() = None;
    if HOSTING.swap(false, std::sync::atomic::Ordering::Relaxed) {
        let id = member_id(core);
        let _ = send(core, reqwest::Method::DELETE, &format!("/v1/room/{id}"), None);
    }
    let Some((url, code)) = server(core) else { return };
    let id = member_id(core);
    let client = core.http.client();
    tauri::async_runtime::spawn(async move {
        let _ = client.delete(format!("{url}/v1/presence/{id}")).header("authorization", code).timeout(Duration::from_secs(8)).send().await;
    });
}

/// Who's on the couch right now (for the Home row and the Couch tab).
pub async fn list(core: &Core) -> AppResult<Value> {
    if login(core).await.is_err() {
        return Ok(json!({ "setup": false, "members": [] }));
    }
    let Some((url, code)) = server(core) else { return Ok(json!({ "setup": false, "members": [] })) };
    let me = member_id(core);
    let r = core
        .http
        .client()
        .get(format!("{url}/v1/presence"))
        .header("authorization", code)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| AppError::msg(format!("the crew server didn't answer: {e}")))?;
    if r.status().as_u16() == 401 {
        let _ = core.db.set_secret("crew_session", None);
        return Err(AppError::msg("your couch session ran out, trying again shortly"));
    }
    let v: Value = r.json().await.map_err(|e| AppError::msg(e.to_string()))?;
    let members: Vec<Value> = v.get("members").and_then(|m| m.as_array()).cloned().unwrap_or_default().into_iter().filter(|m| m.get("id").and_then(|i| i.as_str()) != Some(me.as_str())).collect();
    Ok(json!({ "setup": true, "members": members }))
}

// ------------------------------------------------------------ watch together

static HOSTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static LAST_ROOM: Mutex<Option<Instant>> = Mutex::new(None);

/// Fire-and-forget request to the crew server.
fn send(core: &Core, method: reqwest::Method, path: &str, body: Option<Value>) -> Option<()> {
    let (url, code) = server(core)?;
    let client = core.http.client();
    let path = path.to_string();
    tauri::async_runtime::spawn(async move {
        let mut r = client.request(method, format!("{url}{path}")).header("authorization", code).timeout(Duration::from_secs(8));
        if let Some(b) = body {
            r = r.json(&b);
        }
        let _ = r.send().await;
    });
    Some(())
}

async fn call(core: &Core, method: reqwest::Method, path: &str, body: Option<Value>) -> AppResult<Value> {
    let (url, code) = server(core).ok_or_else(|| AppError::msg("the crew isn't set up"))?;
    let mut r = core.http.client().request(method, format!("{url}{path}")).header("authorization", code).timeout(Duration::from_secs(8));
    if let Some(b) = body {
        r = r.json(&b);
    }
    let res = r.send().await.map_err(|e| AppError::msg(format!("the crew server didn't answer: {e}")))?;
    let st = res.status();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if !st.is_success() {
        return Err(AppError::msg(v.get("error").and_then(|e| e.as_str()).unwrap_or("the crew server said no").to_string()));
    }
    Ok(v)
}

/// The host side: while someone is watching along, report playback ~1/s.
fn host_tick(core: &Core, known: &Known, meta: &Value, video_id: &str, pos: f64, paused: bool) {
    if !HOSTING.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    {
        let mut last = LAST_ROOM.lock();
        if last.map_or(false, |t| t.elapsed() < Duration::from_millis(900)) {
            return;
        }
        *last = Some(Instant::now());
    }
    let Some((url, code)) = server(core) else { return };
    let body = json!({
        "host": member_id(core),
        "metaId": meta.get("id"),
        "type": meta.get("type"),
        "videoId": video_id,
        "title": known.title,
        "pos": pos,
        "paused": paused,
    });
    let client = core.http.client();
    tauri::async_runtime::spawn(async move {
        if let Ok(r) = client.put(format!("{url}/v1/room")).header("authorization", code).json(&body).timeout(Duration::from_secs(5)).send().await {
            if r.status().as_u16() == 404 {
                HOSTING.store(false, std::sync::atomic::Ordering::Relaxed); // everyone left
            }
        }
    });
}

pub async fn ask(core: &Core, to: &str) -> AppResult<Value> {
    call(core, reqwest::Method::POST, "/v1/ask", Some(json!({ "from": member_id(core), "to": to }))).await
}

pub async fn ask_status(core: &Core, id: &str) -> AppResult<Value> {
    call(core, reqwest::Method::GET, &format!("/v1/ask/{id}"), None).await
}

pub async fn inbox(core: &Core) -> AppResult<Value> {
    if login(core).await.is_err() || server(core).is_none() {
        return Ok(json!({ "asks": [] }));
    }
    let me = member_id(core);
    call(core, reqwest::Method::GET, &format!("/v1/inbox/{me}"), None).await
}

pub async fn answer(core: &Core, id: &str, accept: bool) -> AppResult<Value> {
    let v = call(core, reqwest::Method::POST, &format!("/v1/ask/{id}"), Some(json!({ "me": member_id(core), "accept": accept }))).await?;
    if accept {
        HOSTING.store(true, std::sync::atomic::Ordering::Relaxed);
        *LAST_ROOM.lock() = None;
    }
    Ok(v)
}

pub async fn room(core: &Core, host: &str) -> AppResult<Value> {
    call(core, reqwest::Method::GET, &format!("/v1/room/{host}"), None).await
}

pub async fn leave(core: &Core, host: &str) -> AppResult<Value> {
    let me = member_id(core);
    call(core, reqwest::Method::DELETE, &format!("/v1/room/{host}/{me}"), None).await
}
