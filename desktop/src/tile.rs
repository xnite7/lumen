//! The Windows 10 live tile. couch.exe gets an app identity from a small
//! signed "sparse" package (couch.msix, shipped beside it): registered once
//! (trusting its certificate is the one admin prompt), after which couch can
//! feed its Start tile. The tile flips through the crew's latest (with their
//! face peeking in), what you're in the middle of and something from your
//! plan to watch, and keeps doing so after couch is closed.
use crate::api;
use gpui::App;
use serde_json::{json, Value};
use std::time::Duration;

/// Get going: register the package if it's shipped and not done yet, then
/// keep the tile fresh (every 20 minutes while couch runs).
pub fn start(cx: &mut App) {
    #[cfg(target_os = "windows")]
    cx.spawn(async move |cx| {
        crate::timer::after(Duration::from_secs(8)).await;
        if !imp::has_identity() {
            let fresh = couch::rt::spawn_blocking(imp::register).await.unwrap_or(false);
            if fresh {
                let _ = cx.update(|cx| crate::Nav::toast("live tile ready — restart couch, then pin “midnite couch” to Start".to_string(), cx));
            }
            return;
        }
        loop {
            let tiles = content().await;
            if !tiles.is_empty() {
                let _ = couch::rt::spawn_blocking(move || imp::push(&tiles)).await;
            }
            crate::timer::after(Duration::from_secs(20 * 60)).await;
        }
    })
    .detach();
    #[cfg(not(target_os = "windows"))]
    let _ = cx;
}

