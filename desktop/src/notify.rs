//! "Invincible S4 E3 is out": a Windows notification on the day a new
//! episode of something you watch comes out (once per episode). Clicking it
//! opens the show in couch, through a couch: link registered for this user.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[cfg(target_os = "windows")]
use std::time::Duration;
use crate::api;
use gpui::App;
use serde_json::{json, Value};

const SEEN_KEY: &str = "gpui_notified_eps2";

pub fn start(cx: &mut App) {
    #[cfg(target_os = "windows")]
    {
        // (the installed couch declares its couch: links in its package)
        if !crate::packaged::is_packaged() {
            std::thread::spawn(imp::register_links);
        }
        cx.spawn(async move |_| {
            crate::timer::after(Duration::from_secs(45)).await;
            loop {
                check().await;
                crate::timer::after(Duration::from_secs(60 * 60)).await;
            }
        })
        .detach();
    }
    #[cfg(not(target_os = "windows"))]
    let _ = cx;
}

/// One alert: the words, where a click goes, a picture, and "watch now".
pub struct Alert {
    pub title: String,
    pub body: String,
    pub link: String,
    pub play: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
}

async fn check() {
    let core = couch::state::get().core.clone();
    if !core.settings().general.episode_alerts {
        return;
    }
    let items = api::call::<Vec<Value>>("calendar_get", json!({ "hits": false })).await.unwrap_or_default();
    // out today, or in the last two days (couch was closed, or it aired
    // overnight in the US)
    let today = couch::integrations::tmdb::today_str();
    let from = couch::integrations::tmdb::day_str(-2);
    let db = &core.db;
    let first = db.get::<Vec<String>>(SEEN_KEY).is_none();
    let mut seen: Vec<String> = db.get::<Vec<String>>(SEEN_KEY).unwrap_or_default();
    let mut out = vec![];
    for i in &items {
        let d = i["date"].as_str().map(|d| &d[..d.len().min(10)]).unwrap_or("");
        if i["source"].as_str() != Some("yours") || d < from.as_str() || d > today.as_str() {
            continue;
        }
        let id = i["imdb"].as_str().or(i["id"].as_str()).unwrap_or("").to_string();
        let label = i["label"].as_str().unwrap_or("").to_string();
        let key = format!("{id}|{label}");
        if id.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let name = i["name"].as_str().unwrap_or("").to_string();
        let ep = i["episode"].as_str().filter(|e| !e.starts_with("Episode ")).unwrap_or("").to_string();
        let kind = i["type"].as_str().unwrap_or("series").to_string();
        let (sn, en) = (i["season"].as_u64().unwrap_or(0), i["number"].as_u64().unwrap_or(0));
        let play = if kind == "series" && sn > 0 && en > 0 && id.starts_with("tt") {
            Some(format!("couch:play:series:{id}:{sn}:{en}"))
        } else if kind == "movie" && label == "at home" {
            Some(format!("couch:play:movie:{id}"))
        } else {
            None
        };
        let when = if d == today { "out today" } else { "just out" };
        out.push(Alert {
            title: format!("{name} · {label}"),
            body: if ep.is_empty() { format!("{when} on your couch") } else { format!("{ep} · {when}") },
            link: format!("couch:detail:{kind}:{id}"),
            play,
            hero: i["still"].as_str().or(i["background"].as_str()).or(i["backdrop"].as_str()).map(String::from),
            logo: i["poster"].as_str().map(String::from),
        });
    }
    let keep = seen.len().saturating_sub(300);
    let _ = db.set(SEEN_KEY, &seen[keep..].to_vec());
    // the first look: what's already out isn't news
    if first || out.is_empty() {
        return;
    }
    for mut a in out.into_iter().take(4) {
        a.hero = picture(&core, a.hero.as_deref()).await;
        a.logo = picture(&core, a.logo.as_deref()).await;
        #[cfg(target_os = "windows")]
        let _ = couch::rt::spawn_blocking(move || imp::toast(&a)).await;
        #[cfg(not(target_os = "windows"))]
        let _ = a;
    }
}

/// a picture for the notification: a file on disk (Windows reads it there)
async fn picture(core: &std::sync::Arc<couch::core::Core>, url: Option<&str>) -> Option<String> {
    let url = url?;
    let (bytes, _) = couch::images::load(core, url).await.ok()?;
    let dir = core.cache_dir.join("alerts");
    std::fs::create_dir_all(&dir).ok()?;
    let name = format!("{:x}.jpg", {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut h);
        h.finish()
    });
    let path = dir.join(name);
    if !path.exists() {
        std::fs::write(&path, bytes).ok()?;
    }
    Some(path.to_string_lossy().to_string())
}

#[cfg(target_os = "windows")]
mod imp {
    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

    fn esc(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    /// couch: links open couch (HKEY_CURRENT_USER, no admin)
    pub fn register_links() {
        let Ok(exe) = std::env::current_exe() else { return };
        let cmd = format!("\"{}\" --open \"%1\"", exe.display());
        use std::os::windows::process::CommandExt;
        let base = r"HKCU\Software\Classes\couch";
        for args in [
            vec!["add", base, "/ve", "/d", "URL:midnite couch", "/f"],
            vec!["add", base, "/v", "URL Protocol", "/d", "", "/f"],
            vec!["add", &format!(r"{base}\shell\open\command"), "/ve", "/d", &cmd, "/f"],
        ] {
            let _ = std::process::Command::new("reg").args(&args).creation_flags(0x0800_0000).status();
        }
    }

    pub fn toast(a: &super::Alert) {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
        }
        // the episode's picture across the top, the show's poster beside the
        // words, and "watch now" straight into the episode
        let hero = a.hero.as_ref().map(|p| format!(r#"<image placement="hero" src="file:///{}"/>"#, esc(&p.replace('\\', "/")))).unwrap_or_default();
        let logo = a.logo.as_ref().map(|p| format!(r#"<image placement="appLogoOverride" hint-crop="none" src="file:///{}"/>"#, esc(&p.replace('\\', "/")))).unwrap_or_default();
        let play = a.play.as_ref().map(|p| format!(r#"<action content="▶ watch now" activationType="protocol" arguments="{}"/>"#, esc(p))).unwrap_or_default();
        let xml = format!(
            r#"<toast activationType="protocol" launch="{link}"><visual><binding template="ToastGeneric">{hero}{logo}<text>{}</text><text>{}</text></binding></visual><actions>{play}<action content="details" activationType="protocol" arguments="{link}"/><action content="later" activationType="system" arguments="dismiss"/></actions></toast>"#,
            esc(&a.title),
            esc(&a.body),
            link = esc(&a.link),
        );
        let go = || -> windows::core::Result<()> {
            let doc = XmlDocument::new()?;
            doc.LoadXml(&HSTRING::from(xml.as_str()))?;
            let n = ToastNotification::CreateToastNotification(&doc)?;
            // with the live tile's package couch has its own identity; without it,
            // the name couch's window and shortcut go by
            let notifier = if crate::tile::live() {
                ToastNotificationManager::CreateToastNotifier()?
            } else {
                ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from("com.midnite.couch"))?
            };
            notifier.Show(&n)
        };
        let _ = go();
    }
}
