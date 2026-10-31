//! A local mirror of the user's Simkl library, kept current the way Simkl's
//! sync guide prescribes:
//!
//! - first run: read `/sync/activities` (the snapshot) BEFORE pulling each
//!   type in full, one after another; save the snapshot only if every pull
//!   worked;
//! - afterwards: read `/sync/activities`; if `all` hasn't moved, stop (the
//!   common, nearly free path); otherwise fetch only the buckets that moved,
//!   each with `date_from` = the saved snapshot's `all`, deletions by an
//!   id-only diff, ratings by their own delta, paused playbacks only when
//!   `playback` moved.
//!
//! Everything that used to call Simkl on demand (Home rows, the detail
//! page's rating and status, "is this in Plan to Watch?") reads the mirror,
//! so browsing costs no API calls at all.

use super::simkl::{self, get, inner};
use crate::core::Core;
use crate::error::AppResult;
use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicBool, Ordering};

const KEY: &str = "simkl_mirror";
static RUNNING: AtomicBool = AtomicBool::new(false);

fn load(core: &Core) -> Value {
    core.db.get::<Value>(KEY).unwrap_or_else(|| json!({ "items": {}, "snapshot": null, "playback": [] }))
}

fn save(core: &Core, m: &Value) {
    let _ = core.db.set(KEY, m);
}

pub fn clear(core: &Core) {
    let _ = core.db.del(KEY);
}

/// "shows" / "movies" / "anime" bucket -> Lumen kind.
fn kind_of(bucket: &str, e: &Value) -> &'static str {
    match bucket {
        "movies" => "movie",
        "anime" if e.get("anime_type").and_then(|t| t.as_str()) == Some("movie") => "movie",
        _ => "series",
    }
}

fn simkl_id(e: &Value) -> Option<String> {
    inner(e).pointer("/ids/simkl").or_else(|| inner(e).pointer("/ids/simkl_id")).map(|v| v.to_string().trim_matches('"').to_string())
}

fn merge_bucket(items: &mut Map<String, Value>, bucket: &str, rows: &Value) {
    for e in rows.as_array().into_iter().flatten() {
        let Some(id) = simkl_id(e) else { continue };
        let mut e = e.clone();
        e["_bucket"] = json!(bucket);
        e["_kind"] = json!(kind_of(bucket, &e));
        items.insert(id, e);
    }
}

fn merge_all(items: &mut Map<String, Value>, v: &Value) {
    for b in ["shows", "movies", "anime"] {
        if let Some(rows) = v.get(b) {
            merge_bucket(items, b, rows);
        }
    }
}

/// Run one sync pass. Cheap when nothing changed (one tiny request).
pub async fn sync(core: &Core) -> AppResult<bool> {
    if !simkl::connected(core) || RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(false);
    }
    let r = run(core).await;
    RUNNING.store(false, Ordering::SeqCst);
    r
}

