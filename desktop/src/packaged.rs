//! couch as a packaged app (MSIX): installed with Windows' own installer,
//! uninstalled straight from Start. This is everything that differs when it
//! runs that way: how it updates, how an older unpackaged couch moves over to
//! it, and tidying up after that move.

/// The package's name (desktop/resources/msix/AppxManifest.xml).
pub const NAME: &str = "MidniteCouch";

/// Is this couch the installed package (not the loose folder)?
pub fn is_packaged() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::sync::OnceLock;
        static P: OnceLock<bool> = OnceLock::new();
        return *P.get_or_init(|| {
            let mut len = 0u32;
            let r = unsafe { windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName(&mut len, None) };
            // 15700 = no package; otherwise (ERROR_INSUFFICIENT_BUFFER) there is one: is it ours?
            if r.0 == 15700 || len == 0 {
                return false;
            }
            let mut buf = vec![0u16; len as usize];
            let r = unsafe { windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName(&mut len, Some(windows::core::PWSTR(buf.as_mut_ptr()))) };
            r.0 == 0 && String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]).starts_with(NAME)
        });
    }
    #[cfg(not(target_os = "windows"))]
    false
}

/// Where a version's signed package and certificate are (public).
pub fn asset_url(version: &str, file: &str) -> String {
    format!("https://github.com/xnite7/lumen/releases/download/gpui-{version}/{file}")
}

#[cfg(target_os = "windows")]
fn ps_hidden(script: &str, wait: bool) -> bool {
    use std::os::windows::process::CommandExt;
    let mut c = std::process::Command::new("powershell");
    c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-Command", script]).creation_flags(0x0800_0000);
    if wait {
        c.status().map_or(false, |s| s.success())
    } else {
        // detached: it outlives couch
        c.creation_flags(0x0800_0000 | 0x0000_0008).spawn().is_ok()
    }
}

/// Download a file to couch's temp folder.
pub async fn fetch(url: &str, name: &str) -> Option<std::path::PathBuf> {
    let core = couch::state::get().core.clone();
    let url = url.to_string();
    let path = std::env::temp_dir().join("midnite-couch").join(name);
    let p2 = path.clone();
    couch::rt::spawn(async move {
        let r = core.http.client().get(&url).timeout(std::time::Duration::from_secs(600)).send().await.ok()?;
        if !r.status().is_success() {
            return None;
        }
        let b = r.bytes().await.ok()?;
        std::fs::create_dir_all(p2.parent()?).ok()?;
        std::fs::write(&p2, &b).ok()?;
        Some(())
    })
    .await
    .ok()
    .flatten()?;
    Some(path)
}

/// Is couch's certificate trusted on this PC yet?
pub fn trusted() -> bool {
    #[cfg(target_os = "windows")]
    return ps_hidden("if (Get-ChildItem Cert:\\LocalMachine\\TrustedPeople | Where-Object { $_.Subject -eq 'CN=midnite couch' }) { exit 0 } else { exit 1 }", true);
    #[cfg(not(target_os = "windows"))]
    true
}

/// Trust couch's certificate (the one admin prompt). True when it worked.
pub fn trust(cer: &std::path::Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        // a tiny script, run as admin (the one prompt)
        let script = std::env::temp_dir().join("midnite-couch").join("trust.ps1");
        let body = format!("Import-Certificate -FilePath '{}' -CertStoreLocation Cert:\\LocalMachine\\TrustedPeople | Out-Null", cer.display().to_string().replace('\'', "''"));
        if std::fs::write(&script, body).is_err() {
            return false;
        }
        let sp = script.display().to_string().replace('\'', "''");
        ps_hidden(&format!("Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList ('-NoProfile -ExecutionPolicy Bypass -File \"' + '{sp}' + '\"')"), true);
        return trusted();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = cer;
        true
    }
}

/// Open the package in Windows' installer (the "Install midnite couch?" window).
pub fn open_installer(msix: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let p = msix.display().to_string().replace('\'', "''");
        ps_hidden(&format!("Start-Process '{p}'"), false);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = msix;
}

/// Update the installed package to this file, then start it again (couch
/// closes; Windows swaps the files while it's closed).
pub fn apply_update(msix: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let p = msix.display().to_string().replace('\'', "''");
        ps_hidden(
            &format!(
                "Start-Sleep -Milliseconds 800; Add-AppxPackage -Path '{p}' -ForceApplicationShutdown -ForceUpdateFromAnyVersion; Start-Process (Join-Path $env:LOCALAPPDATA 'Microsoft\\WindowsApps\\couch.exe') -ArgumentList '--after-update'"
            ),
            false,
        );
    }
    #[cfg(not(target_os = "windows"))]
    let _ = msix;
}

/// First start as the package: take away what the older, unpackaged couch
/// left (its folder once it's closed, shortcuts, Apps entry, links, and the
/// small live-tile package it used).
pub fn tidy_old() {
    #[cfg(target_os = "windows")]
    {
        let db = &couch::state::get().core.db;
        if db.get::<bool>("gpui_tidied_v1").unwrap_or(false) {
            return;
        }
        let _ = db.set("gpui_tidied_v1", &true);
        let old = couch::beta::install_dir().display().to_string().replace('\'', "''");
        let script = format!(
            r#"$ErrorActionPreference = 'SilentlyContinue'
$c = Get-Process couch | Where-Object {{ $_.Path -like '{old}*' }}
if ($c) {{ $c | Wait-Process -Timeout 20 }}
Get-AppxPackage -Name Midnite.Couch | Remove-AppxPackage
foreach ($p in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))) {{ Remove-Item (Join-Path $p 'midnite couch (new).lnk') }}
Remove-Item 'HKCU:\Software\Classes\couch' -Recurse
Remove-Item 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MidniteCouchNew' -Recurse
Remove-Item (Join-Path $env:LOCALAPPDATA 'midnite couch (new) uninstall.ps1')
Set-Location $env:TEMP
Remove-Item '{old}' -Recurse -Force"#
        );
        ps_hidden(&script, false);
    }
}
