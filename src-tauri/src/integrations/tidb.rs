//! TheIntroDB: intro / recap / credits / preview segments per episode.

use crate::core::Core;
use crate::error::AppResult;
use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "https://api.theintrodb.org/v3";

/// `video_id` is Stremio's: `tt123` for a film, `tt123:1:4` for S1E4.
pub async fn segments(core: &Core, video_id: &str, duration_ms: Option<u64>) -> AppResult<Value> {
    if !core.settings.read().integrations.tidb_enabled {
        return Ok(Value::Null);
    }
    let parts: Vec<&str> = video_id.split(':').collect();
    let id = parts[0];
    let mut q = if id.starts_with("tt") { format!("imdb_id={id}") } else { format!("tmdb_id={id}") };
    if parts.len() >= 3 {
        q.push_str(&format!("&season={}&episode={}", parts[1], parts[2]));
    }
    if let Some(d) = duration_ms {
        q.push_str(&format!("&duration_ms={d}"));
    }
    let mut req = core.http.client().get(format!("{API}/media?{q}")).timeout(Duration::from_secs(8)).header("User-Agent", "Lumen");
    if let Some(k) = core.db.secret("tidb") {
        req = req.bearer_auth(k);
    }
    let res = req.send().await?;
    if !res.status().is_success() {
        return Ok(Value::Null);
    }
    let v: Value = res.json().await?;
    let mut out = serde_json::Map::new();
    for kind in ["intro", "recap", "credits", "preview"] {
        let list: Vec<Value> = v
            .get(kind)
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter(|s| s.get("start_ms").and_then(|x| x.as_f64()).unwrap_or(0.0) > 0.0 || s.get("end_ms").and_then(|x| x.as_f64()).unwrap_or(0.0) > 0.0)
            .map(|s| {
                json!({
                    "start": s.get("start_ms").and_then(|x| x.as_f64()).unwrap_or(0.0) / 1000.0,
                    "end": s.get("end_ms").and_then(|x| x.as_f64()).map(|e| e / 1000.0),
                })
            })
            .collect();
        out.insert(kind.into(), Value::Array(list));
    }
    Ok(Value::Object(out))
}
