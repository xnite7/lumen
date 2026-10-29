//! Library + watch progress, stored in Stremio's own `LibraryItem` shape so
//! the same records sync to your Stremio account (and so to your phone)
//! without translation.

use crate::core::Core;
use crate::db::now_ms;
use crate::error::AppResult;
use serde_json::{json, Value};

fn iso(ms: i64) -> String {
    // RFC3339 with millis, which is what api.strem.io writes.
    let secs = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}.{millis:03}Z")
}

pub fn parse_iso(s: &str) -> i64 {
    // Good enough for ordering: YYYY-MM-DDTHH:MM:SS(.mmm)Z
    let b = s.as_bytes();
    if b.len() < 19 {
        return 0;
    }
    let n = |r: std::ops::Range<usize>| s.get(r).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
    let (y, mo, d, h, mi, se) = (n(0..4), n(5..7), n(8..10), n(11..13), n(14..16), n(17..19));
    let ms = if b.len() >= 23 && b[19] == b'.' { n(20..23) } else { 0 };
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    ((days * 86_400) + h * 3600 + mi * 60 + se) * 1000 + ms
}

pub fn new_item(meta: &Value) -> Value {
    let now = iso(now_ms());
    json!({
        "_id": meta["id"],
        "name": meta["name"],
        "type": meta["type"],
        "poster": meta["poster"],
        "posterShape": meta.get("posterShape").cloned().unwrap_or(json!("poster")),
        "background": meta["background"],
        "logo": meta["logo"],
        "removed": false,
        "temp": false,
        "_ctime": now,
        "_mtime": now,
        "state": {
            "lastWatched": null,
            "timeWatched": 0,
            "timeOffset": 0,
            "overallTimeWatched": 0,
            "timesWatched": 0,
            "flaggedWatched": 0,
            "duration": 0,
            "video_id": null,
            "watched": null,
            "noNotif": false,
            "season": 0,
            "episode": 0
        },
        "behaviorHints": { "defaultVideoId": null, "featuredVideoId": null, "hasScheduledVideos": false }
    })
}

fn touch(item: &mut Value) -> i64 {
    let now = now_ms();
    item["_mtime"] = json!(iso(now));
    now
}

pub fn all(core: &Core) -> AppResult<Vec<Value>> {
    core.db.library_all()
}

pub fn get(core: &Core, id: &str) -> Option<Value> {
    core.db.library_get(id)
}

pub fn add(core: &Core, meta: &Value) -> AppResult<Value> {
    let id = meta["id"].as_str().unwrap_or_default().to_string();
    let mut item = get(core, &id).unwrap_or_else(|| new_item(meta));
    item["removed"] = json!(false);
    item["temp"] = json!(false);
    for k in ["name", "poster", "background", "logo"] {
        if !meta[k].is_null() {
            item[k] = meta[k].clone();
        }
    }
    let t = touch(&mut item);
    core.db.library_put(&id, &item, t, true)?;
    Ok(item)
}

pub fn remove(core: &Core, id: &str) -> AppResult<()> {
    if let Some(mut item) = get(core, id) {
        item["removed"] = json!(true);
        let t = touch(&mut item);
        core.db.library_put(id, &item, t, true)?;
    }
    Ok(())
}

