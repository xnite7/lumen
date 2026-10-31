//! Simkl: sign-in (AUTH V2 OAuth + device code, with the old V1 PIN flow
//! still honoured for existing tokens), lists, ratings, history, scrobbling.
//!
//! Simkl wants the client id as a query parameter AND the `simkl-api-key`
//! header; with only the header it answers 401 even on a valid token (found
//! the hard way in the worker). Both are always sent.

use crate::core::Core;
use crate::error::{AppError, AppResult};
use serde_json::{json, Value};
use std::time::Duration;

pub const API: &str = "https://api.simkl.com";
const APP: &str = "lumen";

/// Lumen's AUTH V2 app ("Mobile, desktop & browser apps"). Client ids are
/// public by design; PKCE protects the sign-in. Overridable in secrets.
pub const V2_CLIENT_ID: &str = "9c325049149440f9e66dc4499c37fc140ba6962583a3fa7927d8e27e3bd68cfc";
const SCOPE: &str = "media:read media:write";

fn v2_id(core: &Core) -> String {
    core.db.secret("simkl_v2_client_id").unwrap_or_else(|| V2_CLIENT_ID.to_string())
}

fn v2(core: &Core) -> bool {
    core.db.secret("simkl_v2_access").is_some()
}

/// The client id that matches the token in use (V2 wins once signed in).
pub fn client_id(core: &Core) -> Option<String> {
    if v2(core) {
        Some(v2_id(core))
    } else {
        core.db.secret("simkl_client_id")
    }
}

pub fn token(core: &Core) -> Option<String> {
    core.db.secret("simkl_v2_access").or_else(|| core.db.secret("simkl_token"))
}

pub fn connected(core: &Core) -> bool {
    client_id(core).is_some() && token(core).is_some() && core.settings.read().integrations.simkl_enabled
}

fn url(core: &Core, path: &str) -> AppResult<String> {
    let cid = client_id(core).ok_or_else(|| AppError::msg("Simkl client id missing"))?;
    let sep = if path.contains('?') { '&' } else { '?' };
    Ok(format!("{API}{path}{sep}client_id={}&app-name={APP}&app-version=1.0", urlencoding::encode(&cid)))
}

fn req(core: &Core, method: reqwest::Method, path: &str) -> AppResult<reqwest::RequestBuilder> {
    let cid = client_id(core).ok_or_else(|| AppError::msg("Simkl client id missing"))?;
    let mut b = core
        .http
        .client()
        .request(method, url(core, path)?)
        .header("simkl-api-key", cid)
        .header("User-Agent", "Lumen/1.0")
        .header("Content-Type", "application/json")
        .timeout(Duration::from_secs(12));
    if let Some(t) = token(core) {
        b = b.bearer_auth(t);
    }
    Ok(b)
}

pub async fn get(core: &Core, path: &str) -> AppResult<Value> {
    ensure_fresh(core).await;
    let res = req(core, reqwest::Method::GET, path)?.send().await?;
    if !res.status().is_success() {
        return Err(AppError::msg(format!("Simkl HTTP {} on {path}", res.status())));
    }
    Ok(res.json().await?)
}

pub async fn delete(core: &Core, path: &str) -> AppResult<()> {
    ensure_fresh(core).await;
    req(core, reqwest::Method::DELETE, path)?.send().await?;
    Ok(())
}

pub async fn post(core: &Core, path: &str, body: Value) -> AppResult<Value> {
    ensure_fresh(core).await;
    let res = req(core, reqwest::Method::POST, path)?.json(&body).send().await?;
    let status = res.status();
    let v = res.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(AppError::msg(format!("Simkl HTTP {status} on {path}")));
    }
    Ok(v)
}

// ------------------------------------------------------------ sign in

/// Step 1 of the device flow: a short code to type at simkl.com/pin.
pub async fn pin_start(core: &Core) -> AppResult<Value> {
    let cid = client_id(core).ok_or_else(|| AppError::msg("Paste your Simkl client id first"))?;
    let u = format!("{API}/oauth/pin?client_id={}&redirect=", urlencoding::encode(&cid));
    let v: Value = core.http.get_json(&u, Duration::from_secs(10)).await?;
    Ok(v)
}

