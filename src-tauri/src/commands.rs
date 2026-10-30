//! Everything the UI can ask for. Thin: each command is a call into the
//! subsystem that owns the behaviour.

use crate::addons::{health, AddonDescriptor, CatalogRef};
use crate::error::{AppError, AppResult};
use crate::integrations::{ai, motn, simkl, stremio, tidb, tmdb};
use crate::player::PlayRequest;
use crate::settings::Settings;
use crate::state::AppState;
use crate::{images, library, streams};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};

type S<'a> = State<'a, AppState>;

// ------------------------------------------------------------------ app

#[tauri::command]
pub fn app_info(s: S<'_>) -> Value {
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "native": s.player.native(),
        "platform": std::env::consts::OS,
        "dataDir": s.core.data_dir,
    })
}

#[tauri::command]
pub fn settings_get(s: S<'_>) -> Settings {
    s.core.settings()
}

#[tauri::command]
pub fn settings_set(s: S<'_>, settings: Settings) -> AppResult<()> {
    let prev = s.core.settings();
    s.core.db.set("settings", &settings)?;
    if prev.network.doh != settings.network.doh || prev.network.doh_provider != settings.network.doh_provider {
        s.core.http.reconfigure(settings.network.doh.then_some(settings.network.doh_provider.as_str()));
    }
    *s.core.settings.write() = settings.clone();
    #[cfg(desktop)]
    {
        if settings.tobii.enabled && !prev.tobii.enabled {
            let _ = s.tobii.start();
        } else if !settings.tobii.enabled && prev.tobii.enabled {
            s.tobii.stop();
        }
    }
    Ok(())
}

const SECRETS: &[&str] = &["tmdb", "simkl_client_id", "simkl_token", "anthropic", "tidb", "motn", "art_worker", "stremio_auth", "hue_username"];

#[tauri::command]
pub fn secret_set(s: S<'_>, name: String, value: Option<String>) -> AppResult<()> {
    if !SECRETS.contains(&name.as_str()) {
        return Err(AppError::msg("unknown secret"));
    }
    s.core.db.set_secret(&name, value.as_deref().map(|v| v.trim()))
}

/// Which secrets are set -- never their values.
#[tauri::command]
pub fn secrets_status(s: S<'_>) -> Value {
    let mut m = serde_json::Map::new();
    for k in SECRETS {
        m.insert(k.to_string(), json!(s.core.db.secret(k).is_some()));
    }
    m.insert("simklUser".into(), s.core.db.get::<Value>("simkl_user").unwrap_or(Value::Null));
    m.insert("stremioUser".into(), s.core.db.get::<Value>("stremio_user").unwrap_or(Value::Null));
    Value::Object(m)
}

#[tauri::command]
pub async fn open_external(app: AppHandle, url: String) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(url, None::<&str>).map_err(|e| AppError::msg(e.to_string()))
}

// --------------------------------------------------------------- addons

#[tauri::command]
pub fn addons_list(s: S<'_>) -> Vec<AddonDescriptor> {
    s.addons.effective()
}

#[tauri::command]
pub async fn addon_preview(s: S<'_>, url: String) -> AppResult<AddonDescriptor> {
    s.addons.fetch_descriptor(&url).await
}

#[tauri::command]
pub async fn addons_install(s: S<'_>, url: String) -> AppResult<AddonDescriptor> {
    let d = s.addons.install(&url).await?;
    push_addons_quietly(&s).await;
    Ok(d)
}

#[tauri::command]
pub async fn addons_uninstall(s: S<'_>, transport_url: String) -> AppResult<()> {
    s.addons.uninstall(&transport_url);
    push_addons_quietly(&s).await;
    Ok(())
}

#[tauri::command]
pub async fn addons_reorder(s: S<'_>, order: Vec<String>) -> AppResult<()> {
    s.addons.reorder(&order);
    push_addons_quietly(&s).await;
    Ok(())
}

async fn push_addons_quietly(s: &S<'_>) {
    if s.core.settings.read().integrations.stremio_sync && s.core.db.secret("stremio_auth").is_some() {
        let _ = stremio::push_addons(&s.core, &s.addons).await;
    }
}

#[tauri::command]
pub async fn addons_refresh(s: S<'_>) -> AppResult<()> {
    s.addons.refresh_manifests().await;
    Ok(())
}

