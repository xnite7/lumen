//! Downloads: a film or an episode saved to this computer, to watch offline.
//!
//! The stream couch would play is fetched to Videos\couch, resuming where it
//! stopped if the app closes or the connection drops. Once there, playing that
//! title or episode uses the file (no internet needed).

use crate::core::Core;
use crate::error::{AppError, AppResult};
use futures::StreamExt;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

const KEY: &str = "downloads";

struct Live {
    got: AtomicU64,
    total: AtomicU64,
    stop: AtomicBool,
    /// bytes per second, smoothed
    speed: AtomicU64,
    error: Mutex<Option<String>>,
}

static LIVE: std::sync::LazyLock<Mutex<HashMap<String, Arc<Live>>>> = std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

fn folder() -> PathBuf {
    dirs::video_dir().or_else(|| dirs::home_dir().map(|h| h.join("Videos"))).unwrap_or_else(|| PathBuf::from(".")).join("couch")
}

fn clean(s: &str) -> String {
    s.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { ' ' } else { c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ").chars().take(120).collect()
}

fn entries(core: &Core) -> Vec<Value> {
    core.db.get::<Vec<Value>>(KEY).unwrap_or_default()
}
fn save(core: &Core, list: &[Value]) {
    let _ = core.db.set(KEY, &list);
}

/// The downloaded file for this film / episode, if it's all here.
pub fn local(core: &Core, video_id: &str) -> Option<String> {
    entries(core)
        .into_iter()
        .find(|e| e["id"].as_str() == Some(video_id) && e["done"] == true)
        .and_then(|e| e["path"].as_str().map(String::from))
        .filter(|p| std::path::Path::new(p).exists())
}

/// Everything saved or saving, with how far along.
pub fn list(core: &Core) -> Value {
    let live = LIVE.lock();
    let out: Vec<Value> = entries(core)
        .into_iter()
        .map(|mut e| {
            let id = e["id"].as_str().unwrap_or("").to_string();
            if let Some(l) = live.get(&id) {
                e["got"] = json!(l.got.load(Ordering::Relaxed));
                e["total"] = json!(l.total.load(Ordering::Relaxed));
                e["speed"] = json!(l.speed.load(Ordering::Relaxed));
                e["active"] = json!(true);
                if let Some(err) = l.error.lock().clone() {
                    e["error"] = json!(err);
                    e["active"] = json!(false);
                }
            } else if e["done"] != true {
                // stopped (couch was closed): it carries on when started again
                let got = std::fs::metadata(format!("{}.part", e["path"].as_str().unwrap_or(""))).map(|m| m.len()).unwrap_or(0);
                e["got"] = json!(got);
            }
            e
        })
        .collect();
    json!(out)
}

/// Start (or carry on with) saving this stream.
pub fn start(core: Arc<Core>, url: String, headers: Vec<(String, String)>, meta: Value, video_id: String, label: String, filename_hint: Option<String>) -> AppResult<()> {
    if LIVE.lock().get(&video_id).is_some_and(|l| !l.stop.load(Ordering::Relaxed) && l.error.lock().is_none()) {
        return Ok(());
    }
    let dir = folder();
    std::fs::create_dir_all(&dir).map_err(|e| AppError::msg(format!("couldn't make {}: {e}", dir.display())))?;
    let ext = filename_hint
        .as_deref()
        .or_else(|| url.split('?').next())
        .and_then(|f| f.rsplit_once('.').map(|(_, e)| e.to_lowercase()))
        .filter(|e| ["mkv", "mp4", "avi", "m4v", "webm", "ts", "mov"].contains(&e.as_str()))
        .unwrap_or_else(|| "mkv".into());
    let mut list = entries(&core);
    let path = match list.iter().find(|e| e["id"].as_str() == Some(video_id.as_str())).and_then(|e| e["path"].as_str().map(String::from)) {
        Some(p) => PathBuf::from(p),
        None => dir.join(format!("{}.{ext}", clean(&label))),
    };
    list.retain(|e| e["id"].as_str() != Some(video_id.as_str()));
    list.insert(0, json!({ "id": video_id, "meta": meta, "label": label, "path": path.to_string_lossy(), "done": false, "at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }));
    save(&core, &list);
    let live = Arc::new(Live { got: AtomicU64::new(0), total: AtomicU64::new(0), stop: AtomicBool::new(false), speed: AtomicU64::new(0), error: Mutex::new(None) });
    LIVE.lock().insert(video_id.clone(), live.clone());
    crate::rt::spawn(async move {
        let r = fetch(&core, &url, &headers, &path, &live).await;
        match r {
            Ok(()) => {
                let mut list = entries(&core);
                if let Some(e) = list.iter_mut().find(|e| e["id"].as_str() == Some(video_id.as_str())) {
                    e["done"] = json!(true);
                    e["size"] = json!(live.total.load(Ordering::Relaxed));
                }
                save(&core, &list);
                LIVE.lock().remove(&video_id);
                core.emit("downloads-changed", ());
            }
            Err(e) => {
                if !live.stop.load(Ordering::Relaxed) {
                    *live.error.lock() = Some(e.to_string());
                }
                core.emit("downloads-changed", ());
            }
        }
    });
    Ok(())
}

async fn fetch(core: &Core, url: &str, headers: &[(String, String)], path: &PathBuf, live: &Live) -> anyhow::Result<()> {
    let part = PathBuf::from(format!("{}.part", path.to_string_lossy()));
    let mut tries = 0;
    loop {
        let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        live.got.store(have, Ordering::Relaxed);
        let mut req = core.http.client().get(url);
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        let res = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                tries += 1;
                if tries > 5 {
                    anyhow::bail!("the download stopped: {e}");
                }
                tokio::time::sleep(std::time::Duration::from_secs(3 * tries)).await;
                continue;
            }
        };
        let status = res.status().as_u16();
        if !(status == 200 || status == 206) {
            anyhow::bail!("the source answered {status}");
        }
        // asked to carry on but got the whole thing: start over
        let from = if status == 206 { have } else { 0 };
        let total = from + res.content_length().unwrap_or(0);
        live.total.store(total, Ordering::Relaxed);
        let mut f = std::fs::OpenOptions::new().create(true).write(true).append(status == 206).truncate(status != 206).open(&part)?;
        let mut got = from;
        let mut body = res.bytes_stream();
        let mut tick = std::time::Instant::now();
        let mut since = got;
        let mut broke = false;
        while let Some(c) = body.next().await {
            if live.stop.load(Ordering::Relaxed) {
                return Err(anyhow::anyhow!("stopped"));
            }
            let c = match c {
                Ok(c) => c,
                Err(_) => {
                    broke = true;
                    break;
                }
            };
            f.write_all(&c)?;
            got += c.len() as u64;
            live.got.store(got, Ordering::Relaxed);
            let el = tick.elapsed().as_secs_f64();
            if el > 1.0 {
                let now = ((got - since) as f64 / el) as u64;
                let old = live.speed.load(Ordering::Relaxed);
                live.speed.store(if old == 0 { now } else { (old * 3 + now) / 4 }, Ordering::Relaxed);
                tick = std::time::Instant::now();
                since = got;
            }
        }
        f.flush()?;
        drop(f);
        if broke || (total > 0 && got < total) {
            tries += 1;
            if tries > 8 {
                anyhow::bail!("the connection kept dropping");
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            continue;
        }
        std::fs::rename(&part, path)?;
        return Ok(());
    }
}

/// Stop saving (what's there stays, to carry on later).
pub fn pause(video_id: &str) {
    if let Some(l) = LIVE.lock().remove(video_id) {
        l.stop.store(true, Ordering::Relaxed);
    }
}

/// Stop and delete it, file and all.
pub fn remove(core: &Core, video_id: &str) {
    pause(video_id);
    let mut list = entries(core);
    if let Some(e) = list.iter().find(|e| e["id"].as_str() == Some(video_id)) {
        if let Some(p) = e["path"].as_str() {
            let _ = std::fs::remove_file(p);
            let _ = std::fs::remove_file(format!("{p}.part"));
        }
    }
    list.retain(|e| e["id"].as_str() != Some(video_id));
    save(core, &list);
}

pub fn folder_path() -> String {
    folder().to_string_lossy().to_string()
}