/// Step 2: poll until the user has approved. Returns true once signed in.
pub async fn pin_poll(core: &Core, user_code: &str) -> AppResult<bool> {
    let cid = client_id(core).ok_or_else(|| AppError::msg("Simkl client id missing"))?;
    let u = format!("{API}/oauth/pin/{user_code}?client_id={}", urlencoding::encode(&cid));
    let v: Value = core.http.get_json(&u, Duration::from_secs(10)).await?;
    if let Some(t) = v.get("access_token").and_then(|t| t.as_str()) {
        core.db.set_secret("simkl_token", Some(t))?;
        if let Ok(me) = get(core, "/users/settings").await {
            let _ = core.db.set("simkl_user", &me);
        }
        return Ok(true);
    }
    Ok(false)
}

// ------------------------------------------------------- AUTH V2

use base64::Engine as _;
use sha2::Digest as _;
use std::sync::Mutex;

struct Pending {
    verifier: String,
    state: String,
    redirect: String,
}
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static DEVICE: Mutex<Option<String>> = Mutex::new(None);

fn random_token(len: usize) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut r = rand::rng();
    (0..len).map(|_| CHARS[r.random_range(0..CHARS.len())] as char).collect()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Step 1 of the browser sign-in: the consent page URL (PKCE S256).
pub fn oauth_start(core: &Core, redirect: &str) -> String {
    let verifier = random_token(64);
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
    let state = random_token(24);
    let url = format!(
        "https://simkl.com/oauth2/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        urlencoding::encode(&v2_id(core)),
        urlencoding::encode(redirect),
        urlencoding::encode(SCOPE),
        state,
        challenge
    );
    *PENDING.lock().unwrap() = Some(Pending { verifier, state, redirect: redirect.to_string() });
    url
}

/// Step 2: the redirect came back (`...?code=..&state=..`); swap the code
/// for tokens.
pub async fn oauth_finish(core: &Core, callback: &str) -> AppResult<()> {
    let query = callback.split_once('?').map(|(_, q)| q).unwrap_or("");
    let params: std::collections::HashMap<String, String> = query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), urlencoding::decode(&v.replace('+', " ")).map(|c| c.into_owned()).unwrap_or_default()))
        .collect();
    if let Some(e) = params.get("error") {
        return Err(AppError::msg(if e == "access_denied" { "Simkl sign-in was cancelled".to_string() } else { format!("Simkl sign-in failed: {e}") }));
    }
    let code = params.get("code").ok_or_else(|| AppError::msg("Simkl didn't send a sign-in code back"))?;
    let pending = PENDING.lock().unwrap().take().ok_or_else(|| AppError::msg("No Simkl sign-in in progress. Start again."))?;
    if params.get("state") != Some(&pending.state) {
        return Err(AppError::msg("That Simkl sign-in link doesn't match. Start again."));
    }
    let v = token_request(
        core,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &v2_id(core)),
            ("code", code),
            ("redirect_uri", &pending.redirect),
            ("code_verifier", &pending.verifier),
        ],
    )
    .await?;
    store_tokens(core, &v).await
}

/// Backup: a short code to approve on any device (RFC 8628).
pub async fn device_start(core: &Core) -> AppResult<Value> {
    let res = core
        .http
        .client()
        .post(format!("{API}/oauth2/device"))
        .header("User-Agent", "Lumen/1.0")
        .form(&[("client_id", v2_id(core).as_str()), ("scope", SCOPE)])
        .timeout(Duration::from_secs(12))
        .send()
        .await?;
    let v: Value = res.json().await?;
    let dc = v.get("device_code").and_then(|d| d.as_str()).ok_or_else(|| AppError::msg(v.get("error_description").and_then(|e| e.as_str()).unwrap_or("Simkl didn't return a code").to_string()))?;
    *DEVICE.lock().unwrap() = Some(dc.to_string());
    // never hand the device code itself to the page
    Ok(json!({
        "user_code": v["user_code"],
        "verification_url": v.get("verification_uri_complete").or_else(|| v.get("verification_uri")),
        "interval": v["interval"],
        "expires_in": v["expires_in"],
    }))
}

