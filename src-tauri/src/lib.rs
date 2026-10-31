mod addons;
mod commands;
mod core;
mod db;
mod error;
mod http;
mod hue;
mod images;
mod integrations;
mod library;
mod migrate;
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
                if a.starts_with("stremio://") || a.starts_with("lumen://") || a.starts_with("magnet:") {
                    let _ = app.emit("deeplink", a.clone());
                }
            }
        }));
        builder = builder.plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(tauri_plugin_window_state::StateFlags::all() & !tauri_plugin_window_state::StateFlags::VISIBLE)
                .build(),
        );
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_lumen_native::init())
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

            let db = Arc::new(db::Db::open(&data_dir.join("lumen.db"))?);
            db.cache_prune();
            let settings: settings::Settings = db.get("settings").unwrap_or_default();
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
            if settings.tobii.enabled {
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
            tauri::async_runtime::spawn(async move {
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
                    tokio::time::sleep(std::time::Duration::from_secs(600)).await;
                }
            });

            {
                use tauri_plugin_deep_link::DeepLinkExt;
                #[cfg(desktop)]
                let _ = app.deep_link().register_all();
                // iPhone too: lumen:// links (Simkl sign-in coming back,
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
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Leave nothing behind: progress saved, Simkl told, torrent
                // cache wiped, lights restored.
                let app = window.app_handle().clone();
                let p = state::player(&app);
                tauri::async_runtime::block_on(async move {
                    let _ = p.stop().await;
                });
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
            commands::simkl_oauth_start,
            commands::simkl_oauth_finish,
            commands::simkl_device_start,
            commands::simkl_device_poll,
            commands::simkl_rate,
            commands::simkl_item_status,
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
            commands::themes_user,
            commands::theme_save,
            commands::themes_open_dir,
            commands::cache_info,
            commands::cache_clear,
            commands::import_stremio_enhanced,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lumen");
}
