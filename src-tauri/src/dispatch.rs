//! The same commands the Svelte UI calls, by name with JSON arguments, for
//! the GPUI app. Generated from commands.rs (tools/gen_dispatch.py).
#![allow(unused_mut, clippy::all)]

use crate::addons::CatalogRef;
use crate::commands;
use crate::player::PlayRequest;
use crate::settings::Settings;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn arg<T: DeserializeOwned>(a: &Value, k: &str) -> Result<T, String> {
    serde_json::from_value(a.get(k).cloned().unwrap_or(Value::Null)).map_err(|e| format!("argument {k}: {e}"))
}
fn out<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

/// Run a command: `invoke("meta_get", json!({ "kind": "movie", "id": "tt.." }))`.
pub async fn invoke(cmd: &str, a: Value) -> Result<Value, String> {
    let st = crate::state::get();
    match cmd {
        "app_info" => {
            out(commands::app_info(st))
        }
        "settings_get" => {
            out(commands::settings_get(st))
        }
        "settings_set" => {
            let settings: Settings = arg(&a, "settings")?;
            out(commands::settings_set(st, settings).map_err(|e| e.to_string())?)
        }
        "secret_set" => {
            let name: String = arg(&a, "name")?;
            let value: Option<String> = arg(&a, "value")?;
            out(commands::secret_set(st, name, value).map_err(|e| e.to_string())?)
        }
        "secrets_status" => {
            out(commands::secrets_status(st))
        }
        "open_external" => {
            let url: String = arg(&a, "url")?;
            out(commands::open_external(url).await.map_err(|e| e.to_string())?)
        }
        "addons_list" => {
            out(commands::addons_list(st))
        }
        "addon_preview" => {
            let url: String = arg(&a, "url")?;
            out(commands::addon_preview(st, url).await.map_err(|e| e.to_string())?)
        }
        "addons_install" => {
            let url: String = arg(&a, "url")?;
            out(commands::addons_install(st, url).await.map_err(|e| e.to_string())?)
        }
        "addons_uninstall" => {
            let transport_url: String = arg(&a, "transportUrl")?;
            out(commands::addons_uninstall(st, transport_url).await.map_err(|e| e.to_string())?)
        }
        "addons_reorder" => {
            let order: Vec<String> = arg(&a, "order")?;
            out(commands::addons_reorder(st, order).await.map_err(|e| e.to_string())?)
        }
        "addons_refresh" => {
            out(commands::addons_refresh(st).await.map_err(|e| e.to_string())?)
        }
        "addons_health" => {
            out(commands::addons_health(st).await.map_err(|e| e.to_string())?)
        }
        "addons_export" => {
            out(commands::addons_export(st))
        }
        "addons_import" => {
            let urls: Vec<String> = arg(&a, "urls")?;
            out(commands::addons_import(st, urls).await.map_err(|e| e.to_string())?)
        }
        "catalogs_list" => {
            out(commands::catalogs_list(st))
        }
        "catalog_fetch" => {
            let catalog: CatalogRef = arg(&a, "catalog")?;
            let extra: Vec<(String, String)> = arg(&a, "extra")?;
            out(commands::catalog_fetch(st, catalog, extra).await.map_err(|e| e.to_string())?)
        }
        "meta_get" => {
            let kind: String = arg(&a, "kind")?;
            let id: String = arg(&a, "id")?;
            let hint: Option<String> = arg(&a, "hint")?;
            out(commands::meta_get(st, kind, id, hint).await.map_err(|e| e.to_string())?)
        }
        "tmdb_resolve" => {
            let kind: String = arg(&a, "kind")?;
            let id: String = arg(&a, "id")?;
            out(commands::tmdb_resolve(st, kind, id).await.map_err(|e| e.to_string())?)
        }
        "meta_enrich" => {
            let kind: String = arg(&a, "kind")?;
            let id: String = arg(&a, "id")?;
            out(commands::meta_enrich(st, kind, id).await.map_err(|e| e.to_string())?)
        }
        "trailer_get" => {
            let kind: String = arg(&a, "kind")?;
            let id: String = arg(&a, "id")?;
            out(commands::trailer_get(st, kind, id).await.map_err(|e| e.to_string())?)
        }
        "person_get" => {
            let name: String = arg(&a, "name")?;
            let tmdb_id: Option<u64> = arg(&a, "tmdbId")?;
            let adult: bool = arg(&a, "adult")?;
            out(commands::person_get(st, name, tmdb_id, adult).await.map_err(|e| e.to_string())?)
        }
        "update_check" => {
            out(commands::update_check(st).await.map_err(|e| e.to_string())?)
        }
        "update_install" => {
            out(commands::update_install(st).await.map_err(|e| e.to_string())?)
        }
        "update_on_exit" => {
            out(commands::update_on_exit(st).await.map_err(|e| e.to_string())?)
        }
        "genre_trending" => {
            let kind: String = arg(&a, "kind")?;
            let genre: String = arg(&a, "genre")?;
            out(commands::genre_trending(st, kind, genre).await.map_err(|e| e.to_string())?)
        }
        "home_rows" => {
            let scope: String = arg(&a, "scope")?;
            out(commands::home_rows(st, scope))
        }
        "hero_items" => {
            out(commands::hero_items(st).await.map_err(|e| e.to_string())?)
        }
        "art_get" => {
            let imdb: String = arg(&a, "imdb")?;
            out(commands::art_get(st, imdb).await.map_err(|e| e.to_string())?)
        }
        "palette_get" => {
            let url: String = arg(&a, "url")?;
            out(commands::palette_get(st, url).await.map_err(|e| e.to_string())?)
        }
        "search_start" => {
            let query: String = arg(&a, "query")?;
            let req_id: String = arg(&a, "reqId")?;
            out(commands::search_start(st, query, req_id))
        }
        "search_suggest" => {
            let query: String = arg(&a, "query")?;
            out(commands::search_suggest(st, query).await.map_err(|e| e.to_string())?)
        }
        "recents_get" => {
            out(commands::recents_get(st))
        }
        "recents_clear" => {
            out(commands::recents_clear(st))
        }
        "streams_fetch" => {
            let kind: String = arg(&a, "kind")?;
            let id: String = arg(&a, "id")?;
            let req_id: String = arg(&a, "reqId")?;
            out(commands::streams_fetch(st, kind, id, req_id))
        }
        "subtitles_fetch" => {
            let kind: String = arg(&a, "kind")?;
            let id: String = arg(&a, "id")?;
            out(commands::subtitles_fetch(st, kind, id).await.map_err(|e| e.to_string())?)
        }
        "skip_segments" => {
            let video_id: String = arg(&a, "videoId")?;
            let duration_ms: Option<u64> = arg(&a, "durationMs")?;
            out(commands::skip_segments(st, video_id, duration_ms).await.map_err(|e| e.to_string())?)
        }
        "player_open" => {
            let req: PlayRequest = arg(&a, "req")?;
            out(commands::player_open(st, req).await.map_err(|e| e.to_string())?)
        }
        "player_cmd" => {
            let cmd: String = arg(&a, "cmd")?;
            let args: Option<Value> = arg(&a, "args")?;
            out(commands::player_cmd(st, cmd, args).map_err(|e| e.to_string())?)
        }
        "player_stop" => {
            out(commands::player_stop(st).await.map_err(|e| e.to_string())?)
        }
        "player_state" => {
            out(commands::player_state(st))
        }
        "player_tracks" => {
            out(commands::player_tracks(st))
        }
        "player_report" => {
            let pos: f64 = arg(&a, "pos")?;
            let duration: f64 = arg(&a, "duration")?;
            let paused: bool = arg(&a, "paused")?;
            out(commands::player_report(st, pos, duration, paused))
        }
        "player_next" => {
            let meta: Value = arg(&a, "meta")?;
            let video_id: String = arg(&a, "videoId")?;
            let previous: Value = arg(&a, "previous")?;
            let episode_title: Option<String> = arg(&a, "episodeTitle")?;
            out(commands::player_next(st, meta, video_id, previous, episode_title).await.map_err(|e| e.to_string())?)
        }
        "player_prefetch" => {
            let kind: String = arg(&a, "kind")?;
            let video_id: String = arg(&a, "videoId")?;
            out(commands::player_prefetch(st, kind, video_id).await.map_err(|e| e.to_string())?)
        }
        "player_thumb" => {
            let t: f64 = arg(&a, "t")?;
            out(commands::player_thumb(t).await.map_err(|e| e.to_string())?)
        }
        "torrent_stats" => {
            out(commands::torrent_stats(st))
        }
        "library_list" => {
            out(commands::library_list(st).map_err(|e| e.to_string())?)
        }
        "library_item" => {
            let id: String = arg(&a, "id")?;
            out(commands::library_item(st, id))
        }
        "library_add" => {
            let meta: Value = arg(&a, "meta")?;
            out(commands::library_add(st, meta).await.map_err(|e| e.to_string())?)
        }
        "library_remove" => {
            let id: String = arg(&a, "id")?;
            let kind: String = arg(&a, "kind")?;
            out(commands::library_remove(st, id, kind).await.map_err(|e| e.to_string())?)
        }
        "library_set_watched" => {
            let meta: Value = arg(&a, "meta")?;
            let video_id: Option<String> = arg(&a, "videoId")?;
            let watched: bool = arg(&a, "watched")?;
            let all_videos: Option<Vec<String>> = arg(&a, "allVideos")?;
            out(commands::library_set_watched(st, meta, video_id, watched, all_videos).await.map_err(|e| e.to_string())?)
        }
        "continue_watching" => {
            out(commands::continue_watching(st).map_err(|e| e.to_string())?)
        }
        "cw_dismiss" => {
            let id: String = arg(&a, "id")?;
            out(commands::cw_dismiss(st, id).await.map_err(|e| e.to_string())?)
        }
        "simkl_sync" => {
            out(commands::simkl_sync(st).await.map_err(|e| e.to_string())?)
        }
        "stremio_login" => {
            let email: String = arg(&a, "email")?;
            let password: String = arg(&a, "password")?;
            out(commands::stremio_login(st, email, password).await.map_err(|e| e.to_string())?)
        }
        "stremio_link_start" => {
            out(commands::stremio_link_start(st).await.map_err(|e| e.to_string())?)
        }
        "stremio_link_poll" => {
            let code: String = arg(&a, "code")?;
            out(commands::stremio_link_poll(st, code).await.map_err(|e| e.to_string())?)
        }
        "stremio_logout" => {
            out(commands::stremio_logout(st).map_err(|e| e.to_string())?)
        }
        "stremio_sync" => {
            let pull_addons: bool = arg(&a, "pullAddons")?;
            out(commands::stremio_sync(st, pull_addons).await.map_err(|e| e.to_string())?)
        }
        "simkl_pin_start" => {
            out(commands::simkl_pin_start(st).await.map_err(|e| e.to_string())?)
        }
        "simkl_pin_poll" => {
            let code: String = arg(&a, "code")?;
            out(commands::simkl_pin_poll(st, code).await.map_err(|e| e.to_string())?)
        }
        "simkl_logout" => {
            out(commands::simkl_logout(st).await.map_err(|e| e.to_string())?)
        }
        "simkl_oauth_start" => {
            out(commands::simkl_oauth_start(st).await.map_err(|e| e.to_string())?)
        }
        "simkl_oauth_finish" => {
            let url: String = arg(&a, "url")?;
            out(commands::simkl_oauth_finish(st, url).await.map_err(|e| e.to_string())?)
        }
        "simkl_device_start" => {
            out(commands::simkl_device_start(st).await.map_err(|e| e.to_string())?)
        }
        "simkl_device_poll" => {
            out(commands::simkl_device_poll(st).await.map_err(|e| e.to_string())?)
        }
        "simkl_rate" => {
            let kind: String = arg(&a, "kind")?;
            let id: String = arg(&a, "id")?;
            let rating: Option<u8> = arg(&a, "rating")?;
            out(commands::simkl_rate(st, kind, id, rating).await.map_err(|e| e.to_string())?)
        }
        "local_ratings" => {
            out(commands::local_ratings(st))
        }
        "simkl_item_status" => {
            let kind: String = arg(&a, "kind")?;
            let id: String = arg(&a, "id")?;
            out(commands::simkl_item_status(st, kind, id).await.map_err(|e| e.to_string())?)
        }
        "my_lists" => {
            out(commands::my_lists(st))
        }
        "simkl_custom_lists" => {
            out(commands::simkl_custom_lists(st).await.map_err(|e| e.to_string())?)
        }
        "simkl_list_items" => {
            let id: u64 = arg(&a, "id")?;
            out(commands::simkl_list_items(st, id).await.map_err(|e| e.to_string())?)
        }
        "calendar_get" => {
            let hits: bool = arg(&a, "hits")?;
            out(commands::calendar_get(st, hits).await.map_err(|e| e.to_string())?)
        }
        "wrapped_data" => {
            let year: i32 = arg(&a, "year")?;
            out(commands::wrapped_data(st, year))
        }
        "company_get" => {
            let id: u64 = arg(&a, "id")?;
            out(commands::company_get(st, id).await.map_err(|e| e.to_string())?)
        }
        "couch_lists_get" => {
            out(commands::couch_lists_get(st))
        }
        "couch_lists_set" => {
            let lists: Value = arg(&a, "lists")?;
            out(commands::couch_lists_set(st, lists).map_err(|e| e.to_string())?)
        }
        "follows_list" => {
            out(commands::follows_list(st))
        }
        "follow_toggle" => {
            let kind: String = arg(&a, "kind")?;
            let id: u64 = arg(&a, "id")?;
            let name: String = arg(&a, "name")?;
            let img: Option<String> = arg(&a, "img")?;
            out(commands::follow_toggle(st, kind, id, name, img).await.map_err(|e| e.to_string())?)
        }
        "follow_feed" => {
            out(commands::follow_feed(st).await.map_err(|e| e.to_string())?)
        }
        "crew_profile_publish" => {
            out(commands::crew_profile_publish(st).await.map_err(|e| e.to_string())?)
        }
        "crew_social" => {
            let method: String = arg(&a, "method")?;
            let path: String = arg(&a, "path")?;
            let body: Option<Value> = arg(&a, "body")?;
            out(commands::crew_social(st, method, path, body).await.map_err(|e| e.to_string())?)
        }
        "simkl_episode_info" => {
            let id: String = arg(&a, "id")?;
            out(commands::simkl_episode_info(st, id).await.map_err(|e| e.to_string())?)
        }
        "scene_color" => {
            out(commands::scene_color(st))
        }
        "did_you_mean" => {
            let query: String = arg(&a, "query")?;
            out(commands::did_you_mean(st, query).await.map_err(|e| e.to_string())?)
        }
        "streaming_services" => {
            out(commands::streaming_services(st).await.map_err(|e| e.to_string())?)
        }
        "clip_url" => {
            let id: String = arg(&a, "id")?;
            let title: Option<String> = arg(&a, "title")?;
            let year: Option<String> = arg(&a, "year")?;
            out(commands::clip_url(st, id, title, year).await.map_err(|e| e.to_string())?)
        }
        "discord_rpc_status" => {
            out(commands::discord_rpc_status())
        }
        "discord_me" => {
            out(commands::discord_me(st))
        }
        "share_link" => {
            let kind: String = arg(&a, "kind")?;
            let id: String = arg(&a, "id")?;
            let name: String = arg(&a, "name")?;
            out(commands::share_link(kind, id, name))
        }
        "ai_vibe" => {
            let query: String = arg(&a, "query")?;
            out(commands::ai_vibe(st, query).await.map_err(|e| e.to_string())?)
        }
        "ai_prompts" => {
            out(commands::ai_prompts(st).await.map_err(|e| e.to_string())?)
        }
        "hue_discover" => {
            out(commands::hue_discover(st).await.map_err(|e| e.to_string())?)
        }
        "hue_pair" => {
            let ip: String = arg(&a, "ip")?;
            out(commands::hue_pair(st, ip).await.map_err(|e| e.to_string())?)
        }
        "hue_overview" => {
            out(commands::hue_overview(st).await.map_err(|e| e.to_string())?)
        }
        "hue_identify" => {
            out(commands::hue_identify(st).await.map_err(|e| e.to_string())?)
        }
        "hue_test" => {
            let on: bool = arg(&a, "on")?;
            out(commands::hue_test(st, on).await.map_err(|e| e.to_string())?)
        }
        "hue_mood" => {
            let url: String = arg(&a, "url")?;
            out(commands::hue_mood(st, url).await.map_err(|e| e.to_string())?)
        }
        "tobii_status" => {
            out(commands::tobii_status(st))
        }
        "tobii_start" => {
            out(commands::tobii_start(st).map_err(|e| e.to_string())?)
        }
        "tobii_stop" => {
            out(commands::tobii_stop(st))
        }
        "window_action" => {
            let action: String = arg(&a, "action")?;
            out(commands::window_action(action).map_err(|e| e.to_string())?)
        }
        "window_pip" => {
            let on: bool = arg(&a, "on")?;
            out(commands::window_pip(on).map_err(|e| e.to_string())?)
        }
        "cache_info" => {
            out(commands::cache_info(st).await.map_err(|e| e.to_string())?)
        }
        "cache_clear" => {
            out(commands::cache_clear(st).await.map_err(|e| e.to_string())?)
        }
        "import_stremio_enhanced" => {
            out(commands::import_stremio_enhanced(st).await.map_err(|e| e.to_string())?)
        }
        "crew_list" => {
            out(commands::crew_list(st).await.map_err(|e| e.to_string())?)
        }
        "crew_ask" => {
            let to: String = arg(&a, "to")?;
            out(commands::crew_ask(st, to).await.map_err(|e| e.to_string())?)
        }
        "crew_ask_status" => {
            let id: String = arg(&a, "id")?;
            out(commands::crew_ask_status(st, id).await.map_err(|e| e.to_string())?)
        }
        "crew_share_control" => {
            let on: bool = arg(&a, "on")?;
            out(commands::crew_share_control(on))
        }
        "crew_room_cmd" => {
            let host: String = arg(&a, "host")?;
            let action: String = arg(&a, "action")?;
            let pos: Option<f64> = arg(&a, "pos")?;
            out(commands::crew_room_cmd(st, host, action, pos).await.map_err(|e| e.to_string())?)
        }
        "crew_cancel_ask" => {
            let id: String = arg(&a, "id")?;
            out(commands::crew_cancel_ask(st, id).await.map_err(|e| e.to_string())?)
        }
        "crew_inbox" => {
            out(commands::crew_inbox(st).await.map_err(|e| e.to_string())?)
        }
        "crew_answer" => {
            let id: String = arg(&a, "id")?;
            let accept: bool = arg(&a, "accept")?;
            out(commands::crew_answer(st, id, accept).await.map_err(|e| e.to_string())?)
        }
        "crew_room" => {
            let host: String = arg(&a, "host")?;
            out(commands::crew_room(st, host).await.map_err(|e| e.to_string())?)
        }
        "crew_line" => {
            let host: String = arg(&a, "host")?;
            out(commands::crew_line(st, host).await.map_err(|e| e.to_string())?)
        }
        "crew_leave" => {
            let host: String = arg(&a, "host")?;
            out(commands::crew_leave(st, host).await.map_err(|e| e.to_string())?)
        }
        "crew_join" => {
            let url: String = arg(&a, "url")?;
            let code: String = arg(&a, "code")?;
            out(commands::crew_join(st, url, code).await.map_err(|e| e.to_string())?)
        }
        "crew_invite" => {
            out(commands::crew_invite(st))
        }
        "discord_start" => {
            out(commands::discord_start(st).await.map_err(|e| e.to_string())?)
        }
        "discord_finish" => {
            let url: String = arg(&a, "url")?;
            out(commands::discord_finish(st, url).await.map_err(|e| e.to_string())?)
        }
        "discord_membership" => {
            out(commands::discord_membership(st).await.map_err(|e| e.to_string())?)
        }
        "discord_logout" => {
            out(commands::discord_logout(st).map_err(|e| e.to_string())?)
        }
        "net_speed" => {
            let fresh: Option<bool> = arg(&a, "fresh")?;
            out(commands::net_speed(st, fresh).await.map_err(|e| e.to_string())?)
        }
        _ => Err(format!("unknown command {cmd}")),
    }
}
