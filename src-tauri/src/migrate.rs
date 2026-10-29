//! One-click move from Stremio Enhanced: pulls the keys your plugins were
//! holding (Simkl token + client id, TMDB key, TheIntroDB key, your art
//! worker URL, image proxy) so nothing has to be pasted again.
//!
//! Chromium keeps localStorage in LevelDB. Recent values sit uncompressed in
//! the .log file and often in .ldb tables, as Latin-1 strings, so a careful
//! scan for the plugins' own JSON is enough -- no LevelDB reader needed.

use crate::core::Core;
use crate::error::{AppError, AppResult};
use regex::Regex;
use serde_json::{json, Value};
use std::path::PathBuf;

fn root() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("stremio-enhanced"))
}

fn latest<'a>(text: &'a str, key: &str, re: &Regex) -> Option<String> {
    // The newest write of a key is the last one in the log.
    let mut found = None;
    for (i, _) in text.match_indices(key) {
        let window = &text[i..(i + 4000).min(text.len())];
        if let Some(c) = re.captures(window) {
            found = Some(c[1].to_string());
        }
    }
    found
}

pub fn import(core: &Core) -> AppResult<Value> {
    let root = root().ok_or_else(|| AppError::msg("No app data folder"))?;
    if !root.exists() {
        return Err(AppError::msg("Stremio Enhanced isn't installed on this computer"));
    }
    let mut blob = String::new();
    let ls = root.join("Local Storage").join("leveldb");
    if let Ok(rd) = std::fs::read_dir(&ls) {
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| matches!(p.extension().and_then(|x| x.to_str()), Some("log") | Some("ldb"))).collect();
        // .ldb (older) first, .log (newest) last, so later matches win.
        files.sort_by_key(|p| (p.extension().and_then(|x| x.to_str()) == Some("log"), p.metadata().and_then(|m| m.modified()).ok()));
        for f in files {
            if let Ok(bytes) = std::fs::read(&f) {
                blob.push_str(&bytes.iter().map(|b| *b as char).collect::<String>());
                blob.push('\n');
            }
        }
    }

    let mut got = serde_json::Map::new();
    let quoted = |field: &str| Regex::new(&format!(r#"\\?"{field}\\?"\s*:\s*\\?"([^"\\]{{6,}})\\?""#)).unwrap();

    if let Some(v) = latest(&blob, "simklSyncConfig", &quoted("token")) {
        core.db.set_secret("simkl_token", Some(&v))?;
        got.insert("simklToken".into(), json!(true));
    }
    if let Some(v) = latest(&blob, "simklSyncConfig", &quoted("clientId")) {
        core.db.set_secret("simkl_client_id", Some(&v))?;
        got.insert("simklClientId".into(), json!(true));
    }
    if let Some(v) = latest(&blob, "dataEnrichmentConfig", &quoted("tmdbApiKey")) {
        core.db.set_secret("tmdb", Some(&v))?;
        got.insert("tmdb".into(), json!(true));
    }
    let url_re = Regex::new(r#"(https://[a-z0-9.\-]+\.workers\.dev)"#).unwrap();
    // Your Simkl worker doubles as the art endpoint; if the explicit setting
    // isn't readable, its addon URL is.
    let simkl_worker = Regex::new(r#"(https://[a-z0-9.\-]*simkl[a-z0-9.\-]*\.workers\.dev)"#).unwrap();
    let worker = latest(&blob, "heroMotnWorker", &url_re).or_else(|| simkl_worker.captures_iter(&blob).last().map(|c| c[1].to_string()));
    if let Some(v) = worker {
        core.db.set_secret("art_worker", Some(&v))?;
        got.insert("artWorker".into(), json!(true));
    }
    if let Some(v) = latest(&blob, "glassPolishConfig", &Regex::new(r#"\\?"imageProxy\\?"\s*:\s*\\?"(https://[^"\\]+)"#).unwrap()) {
        core.settings.write().network.image_proxy = v;
        got.insert("imageProxy".into(), json!(true));
    }

    // TheIntroDB keeps its settings in a plain JSON file.
    if let Ok(txt) = std::fs::read_to_string(root.join("plugins").join("tidb.plugin.json")) {
        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
            if let Some(k) = v.get("tidb_api_key").and_then(|k| k.as_str()).filter(|k| !k.is_empty()) {
                core.db.set_secret("tidb", Some(k))?;
                got.insert("tidb".into(), json!(true));
            }
        }
    }

    let s = core.settings();
    core.db.set("settings", &s)?;
    Ok(Value::Object(got))
}