async fn run(core: &Core) -> AppResult<bool> {
    let mut m = load(core);
    let act = get(core, "/sync/activities").await?;
    let snap = m.get("snapshot").cloned().unwrap_or(Value::Null);
    let mut items: Map<String, Value> = m.get("items").and_then(|i| i.as_object()).cloned().unwrap_or_default();

    if snap.is_null() {
        // Phase 1: full pull, type by type (never in parallel)
        items.clear();
        for t in ["shows", "movies", "anime"] {
            let v = get(core, &format!("/sync/all-items/{t}?next_watch_info=yes")).await?;
            merge_all(&mut items, &v);
        }
        let playback = get(core, "/sync/playback").await.unwrap_or(json!([]));
        m["items"] = Value::Object(items);
        m["playback"] = playback;
        m["snapshot"] = act;
        save(core, &m);
        return Ok(true);
    }

    let all = |v: &Value| v.get("all").and_then(|a| a.as_str()).unwrap_or("").to_string();
    if all(&act) == all(&snap) {
        return Ok(false);
    }
    let date_from = all(&snap);
    let moved = |block: &str, key: &str| act.pointer(&format!("/{block}/{key}")) != snap.pointer(&format!("/{block}/{key}"));
    let blocks = ["tv_shows", "anime", "movies"];

    let list_moved = blocks.iter().any(|b| ["watching", "plantowatch", "hold", "completed", "dropped", "notinteresting"].iter().any(|k| moved(b, k)));
    if list_moved {
        let v = get(core, &format!("/sync/all-items?date_from={}&next_watch_info=yes", urlencoding::encode(&date_from))).await?;
        merge_all(&mut items, &v);
    }
    if blocks.iter().any(|b| moved(b, "removed_from_list")) {
        let v = get(core, "/sync/all-items?extended=simkl_ids_only").await?;
        let mut keep = std::collections::HashSet::new();
        for b in ["shows", "movies", "anime"] {
            for e in v.get(b).and_then(|a| a.as_array()).into_iter().flatten() {
                if let Some(id) = simkl_id(e) {
                    keep.insert(id);
                }
            }
        }
        items.retain(|k, _| keep.contains(k));
    }
    if blocks.iter().any(|b| moved(b, "rated_at")) {
        // full rows, null rating = removed; never filter by rating value
        let v = get(core, &format!("/sync/ratings?date_from={}", urlencoding::encode(&date_from))).await?;
        for b in ["shows", "movies", "anime"] {
            for e in v.get(b).and_then(|a| a.as_array()).into_iter().flatten() {
                let Some(id) = simkl_id(e) else { continue };
                if let Some(cur) = items.get_mut(&id) {
                    cur["user_rating"] = e.get("user_rating").cloned().unwrap_or(Value::Null);
                    cur["user_rated_at"] = e.get("user_rated_at").cloned().unwrap_or(Value::Null);
                } else {
                    merge_bucket(&mut items, b, &json!([e]));
                }
            }
        }
    }
    if blocks.iter().any(|b| moved(b, "playback")) {
        m["playback"] = get(core, "/sync/playback").await?;
    }
    if act.pointer("/settings/all") != snap.pointer("/settings/all") {
        if let Ok(me) = get(core, "/users/settings").await {
            let _ = core.db.set("simkl_user", &me);
        }
    }
    m["items"] = Value::Object(items);
    m["snapshot"] = act;
    save(core, &m);
    Ok(true)
}

// ------------------------------------------------------------- reads

fn imdb_of(e: &Value) -> Option<&str> {
    inner(e).pointer("/ids/imdb").and_then(|i| i.as_str()).filter(|s| s.starts_with("tt"))
}

/// The mirror's entry for an IMDb id, if the title is in any list.
pub fn entry(core: &Core, imdb: &str) -> Option<Value> {
    let m = load(core);
    m.get("items")?.as_object()?.values().find(|e| imdb_of(e) == Some(imdb)).cloned()
}

pub fn has_data(core: &Core) -> bool {
    !load(core).get("snapshot").map_or(true, |s| s.is_null())
}

/// Items in one status for one Lumen kind, most recently touched first.
pub fn list(core: &Core, status: &str, kind: &str) -> Vec<Value> {
    let m = load(core);
    let mut rows: Vec<Value> = m
        .get("items")
        .and_then(|i| i.as_object())
        .map(|o| o.values().filter(|e| e["status"] == status && e["_kind"] == kind).cloned().collect())
        .unwrap_or_default();
    let ts = |e: &Value| {
        e.get("last_watched_at").or_else(|| e.get("added_to_watchlist_at")).and_then(|t| t.as_str()).unwrap_or("").to_string()
    };
    rows.sort_by_key(|e| std::cmp::Reverse(ts(e)));
    rows
}

/// Shows you're watching that have a next episode out: Simkl fills
/// `next_to_watch` itself when a new episode airs.
pub fn up_next(core: &Core) -> Vec<Value> {
    let mut rows = list(core, "watching", "series");
    rows.retain(|e| e.get("next_to_watch_info").map_or(false, |n| !n.is_null()) || e.get("next_to_watch").and_then(|n| n.as_str()).is_some());
    rows
}

