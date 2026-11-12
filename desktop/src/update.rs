//! Keeping the new couch up to date: the crew server hands out new builds;
//! couch downloads one in the background and swaps its files (the running
//! .exe is moved aside), checking every 2 minutes. Then a card offers "restart now"; otherwise the
//! next launch is the new version.
use gpui::{div, prelude::*, px, App, FontWeight, Global};
use std::time::Duration;

pub const VERSION: &str = match option_env!("COUCH_VERSION") {
    Some(v) => v,
    None => "0.0.0",
};

/// a newer couch is downloaded and waiting (its version), unless dismissed
#[derive(Default)]
pub struct Ready(pub Option<String>);
impl Global for Ready {}

fn newer(a: &str, b: &str) -> bool {
    let p = |s: &str| s.split('.').map(|x| x.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    let (a, b) = (p(a), p(b));
    for i in 0..3 {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

fn dir() -> Option<std::path::PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

/// A Start menu entry (and the right icon on it), made once if it's missing.
fn start_menu(d: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let Some(programs) = dirs::data_dir().map(|a| a.join("Microsoft\\Windows\\Start Menu\\Programs")) else { return };
        let lnk = programs.join("midnite couch (new).lnk");
        if lnk.exists() {
            return;
        }
        let exe = d.join("couch.exe");
        let ps = format!(
            "$s=New-Object -ComObject WScript.Shell; $l=$s.CreateShortcut('{}'); $l.TargetPath='{}'; $l.WorkingDirectory='{}'; $l.IconLocation='{},0'; $l.Save()",
            lnk.display(),
            exe.display(),
            d.display(),
            exe.display()
        );
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("powershell").args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps]).creation_flags(0x0800_0000).spawn();
    }
    #[cfg(not(target_os = "windows"))]
    let _ = d;
}

pub fn start(cx: &mut App) {
    cx.set_global(Ready::default());
    if let Some(d) = dir() {
        couch::beta::clean_old(&d);
        if VERSION != "0.0.0" {
            start_menu(&d);
        }
    }
    // a dev build (no version) doesn't update itself
    if VERSION == "0.0.0" {
        return;
    }
    cx.spawn(async move |cx| {
        // what's already been fetched (a newer one can still come along)
        let mut have = VERSION.to_string();
        let mut first = true;
        loop {
            crate::timer::after(Duration::from_secs(if first { 10 } else { 120 })).await;
            first = false;
            let Some(dir) = dir() else { break };
            let core = couch::state::get().core.clone();
            let latest = couch::rt::spawn({
                let core = core.clone();
                async move { couch::beta::latest(&core).await.ok() }
            })
            .await
            .ok()
            .flatten();
            let Some(v) = latest.and_then(|l| l["version"].as_str().map(|s| s.to_string())) else { continue };
            if !newer(&v, &have) {
                continue;
            }
            let ok = couch::rt::spawn(async move { couch::beta::download_into(&core, &dir).await }).await.map(|r| r.is_ok()).unwrap_or(false);
            if ok {
                have = v.clone();
                let _ = cx.update(|cx| {
                    cx.set_global(Ready(Some(v)));
                    cx.refresh_windows();
                });
            }
        }
    })
    .detach();
}

/// Into the new version now: save what's playing, open the new couch, close this one.
pub fn restart(cx: &mut App) {
    let Some(d) = dir() else { return };
    let player = couch::state::get().player.clone();
    let _ = couch::rt::block_on(async move { player.stop().await });
    if std::process::Command::new(d.join("couch.exe")).current_dir(&d).spawn().is_ok() {
        cx.quit();
    }
}

/// The card in the corner when a new version is ready.
pub fn card(cx: &mut App) -> Option<impl IntoElement> {
    let v = cx.try_global::<Ready>().and_then(|r| r.0.clone())?;
    let btn = |id: &'static str, label: &'static str, strong: bool| {
        div()
            .id(id)
            .px(px(16.))
            .h(px(36.))
            .flex()
            .items_center()
            .rounded_full()
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(14.))
            .cursor_pointer()
            .when(strong, |d| d.bg(crate::theme::neon()))
            .when(!strong, |d| d.bg(crate::theme::white(0.1)))
            .hover(|s| s.opacity(0.85))
            .child(label)
    };
    Some(
        div()
            .absolute()
            .left(px(24.))
            .bottom(px(24.))
            .w(px(340.))
            .p(px(16.))
            .rounded(px(18.))
            .bg(crate::theme::surface())
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().flex().flex_col().gap(px(2.)).child(div().text_size(px(16.)).font_weight(FontWeight::BLACK).child(format!("couch {v} is ready"))).child(div().text_size(px(13.)).text_color(crate::theme::label2()).child("restart to get it — or it's there next time you open couch")))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(btn("upd-now", "restart now", true).on_click(|_, _, cx| restart(cx)))
                    .child(btn("upd-later", "later", false).on_click(|_, _, cx| {
                        cx.set_global(Ready(None));
                        cx.refresh_windows();
                    })),
            ),
    )
}
