//! "Invincible S4 E3 is out": a Windows notification on the day a new
//! episode of something you watch comes out (once per episode). Clicking it
//! opens the show in couch, through a couch: link registered for this user.
use crate::api;
use gpui::App;
use serde_json::{json, Value};
use std::time::Duration;

const SEEN_KEY: &str = "gpui_notified_eps";

pub fn start(cx: &mut App) {
    #[cfg(target_os = "windows")]
    {
        std::thread::spawn(imp::register_links);
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

async fn check() {
    let items = api::call::<Vec<Value>>("calendar_get", json!({ "hits": false })).await.unwrap_or_default();
    let today = couch::integrations::tmdb::today_str();
    let db = &couch::state::get().core.db;
    let mut seen: Vec<String> = db.get::<Vec<String>>(SEEN_KEY).unwrap_or_default();
    let mut out = vec![];
    for i in &items {
        // yours (shows you watch), out today
        if i["source"].as_str() != Some("yours") || i["date"].as_str().map(|d| &d[..d.len().min(10)]) != Some(today.as_str()) {
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
        out.push((format!("{name} · {label} is out"), if ep.is_empty() { "new tonight on your couch".into() } else { ep }, format!("couch:detail:{kind}:{id}")));
    }
    if out.is_empty() {
        return;
    }
    let keep = seen.len().saturating_sub(300);
    let _ = db.set(SEEN_KEY, &seen[keep..].to_vec());
    #[cfg(target_os = "windows")]
    for (title, body, link) in out.into_iter().take(4) {
        let _ = couch::rt::spawn_blocking(move || imp::toast(&title, &body, &link)).await;
    }
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

    pub fn toast(title: &str, body: &str, link: &str) {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
        }
        let xml = format!(
            r#"<toast activationType="protocol" launch="{}"><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual><actions><action content="open" activationType="protocol" arguments="{}"/></actions></toast>"#,
            esc(link),
            esc(title),
            esc(body),
            esc(link)
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