#[tauri::command]
pub async fn addons_health(s: S<'_>) -> AppResult<Vec<health::Health>> {
    Ok(health::check_all(s.addons.clone()).await)
}

#[tauri::command]
pub fn addons_export(s: S<'_>) -> Value {
    json!({ "lumen": 1, "addons": s.addons.list().iter().map(|a| a.transport_url.clone()).collect::<Vec<_>>() })
}

#[tauri::command]
pub async fn addons_import(s: S<'_>, urls: Vec<String>) -> AppResult<usize> {
    let mut n = 0;
    for u in urls {
        if s.addons.install(&u).await.is_ok() {
            n += 1;
        }
    }
    Ok(n)
}

// ------------------------------------------------------------- browsing

#[tauri::command]
pub fn catalogs_list(s: S<'_>) -> Vec<CatalogRef> {
    s.addons.catalogs()
}

#[tauri::command]
pub async fn catalog_fetch(s: S<'_>, catalog: CatalogRef, extra: Vec<(String, String)>) -> AppResult<Vec<Value>> {
    s.addons.catalog(&catalog, &extra).await
}

#[tauri::command]
pub async fn meta_get(s: S<'_>, kind: String, id: String, hint: Option<String>) -> AppResult<Value> {
    // tmdb:* ids (from recommendation tiles) are resolved to IMDb first.
    let (kind, id) = resolve_tmdb_id(&s, &kind, &id).await?;
    s.addons.meta(&kind, &id, hint.as_deref()).await
}

async fn resolve_tmdb_id(s: &S<'_>, kind: &str, id: &str) -> AppResult<(String, String)> {
    if let Some(rest) = id.strip_prefix("tmdb:") {
        let mut parts = rest.split(':');
        let tk = parts.next().unwrap_or("movie");
        let tid: u64 = parts.next().and_then(|x| x.parse().ok()).ok_or_else(|| AppError::msg("bad tmdb id"))?;
        let imdb = tmdb::imdb_for_tmdb(&s.core, tk, tid).await?.ok_or_else(|| AppError::msg("No IMDb id for this title, so no addon can play it"))?;
        return Ok((if tk == "tv" { "series".into() } else { "movie".into() }, imdb));
    }
    Ok((kind.to_string(), id.to_string()))
}

#[tauri::command]
pub async fn tmdb_resolve(s: S<'_>, kind: String, id: String) -> AppResult<Value> {
    let (k, i) = resolve_tmdb_id(&s, &kind, &id).await?;
    Ok(json!({ "type": k, "id": i }))
}

#[tauri::command]
pub async fn meta_enrich(s: S<'_>, kind: String, id: String) -> AppResult<Value> {
    tmdb::enrich(&s.core, &kind, &id).await
}

#[tauri::command]
pub async fn trailer_get(s: S<'_>, kind: String, id: String) -> AppResult<Option<String>> {
    tmdb::trailer(&s.core, &kind, &id).await
}

