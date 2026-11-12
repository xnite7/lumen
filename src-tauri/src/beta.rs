//! The new couch (the GPUI app), handed out by the crew server like updates:
//! the webview app can install it next to itself, and the new app keeps
//! itself up to date the same way.
use crate::core::Core;
use crate::error::{AppError, AppResult};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where the new app lives: %LOCALAPPDATA%\midnite couch (new)
pub fn install_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("midnite couch (new)")
}

/// { version, size } of the newest build, if there is one.
pub async fn latest(core: &Core) -> AppResult<Value> {
    crate::crew::login(core).await?;
    let (url, auth) = crate::crew::server(core).ok_or_else(|| AppError::msg("sign in with Discord first"))?;
    let r = core.http.client().get(format!("{url}/v1/gpui/latest")).header("authorization", auth).timeout(Duration::from_secs(15)).send().await?;
    if !r.status().is_success() {
        return Err(AppError::msg("there's no new build yet"));
    }
    Ok(r.json().await?)
}

async fn fetch_zip(core: &Core) -> AppResult<Vec<u8>> {
    crate::crew::login(core).await?;
    let (url, auth) = crate::crew::server(core).ok_or_else(|| AppError::msg("sign in with Discord first"))?;
    let r = core.http.client().get(format!("{url}/v1/gpui/file")).header("authorization", auth).timeout(Duration::from_secs(600)).send().await?;
    if !r.status().is_success() {
        return Err(AppError::msg("the download didn't work"));
    }
    Ok(r.bytes().await?.to_vec())
}

/// Unpack into `dir`. A file that's in use (the running app) is moved
/// aside first; Windows lets you rename a running .exe.
fn unpack(bytes: &[u8], dir: &Path) -> AppResult<()> {
    std::fs::create_dir_all(dir)?;
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| AppError::msg(e.to_string()))?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| AppError::msg(e.to_string()))?;
        let Some(name) = f.enclosed_name() else { continue };
        let to = dir.join(name);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if f.is_dir() {
            std::fs::create_dir_all(&to)?;
            continue;
        }
        if to.exists() {
            let old = to.with_extension("old");
            let _ = std::fs::remove_file(&old);
            if std::fs::remove_file(&to).is_err() {
                std::fs::rename(&to, &old)?;
            }
        }
        let mut out = std::fs::File::create(&to)?;
        std::io::copy(&mut f, &mut out)?;
    }
    Ok(())
}

/// Leftovers from the last update (the old .exe that was running).
pub fn clean_old(dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().extension().map_or(false, |x| x == "old") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Download the new app into `dir` (replacing what's there).
pub async fn download_into(core: &Core, dir: &Path) -> AppResult<()> {
    let bytes = fetch_zip(core).await?;
    let dir = dir.to_path_buf();
    crate::rt::spawn_blocking(move || unpack(&bytes, &dir)).await.map_err(|e| AppError::msg(e.to_string()))??;
    Ok(())
}

/// Webview app: install the new couch next to this one, add shortcuts, open it.
pub async fn install_and_open(core: &Core) -> AppResult<()> {
    let dir = install_dir();
    download_into(core, &dir).await?;
    let exe = dir.join("couch.exe");
    #[cfg(target_os = "windows")]
    {
        // Start menu + desktop shortcuts, through the shell
        let ps = format!(
            "$s=New-Object -ComObject WScript.Shell; foreach($p in @([Environment]::GetFolderPath('Programs'),[Environment]::GetFolderPath('Desktop'))){{ $l=$s.CreateShortcut((Join-Path $p 'midnite couch (new).lnk')); $l.TargetPath='{0}'; $l.WorkingDirectory='{1}'; $l.Save() }}",
            exe.display(),
            dir.display()
        );
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("powershell").args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps]).creation_flags(0x0800_0000).status();
    }
    launch(&exe, &dir)?;
    Ok(())
}

/// Start the new couch. Its .exe names the live tile's package; if Windows
/// has that package in a broken state (the folder was deleted or moved, an
/// update half-applied) it refuses to start the .exe at all ("Install failed",
/// os error 156xx). Then the package is removed (it's re-made by itself on the
/// next start) and it's started again.
pub fn launch(exe: &std::path::Path, dir: &std::path::Path) -> std::io::Result<std::process::Child> {
    match std::process::Command::new(exe).current_dir(dir).spawn() {
        Ok(c) => Ok(c),
        #[cfg(target_os = "windows")]
        Err(e) if e.raw_os_error().map_or(false, |c| (15600..15700).contains(&c)) => {
            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", "Get-AppxPackage -Name Midnite.Couch | Remove-AppxPackage"])
                .creation_flags(0x0800_0000)
                .status();
            std::process::Command::new(exe).current_dir(dir).spawn()
        }
        Err(e) => Err(e),
    }
}
