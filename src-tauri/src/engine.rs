//! Starting couch's engine: database, settings, addons, player, lights,
//! and the background sync loop. Both the Tauri app and the GPUI app call
//! `boot` with their own host.

use crate::core::Core;
use crate::host::Host;
use crate::*;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

pub fn boot(host: Arc<dyn Host>, data_dir: PathBuf, cache_dir: PathBuf) -> anyhow::Result<state::AppState> {
    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(&cache_dir)?;

    migrate_from_lumen(&data_dir);
    let db = Arc::new(db::Db::open(&data_dir.join("couch.db"))?);
    db.cache_prune();
    let mut settings: settings::Settings = db.get("settings").unwrap_or_default();
    settings.pin();
    // eye-control dwell used to be 0.8 s, which fired while reading the labels
    if settings.tobii.dwell_ms < 1300 {
        settings.tobii.dwell_ms = 1500;
    }
    // 3D releases are worth seeing now (head-tracked 3D): show them once
    if db.get::<bool>("seen_3d_v1").is_none() {
        settings.streams.hide_3d = false;
        let _ = db.set("seen_3d_v1", &true);
    }
    // couch picks the stream by default now (by connection speed)
    if db.get::<bool>("autopick_v1").is_none() {
        settings.streams.autoplay_best = true;
        let _ = db.set("autopick_v1", &true);
    }
    // previews open about as fast as Netflix's now
    if db.get::<bool>("snappy_previews_v1").is_none() {
        settings.general.hover_delay_ms = settings.general.hover_delay_ms.min(450);
        let _ = db.set("snappy_previews_v1", &true);
    }
    if settings.playback.ios_player == "lumen" {
        settings.playback.ios_player = "couch".into();
    }
    let http = Arc::new(http::Http::new(db.clone(), settings.network.doh.then_some(settings.network.doh_provider.as_str())));
    let core = Arc::new(Core {
        host,
        db,
        http,
        settings: RwLock::new(settings.clone()),
        data_dir: data_dir.clone(),
        cache_dir: cache_dir.clone(),
    });

    let addons = addons::Addons::new(core.clone());
    let torrents = Arc::new(torrent::Torrents::new(cache_dir.join("torrents")));
    let player = player::Player::new(core.clone(), torrents);
    let hue = hue::Hue::new(core.clone());
    #[cfg(desktop)]
    let tobii = tobii::Tobii::new(core.clone());
    #[cfg(desktop)]
    if settings.tobii.enabled && settings.general.experimental {
        let _ = tobii.start();
    }

    let st = state::AppState {
        core: core.clone(),
        addons: addons.clone(),
        player,
        hue,
        #[cfg(desktop)]
        tobii,
    };
    state::set(st.clone());

    // Background: defaults on first run, then keep the account in sync.
    let a2 = addons.clone();
    let c2 = core.clone();
    *addons::builtin::TOP_COUNTRY.write() = core.settings.read().integrations.motn_country.clone();
    crate::rt::spawn(async move {
        // Top 10 follows the country the app is in (where Movie of
        // the Night has one), unless picked by hand in settings
        if c2.db.get::<bool>("motn_country_manual") != Some(true) {
            let geo = c2.http.client().get(format!("{}/geo", crew::CREW_URL.trim_end_matches('/'))).timeout(std::time::Duration::from_secs(8)).send().await;
            if let Ok(r) = geo {
                if let Ok(v) = r.json::<serde_json::Value>().await {
                    let cc = v.get("country").and_then(|c| c.as_str()).unwrap_or("").to_lowercase();
                    if addons::builtin::MOTN_COUNTRIES.contains(&cc.as_str()) && cc != c2.settings.read().integrations.motn_country {
                        c2.settings.write().integrations.motn_country = cc.clone();
                        let snapshot = c2.settings.read().clone();
                        let _ = c2.db.set("settings", &snapshot);
                    }
                }
            }
        }
        *addons::builtin::TOP_COUNTRY.write() = c2.settings.read().integrations.motn_country.clone();
        a2.ensure_defaults().await;
        c2.emit("addons-changed", ());
        loop {
            if c2.settings.read().integrations.stremio_sync && c2.db.secret("stremio_auth").is_some() {
                if let Ok((pulled, _)) = integrations::stremio::sync_library(&c2).await {
                    if pulled > 0 {
                        c2.emit("library-changed", ());
                    }
                }
            }
            // Simkl: one tiny /sync/activities call; real fetches only
            // when something moved.
            if integrations::simkl_sync::sync(&c2).await.unwrap_or(false) {
                c2.emit("library-changed", ());
            }
            // my page on the couch follows what I watch
            crew::publish_profile(&c2, false).await;
            // "new episode today" for the shows I'm watching
            alerts(&c2).await;
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
        }
    });
    Ok(st)
}

