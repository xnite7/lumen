//! Removing the new couch properly, from Windows' "Apps" list or from
//! Settings: once couch has closed, a small script takes out the live tile
//! package, the shortcuts, the couch: links, the Apps entry and the folder.
//! Your lists, ratings and sign-ins stay (the old couch shares them).

const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\MidniteCouchNew";

/// The clean-up script, kept beside (not inside) couch's folder so it works
/// even when couch.exe itself can't start.
#[cfg(target_os = "windows")]
fn script_path() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|d| d.join("midnite couch (new) uninstall.ps1"))
}

#[cfg(target_os = "windows")]
fn write_script() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.display().to_string().replace('\'', "''");
    let me = script_path()?;
    let mes = me.display().to_string().replace('\'', "''");
    let script = format!(
        r#"
$ErrorActionPreference = 'SilentlyContinue'
# couch closes first (it's given a few seconds, then made to)
$c = Get-Process couch | Where-Object {{ $_.Path -like '{dir}*' }}
if ($c) {{ $c | Wait-Process -Timeout 8; Get-Process couch | Where-Object {{ $_.Path -like '{dir}*' }} | Stop-Process -Force; Start-Sleep -Milliseconds 600 }}
Get-AppxPackage -Name Midnite.Couch | Remove-AppxPackage
foreach ($p in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))) {{ Remove-Item (Join-Path $p 'midnite couch (new).lnk') }}
Remove-Item 'HKCU:\Software\Classes\couch' -Recurse
Remove-Item 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MidniteCouchNew' -Recurse
Set-Location $env:TEMP
Remove-Item '{dir}' -Recurse -Force
Remove-Item '{mes}'
"#
    );
    std::fs::write(&me, script).ok()?;
    Some(me)
}

/// List couch in Settings → Apps (and Control Panel), with its own uninstall.
pub fn register(version: &str) {
    #[cfg(target_os = "windows")]
    std::thread::spawn({
        let version = version.to_string();
        move || {
            let Ok(exe) = std::env::current_exe() else { return };
            let Some(dir) = exe.parent().map(|d| d.display().to_string()) else { return };
            let exe = exe.display().to_string();
            let Some(script) = write_script() else { return };
            let size_kb = std::fs::read_dir(&dir).map(|r| r.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum::<u64>() / 1024).unwrap_or(0);
            let vals: Vec<(&str, &str, String)> = vec![
                ("DisplayName", "REG_SZ", "midnite couch (new)".into()),
                ("DisplayVersion", "REG_SZ", version.clone()),
                ("Publisher", "REG_SZ", "midnite".into()),
                ("DisplayIcon", "REG_SZ", format!("{exe},0")),
                ("InstallLocation", "REG_SZ", dir.clone()),
                ("UninstallString", "REG_SZ", format!("powershell -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File \"{}\"", script.display())),
                ("NoModify", "REG_DWORD", "1".into()),
                ("NoRepair", "REG_DWORD", "1".into()),
                ("EstimatedSize", "REG_DWORD", size_kb.to_string()),
            ];
            use std::os::windows::process::CommandExt;
            for (name, ty, data) in vals {
                let _ = std::process::Command::new("reg").args(["add", KEY, "/v", name, "/t", ty, "/d", &data, "/f"]).creation_flags(0x0800_0000).status();
            }
        }
    });
    #[cfg(not(target_os = "windows"))]
    let _ = version;
}

/// Was couch started to remove itself (from the Apps list)?
pub fn requested() -> bool {
    std::env::args().any(|a| a == "--uninstall")
}

/// Hand the clean-up to the script, which waits for couch to close, then
/// removes everything. The caller quits right after.
pub fn run() {
    // the installed couch: Windows takes the package away (as Start's uninstall does)
    #[cfg(target_os = "windows")]
    if crate::packaged::is_packaged() {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", "Start-Sleep -Milliseconds 800; Get-AppxPackage -Name MidniteCouch | Remove-AppxPackage"])
            .creation_flags(0x0800_0000 | 0x0000_0008)
            .spawn();
        return;
    }
    #[cfg(target_os = "windows")]
    if let Some(path) = write_script() {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File", &path.display().to_string()])
            .current_dir(std::env::temp_dir())
            // detached: it outlives couch
            .creation_flags(0x0800_0000 | 0x0000_0008)
            .spawn();
    }
}