/// Paused playbacks from any Simkl app, newest first.
pub fn playbacks(core: &Core) -> Vec<Value> {
    let mut v: Vec<Value> = load(core).get("playback").and_then(|p| p.as_array()).cloned().unwrap_or_default();
    v.sort_by_key(|p| std::cmp::Reverse(p.get("paused_at").and_then(|t| t.as_str()).unwrap_or("").to_string()));
    v
}

/// Resume point (0..1) Simkl holds for this title/episode, if any.
pub fn resume_fraction(core: &Core, imdb: &str, season: Option<u32>, episode: Option<u32>) -> Option<f64> {
    playbacks(core).into_iter().find_map(|p| {
        if imdb_of(&p) != Some(imdb) {
            return None;
        }
        if let (Some(s), Some(e)) = (season, episode) {
            let ps = p.pointer("/episode/tvdb_season").or_else(|| p.pointer("/episode/season")).and_then(|v| v.as_u64());
            let pe = p.pointer("/episode/tvdb_number").or_else(|| p.pointer("/episode/number")).and_then(|v| v.as_u64());
            if ps != Some(s as u64) || pe != Some(e as u64) {
                return None;
            }
        }
        let pct = p.get("progress").and_then(|v| v.as_f64())?;
        (2.0..95.0).contains(&pct).then_some(pct / 100.0)
    })
}

/// Continue Watching entries (Stremio LibraryItem shape) for Simkl pauses
/// that Lumen's own library doesn't already have.
pub fn continue_items(core: &Core, have: &std::collections::HashSet<String>) -> Vec<Value> {
    let mut out = vec![];
    for p in playbacks(core) {
        let Some(imdb) = imdb_of(&p).map(|s| s.to_string()) else { continue };
        if have.contains(&imdb) {
            continue;
        }
        let pct = p.get("progress").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if !(2.0..95.0).contains(&pct) {
            continue;
        }
        let it = inner(&p);
        let is_movie = p.get("type").and_then(|t| t.as_str()) == Some("movie");
        let (s, e) = (
            p.pointer("/episode/tvdb_season").or_else(|| p.pointer("/episode/season")).and_then(|v| v.as_u64()),
            p.pointer("/episode/tvdb_number").or_else(|| p.pointer("/episode/number")).and_then(|v| v.as_u64()),
        );
        let video_id = match (is_movie, s, e) {
            (false, Some(s), Some(e)) => format!("{imdb}:{s}:{e}"),
            _ => imdb.clone(),
        };
        out.push(json!({
            "_id": imdb,
            "name": it.get("title"),
            "type": if is_movie { "movie" } else { "series" },
            "poster": simkl::metahub(&imdb, "poster"),
            "background": simkl::metahub(&imdb, "background"),
            "logo": simkl::metahub(&imdb, "logo"),
            "removed": false,
            "temp": true,
            "_mtime": p.get("paused_at"),
            "_simklPlayback": p.get("id"),
            "state": {
                "lastWatched": p.get("paused_at"),
                "timeOffset": (pct * 1000.0) as i64,
                "duration": 100_000,
                "video_id": video_id,
                "season": s,
                "episode": e,
                "flaggedWatched": 0,
                "timesWatched": 0,
            }
        }));
    }
    out
}

/// Forget a Simkl pause point (Continue Watching → remove).
pub async fn delete_playback(core: &Core, imdb: &str) -> AppResult<()> {
    let ids: Vec<String> = playbacks(core).into_iter().filter(|p| imdb_of(p) == Some(imdb)).filter_map(|p| p.get("id").map(|i| i.to_string())).collect();
    for id in &ids {
        let _ = simkl::delete(core, &format!("/sync/playback/{id}")).await;
    }
    let mut m = load(core);
    if let Some(a) = m.get_mut("playback").and_then(|p| p.as_array_mut()) {
        a.retain(|p| imdb_of(p) != Some(imdb));
    }
    save(core, &m);
    Ok(())
}
