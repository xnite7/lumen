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
    (exp > now + 25).then(|| (CREW_URL.trim_end_matches('/').to_string(), format!("Bearer {session}")))
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
        let _ = core.db.set_secret("crew_session", None);
        // the server needs to see your roles: sign in again ("one more knock")
        if v.get("reauth").and_then(|r| r.as_bool()) == Some(true) {
            let _ = core.db.set("discord_scopes", &json!(""));
            core.emit("discord-gate", "signin");
            return Err(AppError::msg("sign in with Discord again"));
        }
        let _ = core.db.set("discord_member", &json!(false));
        core.emit("discord-gate", "outsider"); // kicked / no access role: lock the app now
        return Err(AppError::msg("no access to couch"));
    }
    let session = v.get("session").and_then(|s| s.as_str()).ok_or_else(|| AppError::msg(v.get("error").and_then(|e| e.as_str()).unwrap_or("the crew server said no").to_string()))?;
    core.db.set_secret("crew_session", Some(session))?;
    core.db.set("crew_session_exp", &json!(v.get("exp").and_then(|e| e.as_u64()).unwrap_or(0)))?;
    Ok(())
}

/// Does the crew server let this Discord sign-in in (right role)? None = couldn't tell.
pub async fn access(core: &Core, tok: &str) -> Option<bool> {
    let r = core
        .http
        .client()
        .post(format!("{}/v1/auth", CREW_URL.trim_end_matches('/')))
        .json(&json!({ "token": tok }))
        .timeout(Duration::from_secs(12))
        .send()
        .await
        .ok()?;
    match r.status().as_u16() {
        200 => {
            if let Ok(v) = r.json::<Value>().await {
                if let Some(s) = v.get("session").and_then(|s| s.as_str()) {
                    let _ = core.db.set_secret("crew_session", Some(s));
                    let _ = core.db.set("crew_session_exp", &json!(v.get("exp").and_then(|e| e.as_u64()).unwrap_or(0)));
                }
            }
            Some(true)
        }
        403 => Some(false),
        _ => None,
    }
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
    // the session lapses every couple of minutes: earn a new one quietly
    if server(core).is_none() {
        static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !BUSY.swap(true, std::sync::atomic::Ordering::Relaxed) {
            let h = core.app.clone();
            tauri::async_runtime::spawn(async move {
                use tauri::Manager;
                let c = h.state::<crate::state::AppState>().core.clone();
                let _ = login(&c).await;
                BUSY.store(false, std::sync::atomic::Ordering::Relaxed);
            });
        }
        return;
    }
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
    let (u2, c2) = (url.clone(), code.clone());
    tauri::async_runtime::spawn(async move {
        let _ = client.put(format!("{url}/v1/presence")).header("authorization", code).json(&body).timeout(Duration::from_secs(8)).send().await;
    });
    if !gate && !paused {
        send_frame(core, u2, c2);
    }
}

static LAST_FRAME: Mutex<Option<Instant>> = Mutex::new(None);

/// About once a minute while playing: a small still of the current frame,
/// so the couch row shows the scene instead of the poster.
fn send_frame(core: &Core, url: String, code: String) {
    {
        let mut last = LAST_FRAME.lock();
        if last.map_or(false, |t| t.elapsed() < Duration::from_secs(60)) {
            return;
        }
        *last = Some(Instant::now());
    }
    let app = core.app.clone();
    let client = core.http.client();
    tauri::async_runtime::spawn(async move {
        use tauri::Manager;
        let path = std::env::temp_dir().join("couch-frame.jpg");
        let _ = std::fs::remove_file(&path);
        let player = app.state::<crate::state::AppState>().player.clone();
        if player.command("snap", &json!({ "path": path.to_string_lossy() })).is_err() {
            return;
        }
        let img = tauri::async_runtime::spawn_blocking(move || -> Option<String> {
            // mpv writes the file a moment after the command returns
            for _ in 0..20 {
                if path.metadata().map_or(false, |m| m.len() > 0) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            std::thread::sleep(Duration::from_millis(150));
            let im = image::open(&path).ok()?.thumbnail(560, 315).to_rgb8();
            let mut out = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 70).encode_image(&im).ok()?;
            let _ = std::fs::remove_file(&path);
            use base64::Engine;
            Some(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(out)))
        })
        .await
        .ok()
        .flatten();
        if let Some(img) = img {
            let _ = client.put(format!("{url}/v1/frame")).header("authorization", code).json(&json!({ "img": img })).timeout(Duration::from_secs(10)).send().await;
        }
    });
}