/// Does couch run with its package identity (the tile is live)?
pub fn live() -> bool {
    #[cfg(target_os = "windows")]
    return imp::has_identity();
    #[cfg(not(target_os = "windows"))]
    false
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// a picture the tile can fetch (http pictures must stay small)
fn pic(u: Option<&str>, size: &str) -> Option<String> {
    let u = u?;
    if !u.starts_with("https://") {
        return None;
    }
    let s = api::sized(u, size);
    // metahub's big backdrops are too heavy for a tile
    if s.contains("metahub.space/background") {
        return None;
    }
    Some(s)
}

struct Tile {
    art: Option<String>,
    peek: Option<String>,
    top: String,
    title: String,
    sub: String,
}

impl Tile {
    fn xml(&self) -> String {
        let art = self.art.as_ref().map(|a| format!(r#"<image src="{}" placement="background" hint-overlay="45"/>"#, esc(a))).unwrap_or_default();
        let peek = self.peek.as_ref().map(|p| format!(r#"<image src="{}" placement="peek" hint-crop="circle"/>"#, esc(p))).unwrap_or_default();
        let (top, title, sub) = (esc(&self.top), esc(&self.title), esc(&self.sub));
        format!(
            r#"<tile><visual branding="name" displayName="couch">
<binding template="TileMedium" hint-textStacking="bottom">{art}{peek}<text hint-style="captionSubtle" hint-wrap="true" hint-maxLines="1">{top}</text><text hint-style="caption" hint-wrap="true" hint-maxLines="2">{title}</text></binding>
<binding template="TileWide" hint-textStacking="bottom">{art}{peek}<text hint-style="captionSubtle">{top}</text><text hint-style="body">{title}</text><text hint-style="captionSubtle">{sub}</text></binding>
<binding template="TileLarge" hint-textStacking="bottom">{art}{peek}<text hint-style="captionSubtle">{top}</text><text hint-style="subtitle" hint-wrap="true" hint-maxLines="2">{title}</text><text hint-style="bodySubtle" hint-wrap="true" hint-maxLines="3">{sub}</text></binding>
</visual></tile>"#
        )
    }
}

/// up to five tiles: three from the crew, one you're in the middle of, one
/// from your plan to watch
async fn content() -> Vec<String> {
    let mut out: Vec<Tile> = vec![];
    let feed = api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/activity", "body": Value::Null })).await.unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    for a in feed["feed"].as_array().into_iter().flatten() {
        let t = &a["title"];
        if !seen.insert(t["id"].as_str().unwrap_or("").to_string()) {
            continue;
        }
        let who = a["name"].as_str().unwrap_or("someone");
        let (verb, sub) = match (a["kind"].as_str(), a["rating"].as_i64(), a["text"].as_str().filter(|t| !t.is_empty())) {
            (_, r, Some(text)) => ("reviewed", format!("{}“{text}”", r.map(|r| format!("★ {r}/10 · ")).unwrap_or_default())),
            (_, Some(r), None) => ("rated", format!("★ {r}/10")),
            (Some("finished"), _, _) => ("finished", "all the way through".to_string()),
            _ => ("watched", String::new()),
        };
        out.push(Tile {
            art: pic(t["background"].as_str(), "w300").or_else(|| pic(t["poster"].as_str(), "w342")),
            peek: a["avatar"].as_str().map(String::from),
            top: format!("{who} {verb}"),
            title: t["name"].as_str().unwrap_or("").to_string(),
            sub,
        });
        if out.len() == 3 {
            break;
        }
    }
    if let Ok(cw) = api::call::<Vec<Value>>("continue_watching", json!({})).await {
        if let Some(v) = cw.first() {
            let vid = v["state"]["video_id"].as_str().unwrap_or("");
            let p: Vec<&str> = vid.split(':').collect();
            out.push(Tile {
                art: pic(v["background"].as_str(), "w300").or_else(|| pic(v["poster"].as_str(), "w342")),
                peek: None,
                top: "pick up where you left off".into(),
                title: v["name"].as_str().unwrap_or("").to_string(),
                sub: if p.len() == 3 { format!("S{} E{}", p[1], p[2]) } else { String::new() },
            });
        }
    }
    // something from the plan to watch (Simkl), else what couch would feature
    let cats = api::call::<Vec<Value>>("catalogs_list", json!({})).await.unwrap_or_default();
    if let Some(c) = cats.iter().find(|c| c["id"] == "simkl-plantowatch").cloned() {
        if let Ok(items) = api::call::<Vec<Value>>("catalog_fetch", json!({ "catalog": c, "extra": [] })).await {
            let n = items.len();
            if n > 0 {
                let i = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) / 1200) as usize % n;
                let v = &items[i];
                out.push(Tile {
                    art: pic(v["background"].as_str(), "w300").or_else(|| pic(v["poster"].as_str(), "w342")),
                    peek: None,
                    top: "on your plan to watch".into(),
                    title: v["name"].as_str().unwrap_or("").to_string(),
                    sub: v["releaseInfo"].as_str().unwrap_or("").to_string(),
                });
            }
        }
    }
    out.into_iter().take(5).map(|t| t.xml()).collect()
}

#[cfg(target_os = "windows")]
mod imp {
    use std::os::windows::process::CommandExt;
    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{TileNotification, TileUpdateManager};

    const NO_WINDOW: u32 = 0x0800_0000;

    pub fn has_identity() -> bool {
        let mut len = 0u32;
        let r = unsafe { windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName(&mut len, None) };
        // 15700 = APPMODEL_ERROR_NO_PACKAGE
        r.0 != 15700
    }

    fn ps(cmd: &str) -> bool {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", cmd])
            .creation_flags(NO_WINDOW)
            .status()
            .map_or(false, |s| s.success())
    }

    /// Register couch.msix (beside the exe) with the exe's folder as its
    /// content. Once per shipped package; true when it just got registered.
    pub fn register() -> bool {
        let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) else { return false };
        let msix = dir.join("couch.msix");
        let cer = dir.join("couch.cer");
        let Ok(meta) = std::fs::metadata(&msix) else { return false };
        let stamp = format!("{}:{}", meta.len(), meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()));
        let db = &couch::state::get().core.db;
        if db.get::<String>("tile_pkg").as_deref() == Some(stamp.as_str()) {
            return false;
        }
        let q = |p: &std::path::Path| p.display().to_string().replace('\'', "''");
        let add = format!("Add-AppxPackage -Path '{}' -ExternalLocation '{}' -ForceUpdateFromAnyVersion -ErrorAction Stop", q(&msix), q(&dir));
        let mut ok = ps(&add);
        if !ok && cer.exists() {
            // Windows doesn't trust couch's certificate yet: one admin prompt to
            // add it to Trusted People, then try again
            let trust = format!(
                "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '-NoProfile','-Command','Import-Certificate -FilePath ''{}'' -CertStoreLocation Cert:\\LocalMachine\\TrustedPeople'",
                q(&cer).replace('\'', "''")
            );
            ps(&trust);
            ok = ps(&add);
        }
        let _ = db.set("tile_pkg", &stamp);
        if ok {
            // the package has its own Start entry now: the old shortcut would be a twin
            if let Some(programs) = dirs::data_dir().map(|a| a.join("Microsoft\\Windows\\Start Menu\\Programs")) {
                let _ = std::fs::remove_file(programs.join("midnite couch (new).lnk"));
            }
            let _ = db.set("tile_live", &true);
        }
        ok
    }

    pub fn push(tiles: &[String]) {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
        }
        let go = || -> windows::core::Result<()> {
            let up = TileUpdateManager::CreateTileUpdaterForApplication()?;
            up.EnableNotificationQueue(true)?;
            up.Clear()?;
            for (i, x) in tiles.iter().enumerate() {
                let doc = XmlDocument::new()?;
                doc.LoadXml(&HSTRING::from(x.as_str()))?;
                let n = TileNotification::CreateTileNotification(&doc)?;
                n.SetTag(&HSTRING::from(format!("c{i}")))?;
                up.Update(&n)?;
            }
            Ok(())
        };
        if let Err(e) = go() {
            eprintln!("tile: {e}");
        }
    }
}
