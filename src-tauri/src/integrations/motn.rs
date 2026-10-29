//! Movie of the Night: real 16:9 posters (title baked in) for Continue
//! Watching and the hero, plus today's Top 10 per streaming service.
//!
//! 500 requests a MONTH, so this is cache-first by design: per-title art for
//! 30 days (the signed URLs stay valid ~329 days), top lists for a day. With
//! no key set, it asks your existing Simkl worker's /art endpoint instead,
//! which already holds the key and its own edge cache.

use crate::core::Core;
use crate::error::AppResult;
use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "https://api.movieofthenight.com/v4";
const T: Duration = Duration::from_secs(10);
const MONTH: Duration = Duration::from_secs(86_400 * 30);

fn real_art(u: Option<&Value>) -> Option<String> {
    // MOTN answers missing art with a generated SVG card from www.; genuine
    // art always comes off cdn.
    u.and_then(|v| v.as_str()).filter(|s| s.starts_with("https://cdn.movieofthenight.com/")).map(|s| s.to_string())
}

fn pick(set: Option<&Value>, order: &[&str]) -> Option<String> {
    let set = set?;
    order.iter().find_map(|k| real_art(set.get(*k)))
}

/// Upcoming titles get auto-ingested as stubs with someone else's artwork.
/// Missing overview AND missing availability together is the only tell.
fn curated(s: &Value) -> bool {
    let has_overview = s.get("overview").and_then(|o| o.as_str()).map_or(false, |o| !o.trim().is_empty());
    let has_streaming = s
        .get("streamingOptions")
        .and_then(|o| o.as_object())
        .map_or(false, |o| o.values().any(|a| a.as_array().map_or(false, |a| !a.is_empty())));
    has_overview || has_streaming
}

async fn motn_get(core: &Core, path: &str, ttl: Duration) -> Option<Value> {
    let key = core.db.secret("motn")?;
    let url = format!("{API}{path}");
    if let Some(body) = core.db.cache_get(&url) {
        return serde_json::from_slice(&body).ok();
    }
    let res = core.http.client().get(&url).header("X-API-Key", key).timeout(T).send().await.ok()?;
    if !res.status().is_success() {
        return None; // 402 = out of quota; degrade quietly
    }
    let bytes = res.bytes().await.ok()?;
    core.db.cache_put(&url, &bytes, ttl.as_millis() as i64);
    serde_json::from_slice(&bytes).ok()
}

/// `{ backdrop, poster }` -- wide backdrop and the 16:9 poster with title.
pub async fn art(core: &Core, imdb: &str) -> AppResult<Value> {
    let country = core.settings.read().integrations.motn_country.clone();
    if core.db.secret("motn").is_some() {
        if let Some(s) = motn_get(core, &format!("/shows/{imdb}?country={country}"), MONTH).await {
            if curated(&s) {
                return Ok(json!({
                    "backdrop": pick(s.pointer("/imageSet/horizontalBackdrop"), &["w1440", "w1080", "w720"]),
                    "poster": pick(s.pointer("/imageSet/horizontalPoster"), &["w720", "w1080", "w480"]),
                }));
            }
            return Ok(json!({ "backdrop": null, "poster": null }));
        }
    }
    if let Some(worker) = core.db.secret("art_worker") {
        let u = format!("{}/art/{imdb}.json", worker.trim_end_matches('/'));
        if let Ok(v) = core.http.get_json_cached::<Value>(&u, MONTH, T).await {
            return Ok(json!({ "backdrop": v.get("backdrop"), "poster": v.get("poster") }));
        }
    }
    Ok(json!({ "backdrop": null, "poster": null }))
}

pub async fn top(core: &Core, stremio_type: &str) -> AppResult<Vec<Value>> {
    let s = core.settings.read().integrations.clone();
    let show_type = if stremio_type == "series" { "series" } else { "movie" };
    let path = format!("/shows/top?country={}&service={}&show_type={show_type}", s.motn_country, s.motn_service);
    let Some(rows) = motn_get(core, &path, Duration::from_secs(86_400)).await else { return Ok(vec![]) };
    let mut out = Vec::new();
    for r in rows.as_array().into_iter().flatten() {
        let Some(imdb) = r.get("imdbId").and_then(|v| v.as_str()).filter(|s| s.starts_with("tt")) else { continue };
        let trust = curated(r);
        out.push(json!({
            "id": imdb,
            "type": show_type,
            "name": r.get("title"),
            "poster": trust.then(|| pick(r.pointer("/imageSet/verticalPoster"), &["w480", "w600", "w360"])).flatten()
                .unwrap_or_else(|| format!("https://images.metahub.space/poster/medium/{imdb}/img")),
            "posterShape": "poster",
            "background": trust.then(|| pick(r.pointer("/imageSet/horizontalBackdrop"), &["w1440", "w1080"])).flatten(),
            "releaseInfo": r.get("releaseYear").map(|y| y.to_string()),
            "description": r.get("overview"),
            "imdbRating": r.get("rating").and_then(|v| v.as_f64()).filter(|v| *v > 0.0).map(|v| format!("{:.1}", v / 10.0)),
        }));
    }
    Ok(out)
}