/// Playback ended: leave the couch row.
pub fn stopped(core: &Core) {
    *LAST.lock() = None;
    SHARED.store(false, std::sync::atomic::Ordering::Relaxed);
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
/// the host lets everyone in the room pause / play / seek
pub static SHARED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
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
        "shared": SHARED.load(std::sync::atomic::Ordering::Relaxed),
    });
    let client = core.http.client();
    let app = core.app.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(r) = client.put(format!("{url}/v1/room")).header("authorization", code).json(&body).timeout(Duration::from_secs(5)).send().await {
            if r.status().as_u16() == 404 {
                HOSTING.store(false, std::sync::atomic::Ordering::Relaxed); // everyone left
                return;
            }
            // guests steering (shared control): the player does what they asked
            if let Ok(v) = r.json::<Value>().await {
                use tauri::Emitter;
                for c in v.get("cmds").and_then(|c| c.as_array()).into_iter().flatten() {
                    let _ = app.emit("together-cmd", c);
                }
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

pub async fn room_cmd(core: &Core, host: &str, action: &str, pos: Option<f64>) -> AppResult<Value> {
    call(core, reqwest::Method::POST, &format!("/v1/room/{host}/cmd"), Some(json!({ "action": action, "pos": pos }))).await
}

pub async fn cancel_ask(core: &Core, id: &str) -> AppResult<Value> {
    call(core, reqwest::Method::DELETE, &format!("/v1/ask/{id}"), None).await
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

/// A ClipCafe scene for this title (billboard / previews), as a playable URL.
pub async fn clip_url(core: &Core, id: &str, title: Option<&str>, year: Option<&str>) -> Option<String> {
    let imdb = id.split(':').next().filter(|s| s.starts_with("tt"))?;
    login(core).await.ok()?;
    // title + year: ClipCafe sometimes files a film under another IMDb id
    let mut q = String::new();
    if let Some(t) = title.filter(|t| !t.is_empty()) {
        q.push_str(&format!("title={}", urlencoding::encode(t)));
        if let Some(y) = year.filter(|y| y.len() == 4) {
            q.push_str(&format!("&year={y}"));
        }
    }
    let ask = if q.is_empty() { format!("/v1/clip/{imdb}") } else { format!("/v1/clip/{imdb}?{q}") };
    let v = call(core, reqwest::Method::GET, &ask, None).await.ok()?;
    if v.get("clip").and_then(|c| c.as_bool()) != Some(true) {
        return None;
    }
    let (url, auth) = server(core)?;
    let session = auth.trim_start_matches("Bearer ").to_string();
    Some(format!("{url}/clip/{imdb}?t={}{}{}", urlencoding::encode(&session), if q.is_empty() { "" } else { "&" }, q))
}

pub async fn social(core: &Core, method: &str, path: &str, body: Option<Value>) -> AppResult<Value> {
    let ok = ["/v1/favs", "/v1/activity", "/v1/people", "/v1/profile"].contains(&path)
        || path.starts_with("/v1/reviews/")
        || path.starts_with("/v1/react/")
        || path.starts_with("/v1/frame/")
        || path.starts_with("/v1/profile/");
    if !ok || path.contains("..") {
        return Err(AppError::msg("not a social path"));
    }
    login(core).await?;
    let m = match method {
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        _ => reqwest::Method::GET,
    };
    call(core, m, path, body).await
}

pub async fn room(core: &Core, host: &str) -> AppResult<Value> {
    call(core, reqwest::Method::GET, &format!("/v1/room/{host}"), None).await
}

pub async fn leave(core: &Core, host: &str) -> AppResult<Value> {
    let me = member_id(core);
    call(core, reqwest::Method::DELETE, &format!("/v1/room/{host}/{me}"), None).await
}


// ------------------------------------------------------------ profiles
static LAST_PROFILE: parking_lot::Mutex<Option<(u64, std::time::Instant)>> = parking_lot::Mutex::new(None);

fn slim(m: &Value, at_key: &str) -> Value {
    let at = m.get(at_key).and_then(|v| v.as_str()).map(crate::library::parse_iso).filter(|t| *t > 0);
    json!({
        "id": m.get("id"), "type": m.get("type"), "name": m.get("name"),
        "poster": m.get("poster"), "background": m.get("background"), "logo": m.get("logo"),
        "rating": m.get("userRating"), "at": at, "eps": m.get("episodes"),
    })
}

/// What my page on the couch shows: from Simkl when connected, otherwise
/// from what I've watched in couch.
fn profile_snapshot(core: &Core) -> Value {
    let l = crate::commands::lists_value(core);
    let newest = |key: &str, at: &str| -> Vec<Value> {
        let mut v: Vec<Value> = l.get(key).and_then(|a| a.as_array()).cloned().unwrap_or_default();
        v.sort_by_key(|m| -m.get(at).and_then(|x| x.as_str()).map(crate::library::parse_iso).unwrap_or(0));
        v.iter().take(30).map(|m| slim(m, at)).collect()
    };
    if l.get("simkl").and_then(|b| b.as_bool()) == Some(true) {
        let count = |key: &str, kind: &str| l.get(key).and_then(|a| a.as_array()).map_or(0, |a| a.iter().filter(|m| m["type"] == kind).count());
        let episodes: i64 = ["watching", "completed"]
            .iter()
            .filter_map(|k| l.get(*k).and_then(|a| a.as_array()))
            .flatten()
            .filter_map(|m| m.pointer("/episodes/0").and_then(|v| v.as_i64()))
            .sum();
        return json!({
            "source": "simkl",
            "watching": newest("watching", "watchedAt"),
            "recent": newest("completed", "watchedAt"),
            "plan": newest("plan", "addedAt"),
            "stats": {
                "movies": count("completed", "movie"),
                "shows": count("completed", "series"),
                "episodes": episodes,
                "watching": l.get("watching").and_then(|a| a.as_array()).map_or(0, |a| a.len()),
                "plan": l.get("plan").and_then(|a| a.as_array()).map_or(0, |a| a.len()),
            },
        });
    }
    // no Simkl: sort the couch library the same way My Lists does
    let items = crate::library::all(core).unwrap_or_default();
    let (mut watching, mut recent, mut plan) = (vec![], vec![], vec![]);
    for i in &items {
        let st = &i["state"];
        let removed = i["removed"].as_bool().unwrap_or(false);
        let done = st["flaggedWatched"].as_i64().unwrap_or(0) > 0 || st["timesWatched"].as_i64().unwrap_or(0) > 0;
        if removed && !done {
            continue;
        }
        let going = !done && (st["timeOffset"].as_i64().unwrap_or(0) > 0 || st["watchedIds"].as_array().map_or(false, |a| !a.is_empty()));
        if i["temp"].as_bool().unwrap_or(false) && !done && !going {
            continue;
        }
        let at = crate::library::parse_iso(st["lastWatched"].as_str().or(i["_mtime"].as_str()).unwrap_or(""));
        let eps = st["watchedIds"].as_array().map(|a| a.len());
        let row = json!({ "id": i["_id"], "type": i["type"], "name": i["name"], "poster": i["poster"], "background": i["background"], "logo": i["logo"], "at": at, "eps": eps.map(|n| json!([n, 0])) });
        if done { recent.push(row) } else if going { watching.push(row) } else { plan.push(row) }
    }
    for v in [&mut watching, &mut recent, &mut plan] {
        v.sort_by_key(|m| -m["at"].as_i64().unwrap_or(0));
    }
    let movies = recent.iter().filter(|m| m["type"] == "movie").count();
    let shows = recent.len() - movies;
    let stats = json!({ "movies": movies, "shows": shows, "episodes": 0, "watching": watching.len(), "plan": plan.len() });
    watching.truncate(30);
    recent.truncate(30);
    plan.truncate(30);
    json!({ "source": "couch", "watching": watching, "recent": recent, "plan": plan, "stats": stats })
}

/// Send my profile to the crew when it changed (or `force`), at most every
/// few minutes otherwise.
pub async fn publish_profile(core: &Core, force: bool) {
    if server(core).is_none() && login(core).await.is_err() {
        return;
    }
    let snap = profile_snapshot(core);
    let hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        snap.to_string().hash(&mut h);
        h.finish()
    };
    if !force {
        if let Some((last, when)) = *LAST_PROFILE.lock() {
            if last == hash && when.elapsed() < Duration::from_secs(6 * 3600) {
                return;
            }
        }
    }
    if social(core, "PUT", "/v1/profile", Some(snap)).await.is_ok() {
        *LAST_PROFILE.lock() = Some((hash, std::time::Instant::now()));
    }
}