/// Poll the device code. `true` once approved and stored.
pub async fn device_poll(core: &Core) -> AppResult<bool> {
    let Some(dc) = DEVICE.lock().unwrap().clone() else { return Ok(false) };
    match token_request(core, &[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("client_id", &v2_id(core)), ("device_code", &dc)]).await {
        Ok(v) => {
            *DEVICE.lock().unwrap() = None;
            store_tokens(core, &v).await?;
            Ok(true)
        }
        Err(e) => {
            let m = e.to_string();
            if m.contains("authorization_pending") || m.contains("slow_down") {
                Ok(false)
            } else {
                *DEVICE.lock().unwrap() = None;
                Err(e)
            }
        }
    }
}

async fn token_request(core: &Core, form: &[(&str, &str)]) -> AppResult<Value> {
    let res = core
        .http
        .client()
        .post(format!("{API}/oauth2/token"))
        .header("User-Agent", "Lumen/1.0")
        .form(form)
        .timeout(Duration::from_secs(15))
        .send()
        .await?;
    let status = res.status();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() || v.get("access_token").is_none() {
        let e = v.get("error").and_then(|e| e.as_str()).unwrap_or("error");
        let d = v.get("error_description").and_then(|e| e.as_str()).unwrap_or("");
        return Err(AppError::msg(format!("{e}: {d}")));
    }
    Ok(v)
}

async fn store_tokens(core: &Core, v: &Value) -> AppResult<()> {
    // a new sign-in may be a different account: rebuild the mirror from scratch
    super::simkl_sync::clear(core);
    let access = v.get("access_token").and_then(|t| t.as_str()).ok_or_else(|| AppError::msg("No access token"))?;
    core.db.set_secret("simkl_v2_access", Some(access))?;
    if let Some(r) = v.get("refresh_token").and_then(|t| t.as_str()) {
        core.db.set_secret("simkl_v2_refresh", Some(r))?;
    }
    let ttl = v.get("expires_in").and_then(|e| e.as_u64()).unwrap_or(7 * 86400);
    core.db.set("simkl_v2_expires", &json!(now_secs() + ttl))?;
    if let Ok(me) = get(core, "/users/settings").await {
        let _ = core.db.set("simkl_user", &me);
    }
    Ok(())
}

/// Access tokens last 7 days; refresh a day early so a request never fails.
async fn ensure_fresh(core: &Core) {
    if !v2(core) {
        return;
    }
    let exp = core.db.get::<Value>("simkl_v2_expires").and_then(|v| v.as_u64()).unwrap_or(0);
    if exp > now_secs() + 86400 {
        return;
    }
    let Some(rt) = core.db.secret("simkl_v2_refresh") else { return };
    match token_request(core, &[("grant_type", "refresh_token"), ("client_id", &v2_id(core)), ("refresh_token", &rt)]).await {
        Ok(v) => {
            let _ = store_tokens_quiet(core, &v);
        }
        Err(e) => log::warn!("simkl refresh: {e}"),
    }
}

fn store_tokens_quiet(core: &Core, v: &Value) -> AppResult<()> {
    if let Some(a) = v.get("access_token").and_then(|t| t.as_str()) {
        core.db.set_secret("simkl_v2_access", Some(a))?;
    }
    let ttl = v.get("expires_in").and_then(|e| e.as_u64()).unwrap_or(7 * 86400);
    core.db.set("simkl_v2_expires", &json!(now_secs() + ttl))
}

pub async fn logout(core: &Core) -> AppResult<()> {
    if let Some(rt) = core.db.secret("simkl_v2_refresh") {
        let _ = core
            .http
            .client()
            .post(format!("{API}/oauth2/revoke"))
            .header("User-Agent", "Lumen/1.0")
            .form(&[("client_id", v2_id(core).as_str()), ("token", rt.as_str())])
            .timeout(Duration::from_secs(8))
            .send()
            .await;
    }
    for k in ["simkl_v2_access", "simkl_v2_refresh", "simkl_token"] {
        core.db.set_secret(k, None)?;
    }
    super::simkl_sync::clear(core);
    core.db.del("simkl_v2_expires")?;
    core.db.del("simkl_user")
}

// -------------------------------------------------------------- ids

/// Simkl wraps list entries as {show|movie|anime: {...}}.
pub fn inner(e: &Value) -> &Value {
    e.get("show").or_else(|| e.get("movie")).or_else(|| e.get("anime")).unwrap_or(e)
}

