//! Your Stremio account: same addons and same Continue Watching as the
//! phone app. Pull merges by `_mtime` (newest wins), push sends only what
//! changed here since the last sync.

use crate::addons::{AddonDescriptor, Addons};
use crate::core::Core;
use crate::error::{AppError, AppResult};
use crate::library::parse_iso;
use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "https://api.strem.io/api";

async fn call(core: &Core, method: &str, body: Value) -> AppResult<Value> {
    let res = core
        .http
        .client()
        .post(format!("{API}/{method}"))
        .timeout(Duration::from_secs(20))
        .json(&body)
        .send()
        .await?;
    let v: Value = res.json().await?;
    if let Some(err) = v.get("error") {
        let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("Stremio API error");
        return Err(AppError::msg(msg.to_string()));
    }
    Ok(v.get("result").cloned().unwrap_or(Value::Null))
}

fn auth(core: &Core) -> AppResult<String> {
    core.db.secret("stremio_auth").ok_or_else(|| AppError::msg("Not signed in to Stremio"))
}

pub async fn login(core: &Core, email: &str, password: &str) -> AppResult<Value> {
    let r = call(core, "login", json!({ "type": "Login", "email": email, "password": password, "facebook": false })).await?;
    let key = r.get("authKey").and_then(|k| k.as_str()).ok_or_else(|| AppError::msg("No auth key in reply"))?;
    core.db.set_secret("stremio_auth", Some(key))?;
    core.db.set("stremio_user", &r.get("user").cloned().unwrap_or(Value::Null))?;
    Ok(r.get("user").cloned().unwrap_or(Value::Null))
}

const LINK: &str = "https://link.stremio.com/api/v2";

/// Sign in from another device (or the browser) -- the way Stremio's TV app
/// does it. Works for Apple, Facebook and email accounts alike: the person
/// approves the code wherever they're already signed in.
pub async fn link_start(core: &Core) -> AppResult<Value> {
    let v: Value = core.http.client().get(format!("{LINK}/create?type=Create")).timeout(Duration::from_secs(15)).send().await?.json().await?;
    let r = v.get("result").cloned().ok_or_else(|| AppError::msg("Stremio didn't return a code"))?;
    Ok(json!({ "code": r["code"], "link": r["link"], "qrcode": r["qrcode"] }))
}

/// `true` once the code was approved (and the account is stored).
pub async fn link_poll(core: &Core, code: &str) -> AppResult<bool> {
    let v: Value = core
        .http
        .client()
        .get(format!("{LINK}/read?type=Read&code={}", urlencode(code)))
        .timeout(Duration::from_secs(15))
        .send()
        .await?
        .json()
        .await?;
    let Some(key) = v.pointer("/result/authKey").and_then(|k| k.as_str()) else {
        return Ok(false);
    };
    core.db.set_secret("stremio_auth", Some(key))?;
    let user = call(core, "getUser", json!({ "type": "GetUser", "authKey": key })).await.unwrap_or(Value::Null);
    core.db.set("stremio_user", &user)?;
    Ok(true)
}

fn urlencode(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

pub fn logout(core: &Core) -> AppResult<()> {
    core.db.set_secret("stremio_auth", None)?;
    core.db.del("stremio_user")
}

pub async fn pull_addons(core: &Core, addons: &Addons) -> AppResult<usize> {
    let key = auth(core)?;
    let r = call(core, "addonCollectionGet", json!({ "type": "AddonCollectionGet", "authKey": key, "update": true })).await?;
    let list: Vec<AddonDescriptor> = r
        .get("addons")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|a| serde_json::from_value(a).ok())
        .collect();
    let n = list.len();
    if n > 0 {
        addons.replace_all(list);
    }
    Ok(n)
}

pub async fn push_addons(core: &Core, addons: &Addons) -> AppResult<()> {
    let key = auth(core)?;
    call(core, "addonCollectionSet", json!({ "type": "AddonCollectionSet", "authKey": key, "addons": addons.list() })).await?;
    Ok(())
}

pub async fn sync_library(core: &Core) -> AppResult<(usize, usize)> {
    let key = auth(core)?;
    // Push local changes first so a pull can't overwrite them.
    let dirty = core.db.library_dirty()?;
    let mut pushed = 0;
    if !dirty.is_empty() {
        // Lumen-only fields (watchedIds, dismissed) ride along harmlessly:
        // the API stores the object as given.
        call(core, "datastorePut", json!({ "authKey": key, "collection": "libraryItem", "changes": dirty })).await?;
        let ids: Vec<String> = dirty.iter().filter_map(|i| i["_id"].as_str().map(|s| s.to_string())).collect();
        pushed = ids.len();
        core.db.library_mark_clean(&ids)?;
    }
    let remote = call(core, "datastoreGet", json!({ "authKey": key, "collection": "libraryItem", "ids": [], "all": true })).await?;
    let mut pulled = 0;
    for item in remote.as_array().into_iter().flatten() {
        let Some(id) = item["_id"].as_str() else { continue };
        let rm = parse_iso(item["_mtime"].as_str().unwrap_or(""));
        let local_m = core.db.library_get(id).map(|l| parse_iso(l["_mtime"].as_str().unwrap_or(""))).unwrap_or(0);
        if rm > local_m {
            core.db.library_put(id, item, rm, false)?;
            pulled += 1;
        }
    }
    Ok((pulled, pushed))
}
