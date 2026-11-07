//! couch setup: a small window in couch's own style that installs couch.
//! The real installer (NSIS) runs silently behind it; this is the face.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde_json::json;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

static PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.exe"));

fn installed_exe() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    ["couch\\couch.exe", "couch\\midnite couch.exe"].iter().map(|p| base.join(p)).find(|p| p.exists())
}

/// Write the installer out, run it silently, report back.
#[tauri::command]
async fn install(app: AppHandle) -> Result<(), String> {
    if PAYLOAD.is_empty() {
        return Err("this setup was built without couch inside".into());
    }
    let path = std::env::temp_dir().join("couch-install.exe");
    std::fs::write(&path, PAYLOAD).map_err(|e| e.to_string())?;
    let h = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let status = std::process::Command::new(&path).arg("/S").status();
        let _ = std::fs::remove_file(&path);
        let ok = matches!(status, Ok(s) if s.success());
        let _ = h.emit("installed", json!({ "ok": ok, "error": if ok { None } else { Some(format!("{status:?}")) } }));
    });
    Ok(())
}

#[tauri::command]
fn already_installed() -> bool {
    installed_exe().is_some()
}

#[tauri::command]
fn open_couch(app: AppHandle) -> Result<(), String> {
    let exe = installed_exe().ok_or("couch isn't where it should be")?;
    std::process::Command::new(exe).spawn().map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![install, already_installed, open_couch])
        .run(tauri::generate_context!())
        .expect("couch setup couldn't start");
}