pub fn is_show(e: &Value) -> bool {
    e.get("show").is_some()
        || e.get("anime").map_or(false, |a| a.get("anime_type").and_then(|t| t.as_str()) != Some("movie"))
        || matches!(inner(e).get("type").and_then(|t| t.as_str()), Some("tv") | Some("anime"))
}

pub fn metahub(imdb: &str, kind: &str) -> String {
    format!("https://images.metahub.space/{kind}/medium/{imdb}/img")
}

pub fn to_meta(e: &Value, want: Option<&str>) -> Option<Value> {
    let it = inner(e);
    let imdb = it.pointer("/ids/imdb").and_then(|v| v.as_str()).filter(|s| s.starts_with("tt"))?;
    let kind = if is_show(e) { "series" } else { "movie" };
    if want.map_or(false, |w| w != kind) {
        return None;
    }
    Some(json!({
        "id": imdb,
        "type": kind,
        "name": it.get("title").and_then(|t| t.as_str()).unwrap_or(imdb),
        "poster": metahub(imdb, "poster"),
        "posterShape": "poster",
        "background": metahub(imdb, "background"),
        "logo": metahub(imdb, "logo"),
        "releaseInfo": it.get("year").map(|y| y.to_string()),
        "description": it.get("overview"),
        "genres": it.get("genres"),
        "imdbRating": it.pointer("/ratings/imdb/rating").map(|r| r.to_string()),
        "runtime": it.get("runtime").and_then(|r| r.as_u64()).map(|r| format!("{r} min")),
        "userRating": e.get("user_rating"),
    }))
}

/// The body Simkl's sync endpoints want for one title (and optionally one
/// episode). Anime ids (kitsu/mal/anilist) are passed through when the id
/// is one of those, so anime from Kitsu-keyed addons scrobbles too.
pub fn item_body(kind: &str, id: &str, season: Option<u32>, episode: Option<u32>) -> Value {
    let ids = if let Some(n) = id.strip_prefix("kitsu:") {
        json!({ "kitsu": n.split(':').next().unwrap_or(n) })
    } else if let Some(n) = id.strip_prefix("mal:") {
        json!({ "mal": n.split(':').next().unwrap_or(n) })
    } else if let Some(n) = id.strip_prefix("anilist:") {
        json!({ "anilist": n.split(':').next().unwrap_or(n) })
    } else {
        json!({ "imdb": id.split(':').next().unwrap_or(id) })
    };
    if kind == "series" {
        match (season, episode) {
            (Some(s), Some(e)) => json!({ "show": { "ids": ids }, "episode": { "season": s, "number": e } }),
            _ => json!({ "show": { "ids": ids } }),
        }
    } else {
        json!({ "movie": { "ids": ids } })
    }
}

fn wrap_list(kind: &str, id: &str, extra: Value) -> Value {
    let ids = item_body("movie", id, None, None)["movie"]["ids"].clone();
    let mut entry = json!({ "ids": ids });
    if let (Some(e), Some(o)) = (entry.as_object_mut(), extra.as_object()) {
        for (k, v) in o {
            e.insert(k.clone(), v.clone());
        }
    }
    if kind == "series" {
        json!({ "shows": [entry] })
    } else {
        json!({ "movies": [entry] })
    }
}

// ------------------------------------------------------------ actions

