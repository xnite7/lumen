//! Simkl: sign-in (PIN flow), lists, ratings, history, scrobbling.
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

pub fn client_id(core: &Core) -> Option<String> {
    core.db.secret("simkl_client_id")
}

pub fn token(core: &Core) -> Option<String> {
    core.db.secret("simkl_token")
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
        .header("Content-Type", "application/json")
        .timeout(Duration::from_secs(12));
    if let Some(t) = token(core) {
        b = b.bearer_auth(t);
    }
    Ok(b)
}

pub async fn get(core: &Core, path: &str) -> AppResult<Value> {
    let res = req(core, reqwest::Method::GET, path)?.send().await?;
    if !res.status().is_success() {
        return Err(AppError::msg(format!("Simkl HTTP {} on {path}", res.status())));
    }
    Ok(res.json().await?)
}

pub async fn post(core: &Core, path: &str, body: Value) -> AppResult<Value> {
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

pub async fn scrobble(core: &Core, action: &str, kind: &str, video_id: &str, progress: f64) -> AppResult<()> {
    let s = core.settings.read().integrations.clone();
    if !connected(core) || !s.simkl_scrobble {
        return Ok(());
    }
    let parts: Vec<&str> = video_id.split(':').collect();
    let (season, episode) = if kind == "series" && parts.len() >= 3 {
        (parts[parts.len() - 2].parse().ok(), parts[parts.len() - 1].parse().ok())
    } else {
        (None, None)
    };
    let mut body = item_body(kind, video_id, season, episode);
    body["progress"] = json!((progress * 100.0).clamp(0.0, 100.0));
    post(core, &format!("/scrobble/{action}"), body).await.map(|_| ())
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
        post(core, "/sync/history/remove", wrap_list(kind, id, json!({}))).await?;
    }
    Ok(())
}

/// The user's own state for one title: rating + list status.
pub async fn status_of(core: &Core, kind: &str, id: &str) -> AppResult<Value> {
    if !connected(core) {
        return Ok(Value::Null);
    }
    let imdb = id.split(':').next().unwrap_or(id);
    let body = if kind == "series" {
        json!([{ "imdb": imdb, "type": "show" }])
    } else {
        json!([{ "imdb": imdb, "type": "movie" }])
    };
    let v = post(core, "/sync/watched?extended=counters", body).await.unwrap_or(Value::Null);
    let ratings = get(core, &format!("/sync/ratings/{}", if kind == "series" { "shows" } else { "movies" })).await.unwrap_or(Value::Null);
    let rating = ratings
        .get(if kind == "series" { "shows" } else { "movies" })
        .and_then(|a| a.as_array())
        .and_then(|a| a.iter().find(|e| inner(e).pointer("/ids/imdb").and_then(|i| i.as_str()) == Some(imdb)))
        .and_then(|e| e.get("user_rating").cloned());
    Ok(json!({ "watched": v.get(0), "rating": rating }))
}

/// Trending this week on Simkl, for the hero.
pub async fn trending(core: &Core) -> AppResult<Vec<Value>> {
    let u = format!(
        "{API}/movies/trending/week?extended=overview,metadata,tmdb,genres,trailer&client_id={}",
        urlencoding::encode(&client_id(core).unwrap_or_default())
    );
    let v: Value = core.http.get_json_cached(&u, Duration::from_secs(3600 * 3), Duration::from_secs(10)).await?;
    let tv_u = u.replace("/movies/", "/tv/");
    let tv: Value = core.http.get_json_cached(&tv_u, Duration::from_secs(3600 * 3), Duration::from_secs(10)).await.unwrap_or(Value::Null);
    let mut out = Vec::new();
    let take = |arr: &Value, kind: &str, out: &mut Vec<Value>| {
        for e in arr.as_array().into_iter().flatten().take(10) {
            let Some(imdb) = e.pointer("/ids/imdb").and_then(|i| i.as_str()) else { continue };
            out.push(json!({
                "id": imdb,
                "type": kind,
                "name": e.get("title"),
                "description": e.get("overview"),
                "releaseInfo": e.get("release_date").and_then(|d| d.as_str()).and_then(|d| d.get(0..4)),
                "genres": e.get("genres"),
                "imdbRating": e.pointer("/ratings/imdb/rating"),
                "runtime": e.get("runtime"),
                "background": e.get("fanart").and_then(|f| f.as_str()).map(|f| format!("https://simkl.in/fanart/{f}_medium.jpg")).unwrap_or_else(|| metahub(imdb, "background")),
                "poster": metahub(imdb, "poster"),
                "logo": metahub(imdb, "logo"),
                "trailer": e.pointer("/trailer/youtube").or_else(|| e.get("trailer")),
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
