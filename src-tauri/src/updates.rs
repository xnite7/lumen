//! Updates: new versions of couch come from the crew server, which hands
//! them out only to members of the midnite server (the same session as the
//! couch). Signed with couch's own key, so nothing else can pose as one.

use crate::core::Core;
use crate::error::{AppError, AppResult};
use serde_json::{json, Value};

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
pub async fn check(app: &tauri::AppHandle, core: &Core) -> AppResult<Value> {
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
pub async fn install(app: &tauri::AppHandle, core: &Core) -> AppResult<()> {
    #[cfg(desktop)]
    {
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
        let _ = (app, core);
        Err(AppError::msg("on iPhone, updates come through SideStore"))
    }
}