/// Scrobble one player event (start / pause / stop), the way Simkl's guide
/// wants it: every identifier we have plus title and year, `min_progress` set
/// to the user's own "count as watched" threshold on every call so all of
/// Simkl's checks agree with Lumen's, and `allow_rewatch` only when the user
/// turned rewatch logging on and the title is already completed.
pub async fn scrobble(core: &Core, action: &str, meta: &Value, video_id: &str, progress: f64, ident: Option<&Value>) -> AppResult<Value> {
    let (s, th) = {
        let st = core.settings.read();
        (st.integrations.clone(), st.playback.watched_threshold)
    };
    if !connected(core) || !s.simkl_scrobble {
        return Ok(Value::Null);
    }
    let kind = meta.get("type").and_then(|t| t.as_str()).unwrap_or("movie");
    let parts: Vec<&str> = video_id.split(':').collect();
    let (season, episode) = if kind == "series" && parts.len() >= 3 {
        (parts[parts.len() - 2].parse().ok(), parts[parts.len() - 1].parse().ok())
    } else {
        (None, None)
    };
    let mut body = match ident {
        // a raw link/magnet identified by its file name (/search/file)
        Some(found) => found.clone(),
        None => item_body(kind, video_id, season, episode),
    };
    if ident.is_none() {
        let key = if kind == "series" { "show" } else { "movie" };
        if let Some(obj) = body.get_mut(key).and_then(|o| o.as_object_mut()) {
            if let Some(t) = meta.get("name").and_then(|n| n.as_str()) {
                obj.insert("title".into(), json!(t));
            }
            if let Some(y) = meta.get("releaseInfo").and_then(|y| y.as_str()).and_then(|y| y.get(0..4)).and_then(|y| y.parse::<u32>().ok()) {
                obj.insert("year".into(), json!(y));
            }
            if let (Some(ids), Some(tmdb)) = (obj.get_mut("ids").and_then(|i| i.as_object_mut()), meta.get("tmdbId").and_then(|t| t.as_u64())) {
                ids.insert("tmdb".into(), json!(tmdb));
            }
        }
    }
    body["progress"] = json!(((progress * 100.0).clamp(0.0, 100.0) * 100.0).round() / 100.0);
    let mut path = format!("/scrobble/{action}?min_progress={}", th.clamp(50, 100));
    if action == "stop" && s.simkl_rewatches && progress * 100.0 >= th as f64 {
        let imdb = video_id.split(':').next().unwrap_or(video_id);
        if super::simkl_sync::entry(core, imdb).map_or(false, |e| e["status"] == "completed") {
            path.push_str("&allow_rewatch=yes");
        }
    }
    post(core, &path, body).await
}

/// Identify a raw link or magnet from its file name, for scrobbling.
pub async fn identify_file(core: &Core, filename: &str) -> Option<Value> {
    if !connected(core) || filename.trim().is_empty() {
        return None;
    }
    let v = post(core, "/search/file", json!({ "file": filename })).await.ok()?;
    if let Some(m) = v.get("movie").filter(|m| !m.is_null()) {
        return Some(json!({ "movie": { "ids": m.get("ids"), "title": m.get("title"), "year": m.get("year") } }));
    }
    let show = v.get("show").filter(|s| !s.is_null())?;
    let ep = v.get("episode")?;
    Some(json!({
        "show": { "ids": show.get("ids"), "title": show.get("title"), "year": show.get("year") },
        "episode": { "season": ep.get("season"), "number": ep.get("episode").or_else(|| ep.get("number")) },
    }))
}

pub async fn rate(core: &Core, kind: &str, id: &str, rating: Option<u8>) -> AppResult<()> {
    match rating {
        Some(r) => post(core, "/sync/ratings", wrap_list(kind, id, json!({ "rating": r.clamp(1, 10) }))).await?,
        None => post(core, "/sync/ratings/remove", wrap_list(kind, id, json!({}))).await?,
    };
    Ok(())
}

pub async fn set_watched(core: &Core, kind: &str, id: &str, watched: bool) -> AppResult<()> {
    let path = if watched { "/sync/history" } else { "/sync/history/remove" };
    let parts: Vec<&str> = id.split(':').collect();
    let body = if kind == "series" && parts.len() >= 3 {
        // Episode-level: one show, one season, one episode.
        let ids = item_body("movie", parts[0], None, None)["movie"]["ids"].clone();
        json!({ "shows": [{ "ids": ids, "seasons": [{ "number": parts[1].parse::<u32>().unwrap_or(1), "episodes": [{ "number": parts[2].parse::<u32>().unwrap_or(1) }] }] }] })
    } else {
        wrap_list(kind, id, json!({}))
    };
    post(core, path, body).await.map(|_| ())
}

