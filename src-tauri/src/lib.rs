mod addons;
mod commands;
mod core;
mod db;
mod crew;
mod discord;
mod error;
mod http;
mod hue;
mod images;
mod integrations;
mod library;
mod migrate;
mod updates;
mod player;
mod relay;
mod settings;
mod state;
mod streams;
#[cfg(desktop)]
mod tobii;
mod torrent;

use crate::core::Core;
use parking_lot::RwLock;
use std::sync::Arc;
use tauri::http::{Response, StatusCode};
use tauri::{Emitter, Manager};

fn image_response(bytes: Vec<u8>, ct: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", ct)
        .header("Cache-Control", "public, max-age=31536000, immutable")
        .header("Access-Control-Allow-Origin", "*")
        .body(bytes)
        .unwrap()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,librqbit=warn,tracing=warn")).try_init();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        // Must be first: a second launch (e.g. clicking a stremio:// install
        // link) hands its URL to the running app instead of opening twice.
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
            for a in argv.iter().skip(1) {
                if a.starts_with("stremio://") || a.starts_with("couch://") || a.starts_with("lumen://") || a.starts_with("magnet:") {
                    let _ = app.emit("deeplink", a.clone());
                }
            }
        }));
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
        builder = builder.plugin(tauri_plugin_notification::init());
        builder = builder.plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(tauri_plugin_window_state::StateFlags::all() & !tauri_plugin_window_state::StateFlags::VISIBLE)
                .build(),
        );
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_couch_native::init())
        .plugin(tauri_plugin_deep_link::init())
        .register_asynchronous_uri_scheme_protocol("img", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_string();
            tauri::async_runtime::spawn(async move {
                let Some(url) = images::decode_path(&path) else {
                    responder.respond(Response::builder().status(StatusCode::BAD_REQUEST).body(Vec::new()).unwrap());
                    return;
                };
                let core = state::get(&app).core.clone();
                match images::load(&core, &url).await {
                    Ok((bytes, ct)) => responder.respond(image_response(bytes, ct)),
                    Err(_) => responder.respond(
                        Response::builder().status(StatusCode::NOT_FOUND).header("Access-Control-Allow-Origin", "*").body(Vec::new()).unwrap(),
                    ),
                }
            });
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
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
                app: handle.clone(),
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

            app.manage(state::AppState {
                core: core.clone(),
                addons: addons.clone(),
                player,
                hue,
                #[cfg(desktop)]
                tobii,
            });

            // Background: defaults on first run, then keep the account in sync.
            let a2 = addons.clone();
            let c2 = core.clone();
            *addons::builtin::TOP_COUNTRY.write() = core.settings.read().integrations.motn_country.clone();
            tauri::async_runtime::spawn(async move {
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
                let _ = c2.app.emit("addons-changed", ());
                loop {
                    if c2.settings.read().integrations.stremio_sync && c2.db.secret("stremio_auth").is_some() {
                        if let Ok((pulled, _)) = integrations::stremio::sync_library(&c2).await {
                            if pulled > 0 {
                                let _ = c2.app.emit("library-changed", ());
                            }
                        }
                    }
                    // Simkl: one tiny /sync/activities call; real fetches only
                    // when something moved.
                    if integrations::simkl_sync::sync(&c2).await.unwrap_or(false) {
                        let _ = c2.app.emit("library-changed", ());
                    }
                    // my page on the couch follows what I watch
                    crew::publish_profile(&c2, false).await;
                    // "new episode today" for the shows I'm watching
                    alerts(&c2).await;
                    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                }
            });

            {
                use tauri_plugin_deep_link::DeepLinkExt;
                #[cfg(desktop)]
                let _ = app.deep_link().register_all();
                // iPhone too: couch:// links (Simkl sign-in coming back,
                // addon installs) arrive here.
                let h = handle.clone();
                app.deep_link().on_open_url(move |ev| {
                    for u in ev.urls() {
                        let _ = h.emit("deeplink", u.to_string());
                    }
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(desktop)]
            if let tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. } = event {
                player::refit_video(window);
            }
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Leave nothing behind: progress saved, Simkl told, torrent
                // cache wiped, lights restored.
                let app = window.app_handle().clone();
                let p = state::player(&app);
                tauri::async_runtime::block_on(async move {
                    let _ = p.stop().await;
                });
                // "update when I close couch"
                updates::install_pending();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::settings_get,
            commands::settings_set,
            commands::secret_set,
            commands::secrets_status,
            commands::open_external,
            commands::addons_list,
            commands::addon_preview,
            commands::addons_install,
            commands::addons_uninstall,
            commands::addons_reorder,
            commands::addons_refresh,
            commands::addons_health,
            commands::addons_export,
            commands::addons_import,
            commands::catalogs_list,
            commands::catalog_fetch,
            commands::meta_get,
            commands::tmdb_resolve,
            commands::meta_enrich,
            commands::trailer_get,
            commands::person_get,
            commands::hero_items,
            commands::home_rows,
            commands::genre_trending,
            commands::update_check,
            commands::update_install,
            commands::update_on_exit,
            commands::discord_membership,
            commands::crew_ask,
            commands::crew_ask_status,
            commands::crew_inbox,
            commands::crew_cancel_ask,
            commands::crew_share_control,
            commands::crew_room_cmd,
            commands::crew_answer,
            commands::crew_room,
            commands::crew_leave,
            commands::art_get,
            commands::palette_get,
            commands::search_start,
            commands::search_suggest,
            commands::recents_get,
            commands::recents_clear,
            commands::streams_fetch,
            commands::subtitles_fetch,
            commands::skip_segments,
            commands::player_open,
            commands::player_cmd,
            commands::player_stop,
            commands::player_state,
            commands::player_tracks,
            commands::player_report,
            commands::player_next,
            commands::player_prefetch,
            commands::player_thumb,
            commands::torrent_stats,
            commands::library_list,
            commands::library_item,
            commands::library_add,
            commands::library_remove,
            commands::library_set_watched,
            commands::continue_watching,
            commands::cw_dismiss,
            commands::stremio_login,
            commands::stremio_link_start,
            commands::stremio_link_poll,
            commands::stremio_logout,
            commands::stremio_sync,
            commands::simkl_pin_start,
            commands::simkl_pin_poll,
            commands::simkl_logout,
            commands::simkl_sync,
            commands::crew_list,
            commands::net_speed,
            commands::discord_start,
            commands::discord_finish,
            commands::discord_logout,
            commands::crew_join,
            commands::crew_invite,
            commands::simkl_oauth_start,
            commands::simkl_oauth_finish,
            commands::simkl_device_start,
            commands::simkl_device_poll,
            commands::simkl_rate,
            commands::simkl_item_status,
            commands::local_ratings,
            commands::my_lists,
            commands::simkl_custom_lists,
            commands::simkl_list_items,
            commands::crew_social,
            commands::crew_profile_publish,
            commands::company_get,
            commands::wrapped_data,
            commands::calendar_get,
            commands::follows_list,
            commands::follow_toggle,
            commands::follow_feed,
            commands::share_link,
            commands::discord_me,
            commands::discord_rpc_status,
            commands::clip_url,
            commands::streaming_services,
            commands::did_you_mean,
            commands::scene_color,
            commands::simkl_episode_info,
            commands::ai_vibe,
            commands::ai_prompts,
            commands::hue_discover,
            commands::hue_pair,
            commands::hue_overview,
            commands::hue_identify,
            commands::hue_test,
            commands::hue_mood,
            commands::tobii_status,
            commands::tobii_start,
            commands::tobii_stop,
            commands::window_action,
            commands::window_pip,
            commands::cache_info,
            commands::cache_clear,
            commands::import_stremio_enhanced,
        ])
        .run(tauri::generate_context!())
        .expect("error while running couch");
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
        #[cfg(desktop)]
        {
            use tauri_plugin_notification::NotificationExt;
            let _ = core.app.notification().builder().title(&name).body(&body).show();
        }
        let _ = core.app.emit("episode-alert", serde_json::json!({ "name": name, "body": body, "id": t.get("imdb").cloned().unwrap_or(t["id"].clone()), "type": t["type"] }));
    }
}