/// Called every few seconds by the player. Items you only watched (never
/// added) are `temp`, exactly as in Stremio: they show in Continue Watching
/// but not in the Library grid.
pub fn progress(core: &Core, meta: &Value, video_id: &str, time_ms: i64, duration_ms: i64, watched_threshold: u32) -> AppResult<Value> {
    let id = meta["id"].as_str().unwrap_or_default().to_string();
    let mut item = get(core, &id).unwrap_or_else(|| {
        let mut i = new_item(meta);
        i["temp"] = json!(true);
        i
    });
    item["removed"] = json!(false);
    let st = &mut item["state"];
    let prev = st["timeOffset"].as_i64().unwrap_or(0);
    let delta = (time_ms - prev).clamp(0, 15_000);
    st["timeOffset"] = json!(time_ms);
    st["duration"] = json!(duration_ms);
    st["video_id"] = json!(video_id);
    st["lastWatched"] = json!(iso(now_ms()));
    st["timeWatched"] = json!(st["timeWatched"].as_i64().unwrap_or(0) + delta);
    st["overallTimeWatched"] = json!(st["overallTimeWatched"].as_i64().unwrap_or(0) + delta);
    let parts: Vec<&str> = video_id.split(':').collect();
    if parts.len() >= 3 {
        st["season"] = json!(parts[parts.len() - 2].parse::<i64>().unwrap_or(0));
        st["episode"] = json!(parts[parts.len() - 1].parse::<i64>().unwrap_or(0));
    }
    if duration_ms > 0 && (time_ms as f64 / duration_ms as f64) * 100.0 >= watched_threshold as f64 {
        st["flaggedWatched"] = json!(1);
        let tw = st["timesWatched"].as_i64().unwrap_or(0);
        if prev * 100 / duration_ms.max(1) < watched_threshold as i64 {
            st["timesWatched"] = json!(tw + 1);
        }
        mark_video_watched(&mut item, video_id, true);
    }
    let t = touch(&mut item);
    core.db.library_put(&id, &item, t, true)?;
    Ok(item)
}

/// Per-episode watched set. Lumen keeps a plain list in `state.watchedIds`
/// (Stremio's own field is a bitfield over the video list that needs the
/// full meta to decode, so it is left untouched for Stremio to manage).
pub fn mark_video_watched(item: &mut Value, video_id: &str, watched: bool) {
    let st = &mut item["state"];
    let mut ids: Vec<String> = st.get("watchedIds").and_then(|w| serde_json::from_value(w.clone()).ok()).unwrap_or_default();
    ids.retain(|x| x != video_id);
    if watched {
        ids.push(video_id.to_string());
    }
    st["watchedIds"] = json!(ids);
}

pub fn set_watched(core: &Core, meta: &Value, video_id: Option<&str>, watched: bool) -> AppResult<Value> {
    let id = meta["id"].as_str().unwrap_or_default().to_string();
    let mut item = get(core, &id).unwrap_or_else(|| {
        let mut i = new_item(meta);
        i["temp"] = json!(true);
        i
    });
    match video_id {
        Some(v) if v != id => mark_video_watched(&mut item, v, watched),
        _ => {
            item["state"]["flaggedWatched"] = json!(if watched { 1 } else { 0 });
            if watched {
                item["state"]["timesWatched"] = json!(item["state"]["timesWatched"].as_i64().unwrap_or(0).max(1));
                item["state"]["timeOffset"] = json!(0);
            }
        }
    }
    let t = touch(&mut item);
    core.db.library_put(&id, &item, t, true)?;
    Ok(item)
}

/// In progress, newest first: started, not finished, not dismissed.
pub fn continue_watching(core: &Core) -> AppResult<Vec<Value>> {
    let mut v: Vec<Value> = all(core)?
        .into_iter()
        .filter(|i| !i["removed"].as_bool().unwrap_or(false) || i["temp"].as_bool().unwrap_or(false))
        .filter(|i| i["state"]["timeOffset"].as_i64().unwrap_or(0) > 0 && !i["state"]["video_id"].is_null())
        .filter(|i| !i["state"]["dismissed"].as_bool().unwrap_or(false))
        .filter(|i| {
            let d = i["state"]["duration"].as_i64().unwrap_or(0);
            let t = i["state"]["timeOffset"].as_i64().unwrap_or(0);
            d == 0 || (t as f64) < d as f64 * 0.95
        })
        .collect();
    v.sort_by_key(|i| -parse_iso(i["state"]["lastWatched"].as_str().unwrap_or("")));
    Ok(v)
}

pub fn dismiss(core: &Core, id: &str) -> AppResult<()> {
    if let Some(mut item) = get(core, id) {
        item["state"]["timeOffset"] = json!(0);
        item["state"]["dismissed"] = json!(true);
        let t = touch(&mut item);
        core.db.library_put(id, &item, t, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn iso_roundtrip() {
        let ms = 1_793_312_117_624;
        assert_eq!(super::parse_iso(&super::iso(ms)), ms);
    }
}