pub async fn watchlist(core: &Core, kind: &str, id: &str, add: bool) -> AppResult<()> {
    if !connected(core) || !core.settings.read().integrations.simkl_sync_watchlist {
        return Ok(());
    }
    if add {
        post(core, "/sync/add-to-list", wrap_list(kind, id, json!({ "to": "plantowatch" }))).await?;
    } else {
        // `/sync/history/remove` un-tracks the title completely, history and
        // all -- so only do it when it's sitting in Plan to Watch. Removing
        // something you've watched from Lumen's library must not erase it on
        // Simkl.
        let imdb = id.split(':').next().unwrap_or(id);
        match super::simkl_sync::entry(core, imdb) {
            Some(e) if e["status"] == "plantowatch" => {
                post(core, "/sync/history/remove", wrap_list(kind, id, json!({}))).await?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// The user's own state for one title: rating + list status. Read from the
/// local mirror (no API call); falls back to one `/sync/watched` lookup
/// before the first sync has finished.
pub async fn status_of(core: &Core, kind: &str, id: &str) -> AppResult<Value> {
    if !connected(core) {
        return Ok(Value::Null);
    }
    let imdb = id.split(':').next().unwrap_or(id);
    if super::simkl_sync::has_data(core) {
        let e = super::simkl_sync::entry(core, imdb);
        return Ok(json!({
            "watched": e.as_ref().map(|e| json!({
                "status": e.get("status"),
                "last_watched_at": e.get("last_watched_at"),
                "episodes_watched": e.get("watched_episodes_count"),
                "episodes_total": e.get("total_episodes_count"),
                "next_to_watch": e.get("next_to_watch_info").or_else(|| e.get("next_to_watch")),
            })),
            "rating": e.as_ref().and_then(|e| e.get("user_rating").cloned()),
        }));
    }
    let body = json!([{ "imdb": imdb, "type": if kind == "series" { "show" } else { "movie" } }]);
    let v = post(core, "/sync/watched?extended=counters", body).await.unwrap_or(Value::Null);
    Ok(json!({ "watched": v.get(0), "rating": Value::Null }))
}

/// Trending this week on Simkl, for the hero. Uses Simkl's pre-built data
/// file (CDN, no key or token needed, refreshed daily) instead of the API.
pub async fn trending(core: &Core) -> AppResult<Vec<Value>> {
    let u = format!("https://data.simkl.in/discover/trending/week_100.json?client_id={}&app-name={APP}&app-version=1.0", urlencoding::encode(&v2_id(core)));
    let d: Value = core.http.get_json_cached(&u, Duration::from_secs(3600 * 6), Duration::from_secs(12)).await?;
    let (v, tv) = (d.get("movies").cloned().unwrap_or(Value::Null), d.get("tv").cloned().unwrap_or(Value::Null));
    let mut out = Vec::new();
    let take = |arr: &Value, kind: &str, out: &mut Vec<Value>| {
        for e in arr.as_array().into_iter().flatten().take(10) {
            let Some(imdb) = e.pointer("/ids/imdb").and_then(|i| i.as_str()) else { continue };
            out.push(json!({
                "id": imdb,
                "type": kind,
                "name": e.get("title"),
                "description": e.get("overview"),
                "releaseInfo": e.get("release_date").and_then(|d| d.as_str()).and_then(|d| d.rsplit('/').next()).map(|y| y.get(0..4).unwrap_or(y).to_string()),
                "genres": e.get("genres"),
                "imdbRating": e.pointer("/ratings/imdb/rating"),
                "runtime": e.get("runtime"),
                "background": e.get("fanart").and_then(|f| f.as_str()).map(|f| format!("https://simkl.in/fanart/{f}_medium.jpg")).unwrap_or_else(|| metahub(imdb, "background")),
                "poster": metahub(imdb, "poster"),
                "logo": metahub(imdb, "logo"),
                "trailer": e.get("trailer"),
                "certification": e.get("certification"),
            }));
        }
    };
    take(&v, "movie", &mut out);
    take(&tv, "series", &mut out);
    // Interleave so the hero alternates film / show.
    let (m, s): (Vec<Value>, Vec<Value>) = out.into_iter().partition(|x| x["type"] == "movie");
    let mut mixed = Vec::new();
    let mut mi = m.into_iter();
    let mut si = s.into_iter();
    loop {
        match (mi.next(), si.next()) {
            (None, None) => break,
            (a, b) => {
                if let Some(a) = a {
                    mixed.push(a)
                }
                if let Some(b) = b {
                    mixed.push(b)
                }
            }
        }
    }
    Ok(mixed)
}
