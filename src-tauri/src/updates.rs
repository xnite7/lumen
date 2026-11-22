//! Updates: new versions of couch come from the crew server, which hands
//! them out only to members of the midnite server (the same session as the
//! couch). Signed with couch's own key, so nothing else can pose as one.

use crate::core::Core;
use crate::error::{AppError, AppResult};
use serde_json::{json, Value};

#[cfg(feature = "tauri-app")]
mod imp {
use super::*;

#[cfg(desktop)]
async fn find(app: &tauri::AppHandle, core: &Core) -> AppResult<Option<tauri_plugin_updater::Update>> {
    use tauri_plugin_updater::UpdaterExt;
    let _ = crate::crew::login(core).await;
    let session = core.db.secret("crew_session").ok_or_else(|| AppError::msg("sign in with Discord to get updates"))?;
    let up = app
        .updater_builder()
        .header("authorization", format!("Bearer {session}"))
        .map_err(|e| AppError::msg(e.to_string()))?
        .build()
        .map_err(|e| AppError::msg(e.to_string()))?;
    up.check().await.map_err(|e| AppError::msg(format!("couldn't check for updates: {e}")))
}

/// { current, available: version | null, notes }
pub async fn check(core: &Core) -> AppResult<Value> {
    let app = &crate::tauri_host::app();
    let current = app.package_info().version.to_string();
    #[cfg(desktop)]
    {
        let u = find(app, core).await?;
        return Ok(json!({ "current": current, "available": u.as_ref().map(|u| u.version.clone()), "notes": u.and_then(|u| u.body) }));
    }
    #[cfg(not(desktop))]
    {
        let _ = core;
        Ok(json!({ "current": current, "available": null }))
    }
}

/// Download, install and restart into the new version.
pub async fn install(core: &Core) -> AppResult<()> {
    #[cfg(desktop)]
    {
        let app = &crate::tauri_host::app();
        let Some(u) = find(app, core).await? else { return Ok(()) };
        let h = app.clone();
        u.download_and_install(
            move |got, total| {
                use tauri::Emitter;
                let _ = h.emit("update-progress", json!({ "got": got, "total": total }));
            },
            || {},
        )
        .await
        .map_err(|e| AppError::msg(format!("the update didn't install: {e}")))?;
        app.restart();
    }
    #[cfg(not(desktop))]
    {
        let _ = core;
        Err(AppError::msg("on iPhone, updates come through SideStore"))
    }
}


/// Downloaded now, installed when couch closes ("when I close couch").
#[cfg(desktop)]
static PENDING: parking_lot::Mutex<Option<(tauri_plugin_updater::Update, Vec<u8>)>> = parking_lot::Mutex::new(None);

/// Fetch the update in the background and keep it for the moment the app closes.
pub async fn install_on_exit(core: &Core) -> AppResult<()> {
    #[cfg(desktop)]
    {
        let app = &crate::tauri_host::app();
        let Some(u) = find(app, core).await? else { return Ok(()) };
        let h = app.clone();
        let bytes = u
            .download(
                move |got, total| {
                    use tauri::Emitter;
                    let _ = h.emit("update-progress", json!({ "got": got, "total": total }));
                },
                || {},
            )
            .await
            .map_err(|e| AppError::msg(format!("the update didn't download: {e}")))?;
        *PENDING.lock() = Some((u, bytes));
        Ok(())
    }
    #[cfg(not(desktop))]
    {
        let _ = core;
        Err(AppError::msg("on iPhone, updates come through SideStore"))
    }
}

/// On close: put the waiting update in (the installer takes it from there).
pub fn install_pending() {
    #[cfg(desktop)]
    if let Some((u, bytes)) = PENDING.lock().take() {
        if let Err(e) = u.install(bytes) {
            log::warn!("update on exit failed: {e}");
        }
    }
}
}
#[cfg(feature = "tauri-app")]
pub use imp::*;

// Without Tauri (the GPUI app) updates aren't wired yet.
#[cfg(not(feature = "tauri-app"))]
pub async fn check(_core: &Core) -> AppResult<Value> {
    Ok(json!({ "current": env!("CARGO_PKG_VERSION"), "available": null }))
}
#[cfg(not(feature = "tauri-app"))]
pub async fn install(_core: &Core) -> AppResult<()> {
    Err(AppError::msg("updates aren't in this build yet"))
}
#[cfg(not(feature = "tauri-app"))]
pub async fn install_on_exit(_core: &Core) -> AppResult<()> {
    Err(AppError::msg("updates aren't in this build yet"))
}
#[cfg(not(feature = "tauri-app"))]
pub fn install_pending() {}
