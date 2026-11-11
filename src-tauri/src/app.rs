//! The Tauri app: webview window, plugins, the img:// scheme and the
//! command table the Svelte UI calls.
use crate::*;
use crate::host::Host;
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
            let path = request.uri().path().to_string();
            tauri::async_runtime::spawn(async move {
                let Some(url) = images::decode_path(&path) else {
                    responder.respond(Response::builder().status(StatusCode::BAD_REQUEST).body(Vec::new()).unwrap());
                    return;
                };
                let core = state::get().core.clone();
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
            crate::rt::init(tauri::async_runtime::handle().inner().clone());
            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            let host = std::sync::Arc::new(tauri_host::TauriHost::new(handle.clone()));
            let st = engine::boot(host, data_dir, cache_dir).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            app.manage(st);

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
        .on_window_event(|_window, event| {
            #[cfg(desktop)]
            if let tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. } = event {
                player::refit_video(tauri_host::TauriHost.main_window());
            }
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Leave nothing behind: progress saved, Simkl told, torrent
                // cache wiped, lights restored.
                let p = state::player();
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
            commands::crew_line,
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
            commands::couch_lists_get,
            commands::couch_lists_set,
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