/// The app used to be called Lumen (`app.lumen.player`). On first launch as
/// couch, copy the old database over so addons, library, settings and keys
/// come along. Only ever copies; the old folder is left as it was.
fn migrate_from_lumen(data_dir: &std::path::Path) {
    let new_db = data_dir.join("couch.db");
    if new_db.exists() {
        return;
    }
    let Some(parent) = data_dir.parent() else { return };
    let old_dir = parent.join("app.lumen.player");
    let old_db = old_dir.join("lumen.db");
    if !old_db.exists() {
        return;
    }
    for ext in ["", "-wal", "-shm"] {
        let from = old_dir.join(format!("lumen.db{ext}"));
        if from.exists() {
            let _ = std::fs::copy(&from, data_dir.join(format!("couch.db{ext}")));
        }
    }
    log::info!("migrated data from Lumen");
}


/// New-episode alerts: once an hour, look at what's out today from the shows
/// you're watching (and films on your list), and tell you once per release:
/// a system notification on the computer, a toast inside couch.
async fn alerts(core: &Arc<Core>) {
    static LAST: parking_lot::Mutex<Option<std::time::Instant>> = parking_lot::Mutex::new(None);
    {
        let mut l = LAST.lock();
        if l.map_or(false, |t| t.elapsed() < std::time::Duration::from_secs(3600)) {
            return;
        }
        *l = Some(std::time::Instant::now());
    }
    if !core.settings.read().general.episode_alerts {
        return;
    }
    let mine = commands::calendar_mine(core);
    if mine.is_empty() {
        return;
    }
    let today = integrations::tmdb::today_str();
    let cal = integrations::tmdb::calendar(core, mine, vec![], false).await;
    let mut told: Vec<String> = core.db.get::<Vec<String>>("alerts_told").unwrap_or_default();
    // first run: don't announce what was already out
    let first = core.db.get::<bool>("alerts_seeded") != Some(true);
    let mut fresh = vec![];
    for t in cal.iter().filter(|t| t["date"].as_str() == Some(today.as_str())) {
        let key = format!("{}|{}|{}", t["id"].as_str().unwrap_or(""), t["label"].as_str().unwrap_or(""), today);
        if !told.contains(&key) {
            told.push(key);
            fresh.push(t.clone());
        }
    }
    if told.len() > 300 {
        told.drain(..told.len() - 300);
    }
    let _ = core.db.set("alerts_told", &told);
    let _ = core.db.set("alerts_seeded", &true);
    if first {
        return;
    }
    for t in fresh.into_iter().take(4) {
        let name = t["name"].as_str().unwrap_or("something").to_string();
        let label = t["label"].as_str().unwrap_or("").to_string();
        let ep = t["episode"].as_str().unwrap_or("").to_string();
        let body = if t["type"] == "series" {
            if ep.is_empty() { format!("{label} is out today") } else { format!("{label} · {ep} is out today") }
        } else {
            format!("out today {label}")
        };
        core.host.notify(&name, &body);
        core.emit("episode-alert", serde_json::json!({ "name": name, "body": body, "id": t.get("imdb").cloned().unwrap_or(t["id"].clone()), "type": t["type"] }));
    }
}
