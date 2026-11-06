//! The midnite crew: "on the couch" presence shared with friends through a
//! tiny Cloudflare Worker (see /worker).
//!
//! Same rule as Discord: only titles Simkl knows are ever shared, using
//! Simkl's own title and poster -- never a file name. Sharing is opt-in
//! (Settings → The crew), and the server forgets you two minutes after the
//! last heartbeat, so closing the app is enough to disappear.

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

/// This install's stable member id (random, made on first use).
pub fn member_id(core: &Core) -> String {
    if let Some(id) = core.db.get::<String>("crew_id").filter(|s| !s.is_empty()) {
        return id;
    }
    use rand::Rng;
    let id: String = rand::rng().sample_iter(rand::distr::Alphanumeric).take(16).map(char::from).collect();
    let _ = core.db.set("crew_id", &id);
    id
}

/// (url, code) when the crew is set up.
fn server(core: &Core) -> Option<(String, String)> {
    let url = core.settings.read().integrations.crew_url.trim().trim_end_matches('/').to_string();
    let code = core.db.secret("crew_code")?;
    (!url.is_empty()).then_some((url, code))
}

fn name(core: &Core) -> String {
    let n = core.settings.read().integrations.crew_name.trim().to_string();
    if !n.is_empty() {
        return n;
    }
    if let Some(n) = core.db.get::<Value>("discord_user").and_then(|u| u.get("name").and_then(|v| v.as_str()).map(|s| s.to_string())) {
        return n;
    }
    core.db
        .get::<Value>("stremio_user")
        .and_then(|u| u.get("fullname").or_else(|| u.get("email")).and_then(|v| v.as_str()).map(|s| s.split('@').next().unwrap_or(s).to_string()))
        .unwrap_or_else(|| "someone".into())
}

/// Called from the player tracker on every tick: sends when something
/// changed, otherwise a heartbeat every 45 s.
pub fn playing(core: &Core, known: &Known, meta: &Value, pos: f64, duration: f64, paused: bool) {
    if !core.settings.read().integrations.crew_share {
        return;
    }
    let Some((url, code)) = server(core) else { return };
    let key = format!("{}|{}|{}", known.title, known.episode.clone().unwrap_or_default(), paused);
    {
        let mut last = LAST.lock();
        if let Some(l) = last.as_ref() {
            if l.key == key && l.at.elapsed() < HEARTBEAT {
                return;
            }
        }
        *last = Some(Sent { at: Instant::now(), key });
    }
    let body = json!({
        "id": member_id(core),
        "name": name(core),
        "watching": {
            "title": known.title,
            "year": known.year,
            "poster": known.poster,
            "episode": known.episode,
            "link": known.link,
            "metaId": meta.get("id"),
            "type": meta.get("type"),
            "background": meta.get("background"),
        },
        "paused": paused,
        "pos": pos,
        "duration": duration,
    });
    let client = core.http.client();
    tauri::async_runtime::spawn(async move {
        let _ = client.put(format!("{url}/v1/presence")).header("x-crew-code", code).json(&body).timeout(Duration::from_secs(8)).send().await;
    });
}

/// Playback ended: leave the couch row.
pub fn stopped(core: &Core) {
    *LAST.lock() = None;
    let Some((url, code)) = server(core) else { return };
    let id = member_id(core);
    let client = core.http.client();
    tauri::async_runtime::spawn(async move {
        let _ = client.delete(format!("{url}/v1/presence/{id}")).header("x-crew-code", code).timeout(Duration::from_secs(8)).send().await;
    });
}

/// Who's on the couch right now (for the Home row and the Couch tab).
pub async fn list(core: &Core) -> AppResult<Value> {
    let Some((url, code)) = server(core) else { return Ok(json!({ "setup": false, "members": [] })) };
    let me = member_id(core);
    let r = core
        .http
        .client()
        .get(format!("{url}/v1/presence"))
        .header("x-crew-code", code)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| AppError::msg(format!("the crew server didn't answer: {e}")))?;
    if r.status().as_u16() == 401 {
        return Err(AppError::msg("that crew code doesn't work"));
    }
    let v: Value = r.json().await.map_err(|e| AppError::msg(e.to_string()))?;
    let members: Vec<Value> = v.get("members").and_then(|m| m.as_array()).cloned().unwrap_or_default().into_iter().filter(|m| m.get("id").and_then(|i| i.as_str()) != Some(me.as_str())).collect();
    Ok(json!({ "setup": true, "members": members }))
}