#[tauri::command]
pub async fn person_get(s: S<'_>, name: String, tmdb_id: Option<u64>, adult: bool) -> AppResult<Value> {
    // Adult performers aren't on TMDB; addons that know them expose a
    // /performer/{name}.json route (your PDB Debrid addon does).
    let from_addons = || async {
        for a in s.addons.effective().iter().filter(|a| a.is_adult()) {
            let u = format!("{}/performer/{}.json", a.base_url(), urlencoding::encode(&name));
            if let Ok(v) = s.core.http.get_json::<Value>(&u, std::time::Duration::from_secs(8)).await {
                if v.get("name").is_some() || v.get("metas").is_some() {
                    return Some(json!({
                        "source": "addon",
                        "name": v.get("name").cloned().unwrap_or(json!(name)),
                        "bio": v.pointer("/profile/bio"),
                        "profile": v.pointer("/profile/image"),
                        "gallery": v.pointer("/profile/posters"),
                        "facts": v.pointer("/profile/facts"),
                        "aliases": v.pointer("/profile/aliases"),
                        "credits": v.get("metas").cloned().unwrap_or(json!([])),
                        "addon": a.transport_url,
                    }));
                }
            }
        }
        None
    };
    if adult {
        if let Some(v) = from_addons().await {
            return Ok(v);
        }
    }
    match tmdb::person(&s.core, &name, tmdb_id).await {
        Ok(v) => Ok(v),
        Err(e) => {
            if !adult {
                if let Some(v) = from_addons().await {
                    return Ok(v);
                }
            }
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn hero_items(s: S<'_>) -> AppResult<Vec<Value>> {
    if simkl::client_id(&s.core).is_some() {
        if let Ok(v) = simkl::trending(&s.core).await {
            if !v.is_empty() {
                return Ok(v);
            }
        }
    }
    // No Simkl: TMDB trending is a fine hero too.
    let def = &tmdb::CATALOGS[0];
    let (m, t) = tokio::join!(tmdb::catalog(&s.core, def, "movie", None, 0), tmdb::catalog(&s.core, def, "series", None, 0));
    let mut out = Vec::new();
    let (m, t) = (m.unwrap_or_default(), t.unwrap_or_default());
    for i in 0..10 {
        if let Some(x) = m.get(i) {
            out.push(x.clone());
        }
        if let Some(x) = t.get(i) {
            out.push(x.clone());
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn art_get(s: S<'_>, imdb: String) -> AppResult<Value> {
    motn::art(&s.core, &imdb).await
}

#[tauri::command]
pub async fn palette_get(s: S<'_>, url: String) -> AppResult<images::Palette> {
    images::palette(&s.core, &url).await
}

// --------------------------------------------------------------- search

#[tauri::command]
pub fn search_start(s: S<'_>, query: String, req_id: String) {
    let q = query.trim().to_string();
    if q.len() >= 2 {
        s.core.db.recent_add(&q);
    }
    tauri::async_runtime::spawn(s.addons.clone().search(q, req_id));
}

#[tauri::command]
pub async fn search_suggest(s: S<'_>, query: String) -> AppResult<Vec<Value>> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let first = q.chars().next().unwrap_or('a');
    let u = format!("https://v3.sg.media-imdb.com/suggestion/{first}/{}.json", urlencoding::encode(&q));
    let v: Value = s.core.http.get_json_cached(&u, std::time::Duration::from_secs(3600), std::time::Duration::from_secs(4)).await?;
    Ok(v.get("d")
        .and_then(|d| d.as_array())
        .into_iter()
        .flatten()
        .filter(|x| x.get("id").and_then(|i| i.as_str()).map_or(false, |i| i.starts_with("tt")))
        .take(8)
        .map(|x| {
            let q = x.get("qid").and_then(|q| q.as_str()).unwrap_or("");
            json!({
                "id": x["id"],
                "name": x["l"],
                "year": x["y"],
                "type": if q.contains("tv") { "series" } else { "movie" },
                "poster": x.pointer("/i/imageUrl"),
                "cast": x["s"],
            })
        })
        .collect())
}

#[tauri::command]
pub fn recents_get(s: S<'_>) -> Vec<String> {
    s.core.db.recents(12)
}

#[tauri::command]
pub fn recents_clear(s: S<'_>) {
    s.core.db.recents_clear()
}

// -------------------------------------------------------------- streams

#[tauri::command]
pub fn streams_fetch(s: S<'_>, kind: String, id: String, req_id: String) {
    tauri::async_runtime::spawn(s.addons.clone().streams(kind, id, req_id));
}

#[tauri::command]
pub async fn subtitles_fetch(s: S<'_>, kind: String, id: String) -> AppResult<Vec<Value>> {
    Ok(s.addons.subtitles(&kind, &id, &[]).await)
}

#[tauri::command]
pub async fn skip_segments(s: S<'_>, video_id: String, duration_ms: Option<u64>) -> AppResult<Value> {
    tidb::segments(&s.core, &video_id, duration_ms).await
}

// --------------------------------------------------------------- player

#[tauri::command]
pub async fn player_open(s: S<'_>, req: PlayRequest) -> AppResult<crate::player::OpenResult> {
    s.player.open(req).await
}

#[tauri::command]
pub fn player_cmd(s: S<'_>, cmd: String, args: Option<Value>) -> AppResult<Value> {
    s.player.command(&cmd, &args.unwrap_or(Value::Null))
}

#[tauri::command]
pub async fn player_stop(s: S<'_>) -> AppResult<()> {
    s.player.stop().await
}

#[tauri::command]
pub fn player_state(s: S<'_>) -> Value {
    s.player.state()
}

#[tauri::command]
pub fn player_tracks(s: S<'_>) -> Value {
    s.player.tracks()
}

#[tauri::command]
pub fn player_report(s: S<'_>, pos: f64, duration: f64, paused: bool) {
    s.player.report(pos, duration, paused)
}

/// Next episode, same source: no picker, no wait.
#[tauri::command]
pub async fn player_next(s: S<'_>, meta: Value, video_id: String, previous: Value, episode_title: Option<String>) -> AppResult<Value> {
    let kind = meta.get("type").and_then(|t| t.as_str()).unwrap_or("series").to_string();
    let candidates = s.addons.streams_collect(&kind, &video_id).await;
    let same = s.core.settings.read().playback.binge_same_source;
    let pick = if same { streams::pick_same_source(&candidates, &previous) } else { streams::best(&candidates) };
    let Some(stream) = pick.cloned() else { return Err(AppError::msg("No streams for the next episode")) };
    let r = s.player.open(PlayRequest { stream: stream.clone(), meta, video_id, start_at: None, episode_title }).await?;
    Ok(json!({ "open": r, "stream": stream }))
}

#[tauri::command]
pub fn torrent_stats(s: S<'_>) -> Value {
    s.player.torrents.stats()
}

// -------------------------------------------------------------- library

#[tauri::command]
pub fn library_list(s: S<'_>) -> AppResult<Vec<Value>> {
    library::all(&s.core)
}

#[tauri::command]
pub fn library_item(s: S<'_>, id: String) -> Option<Value> {
    library::get(&s.core, &id)
}

#[tauri::command]
pub async fn library_add(s: S<'_>, meta: Value) -> AppResult<Value> {
    let item = library::add(&s.core, &meta)?;
    let kind = meta["type"].as_str().unwrap_or("movie").to_string();
    let id = meta["id"].as_str().unwrap_or("").to_string();
    let core = s.core.clone();
    tauri::async_runtime::spawn(async move {
        let _ = simkl::watchlist(&core, &kind, &id, true).await;
    });
    Ok(item)
}

#[tauri::command]
pub async fn library_remove(s: S<'_>, id: String, kind: String) -> AppResult<()> {
    library::remove(&s.core, &id)?;
    let core = s.core.clone();
    tauri::async_runtime::spawn(async move {
        let _ = simkl::watchlist(&core, &kind, &id, false).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn library_set_watched(s: S<'_>, meta: Value, video_id: Option<String>, watched: bool) -> AppResult<Value> {
    let item = library::set_watched(&s.core, &meta, video_id.as_deref(), watched)?;
    if simkl::connected(&s.core) {
        let kind = meta["type"].as_str().unwrap_or("movie").to_string();
        let id = video_id.unwrap_or_else(|| meta["id"].as_str().unwrap_or("").to_string());
        let core = s.core.clone();
        tauri::async_runtime::spawn(async move {
            let _ = simkl::set_watched(&core, &kind, &id, watched).await;
        });
    }
    Ok(item)
}

#[tauri::command]
pub fn continue_watching(s: S<'_>) -> AppResult<Vec<Value>> {
    library::continue_watching(&s.core)
}

#[tauri::command]
pub fn cw_dismiss(s: S<'_>, id: String) -> AppResult<()> {
    library::dismiss(&s.core, &id)
}

// ------------------------------------------------------------- accounts

#[tauri::command]
pub async fn stremio_login(s: S<'_>, email: String, password: String) -> AppResult<Value> {
    let u = stremio::login(&s.core, &email, &password).await?;
    let _ = stremio::pull_addons(&s.core, &s.addons).await;
    let _ = stremio::sync_library(&s.core).await;
    Ok(u)
}

#[tauri::command]
pub async fn stremio_link_start(s: S<'_>) -> AppResult<Value> {
    stremio::link_start(&s.core).await
}

#[tauri::command]
pub async fn stremio_link_poll(s: S<'_>, code: String) -> AppResult<bool> {
    if !stremio::link_poll(&s.core, &code).await? {
        return Ok(false);
    }
    let _ = stremio::pull_addons(&s.core, &s.addons).await;
    let _ = stremio::sync_library(&s.core).await;
    Ok(true)
}

#[tauri::command]
pub fn stremio_logout(s: S<'_>) -> AppResult<()> {
    stremio::logout(&s.core)
}

#[tauri::command]
pub async fn stremio_sync(s: S<'_>, pull_addons: bool) -> AppResult<Value> {
    if pull_addons {
        stremio::pull_addons(&s.core, &s.addons).await?;
    }
    let (pulled, pushed) = stremio::sync_library(&s.core).await?;
    Ok(json!({ "pulled": pulled, "pushed": pushed }))
}

#[tauri::command]
pub async fn simkl_pin_start(s: S<'_>) -> AppResult<Value> {
    simkl::pin_start(&s.core).await
}

#[tauri::command]
pub async fn simkl_pin_poll(s: S<'_>, code: String) -> AppResult<bool> {
    simkl::pin_poll(&s.core, &code).await
}

#[tauri::command]
pub fn simkl_logout(s: S<'_>) -> AppResult<()> {
    s.core.db.set_secret("simkl_token", None)?;
    s.core.db.del("simkl_user")
}

#[tauri::command]
pub async fn simkl_rate(s: S<'_>, kind: String, id: String, rating: Option<u8>) -> AppResult<()> {
    simkl::rate(&s.core, &kind, &id, rating).await
}

#[tauri::command]
pub async fn simkl_item_status(s: S<'_>, kind: String, id: String) -> AppResult<Value> {
    simkl::status_of(&s.core, &kind, &id).await
}

// ------------------------------------------------------------------- AI

#[tauri::command]
pub async fn ai_vibe(s: S<'_>, query: String) -> AppResult<Value> {
    ai::vibe(&s.core, &query).await
}

#[tauri::command]
pub async fn ai_prompts(s: S<'_>) -> AppResult<Value> {
    ai::prompts(&s.core).await
}

// ------------------------------------------------------------------ Hue

#[tauri::command]
pub async fn hue_discover(s: S<'_>) -> AppResult<Value> {
    s.hue.discover().await
}

#[tauri::command]
pub async fn hue_pair(s: S<'_>, ip: String) -> AppResult<Value> {
    s.hue.pair(&ip).await
}

#[tauri::command]
pub async fn hue_overview(s: S<'_>) -> AppResult<Value> {
    s.hue.overview().await
}

#[tauri::command]
pub async fn hue_identify(s: S<'_>) -> AppResult<()> {
    s.hue.identify().await
}

#[tauri::command]
pub async fn hue_test(s: S<'_>, on: bool) -> AppResult<()> {
    s.hue.on_playback(on).await;
    Ok(())
}

#[tauri::command]
pub async fn hue_mood(s: S<'_>, url: String) -> AppResult<()> {
    if !s.hue.enabled() {
        return Ok(());
    }
    let p = images::palette(&s.core, &url).await?;
    s.hue.mood(p.colors).await
}

// ---------------------------------------------------------------- Tobii

#[tauri::command]
pub fn tobii_status(s: S<'_>) -> Value {
    #[cfg(desktop)]
    {
        return json!({ "status": s.tobii.status(), "dll": s.tobii.find_dll() });
    }
    #[cfg(not(desktop))]
    {
        let _ = s;
        json!({ "status": "unsupported" })
    }
}

#[tauri::command]
pub fn tobii_start(s: S<'_>) -> AppResult<()> {
    #[cfg(desktop)]
    {
        return s.tobii.start();
    }
    #[cfg(not(desktop))]
    {
        let _ = s;
        Err(AppError::msg("Eye tracking needs the desktop app"))
    }
}

#[tauri::command]
pub fn tobii_stop(s: S<'_>) {
    #[cfg(desktop)]
    s.tobii.stop();
    #[cfg(not(desktop))]
    let _ = s;
}

// --------------------------------------------------------------- window

#[tauri::command]
pub fn window_action(app: AppHandle, action: String) -> AppResult<()> {
    #[cfg(desktop)]
    {
        let w = app.get_webview_window("main").ok_or_else(|| AppError::msg("no window"))?;
        match action.as_str() {
            "minimize" => w.minimize()?,
            "maximize" => {
                if w.is_maximized()? {
                    w.unmaximize()?
                } else {
                    w.maximize()?
                }
            }
            "close" => w.close()?,
            "fullscreen" => w.set_fullscreen(!w.is_fullscreen()?)?,
            "exit-fullscreen" => w.set_fullscreen(false)?,
            "drag" => w.start_dragging()?,
            _ => {}
        }
    }
    #[cfg(mobile)]
    let _ = (app, action);
    Ok(())
}

/// Picture-in-picture for a native player is just a small, always-on-top,
/// borderless window in the corner -- mpv keeps rendering into it.
#[tauri::command]
pub fn window_pip(app: AppHandle, on: bool) -> AppResult<()> {
    #[cfg(desktop)]
    {
        use tauri::{LogicalPosition, LogicalSize, Position, Size};
        let w = app.get_webview_window("main").ok_or_else(|| AppError::msg("no window"))?;
        let core = crate::state::get(&app).core.clone();
        if on {
            if w.is_fullscreen()? {
                w.set_fullscreen(false)?;
            }
            let scale = w.scale_factor()?;
            let pos = w.outer_position()?.to_logical::<f64>(scale);
            let size = w.inner_size()?.to_logical::<f64>(scale);
            core.db.set("pip_restore", &json!({ "x": pos.x, "y": pos.y, "w": size.width, "h": size.height, "max": w.is_maximized()? }))?;
            if w.is_maximized()? {
                w.unmaximize()?;
            }
            let (pw, ph) = (480.0, 270.0);
            let mon = w.current_monitor()?.ok_or_else(|| AppError::msg("no monitor"))?;
            let ms = mon.size().to_logical::<f64>(mon.scale_factor());
            let mp = mon.position().to_logical::<f64>(mon.scale_factor());
            w.set_always_on_top(true)?;
            w.set_size(Size::Logical(LogicalSize::new(pw, ph)))?;
            w.set_position(Position::Logical(LogicalPosition::new(mp.x + ms.width - pw - 28.0, mp.y + ms.height - ph - 72.0)))?;
        } else {
            w.set_always_on_top(false)?;
            if let Some(r) = core.db.get::<Value>("pip_restore") {
                w.set_size(Size::Logical(LogicalSize::new(r["w"].as_f64().unwrap_or(1280.0), r["h"].as_f64().unwrap_or(800.0))))?;
                w.set_position(Position::Logical(LogicalPosition::new(r["x"].as_f64().unwrap_or(80.0), r["y"].as_f64().unwrap_or(80.0))))?;
                if r["max"].as_bool().unwrap_or(false) {
                    w.maximize()?;
                }
            }
        }
    }
    #[cfg(mobile)]
    let _ = (app, on);
    Ok(())
}

// --------------------------------------------------------------- themes

#[tauri::command]
pub fn themes_user(s: S<'_>) -> Vec<Value> {
    let dir = s.core.data_dir.join("themes");
    let _ = std::fs::create_dir_all(&dir);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            if let Ok(txt) = std::fs::read_to_string(&p) {
                if let Ok(mut v) = serde_json::from_str::<Value>(&txt) {
                    // A sibling .css file with the same name is appended.
                    if let Ok(css) = std::fs::read_to_string(p.with_extension("css")) {
                        v["css"] = json!(format!("{}\n{}", v.get("css").and_then(|c| c.as_str()).unwrap_or(""), css));
                    }
                    v["user"] = json!(true);
                    out.push(v);
                }
            }
        }
    }
    out
}

#[tauri::command]
pub fn theme_save(s: S<'_>, theme: Value) -> AppResult<()> {
    let id = theme.get("id").and_then(|i| i.as_str()).ok_or_else(|| AppError::msg("theme needs an id"))?;
    let safe: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    let dir = s.core.data_dir.join("themes");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{safe}.json")), serde_json::to_string_pretty(&theme)?)?;
    Ok(())
}

#[tauri::command]
pub async fn themes_open_dir(app: AppHandle, s: S<'_>) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = s.core.data_dir.join("themes");
    std::fs::create_dir_all(&dir)?;
    app.opener().open_path(dir.to_string_lossy().to_string(), None::<&str>).map_err(|e| AppError::msg(e.to_string()))
}

// ---------------------------------------------------------------- cache

#[tauri::command]
pub async fn cache_info(s: S<'_>) -> AppResult<Value> {
    Ok(json!({ "images": images::cache_size(&s.core).await }))
}

#[tauri::command]
pub async fn cache_clear(s: S<'_>) -> AppResult<()> {
    images::clear(&s.core).await?;
    s.core.db.cache_clear()
}

// ------------------------------------------------------------ migration

#[tauri::command]
pub async fn import_stremio_enhanced(s: S<'_>) -> AppResult<Value> {
    crate::migrate::import(&s.core)
}
