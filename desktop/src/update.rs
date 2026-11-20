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

/// how the waiting version goes in: the loose files were swapped already, or
/// a signed package (an update to the installed couch, or the move to it)
#[derive(Clone)]
enum Via {
    Files,
    Package(std::path::PathBuf),
    MoveTo { msix: std::path::PathBuf, cer: std::path::PathBuf },
}
static VIA: parking_lot::Mutex<Option<Via>> = parking_lot::Mutex::new(None);

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
        // with the live tile package, its own Start entry stands in for this
        if lnk.exists() || crate::tile::live() || couch::state::try_get().and_then(|s| s.core.db.get::<bool>("tile_live")).unwrap_or(false) {
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
    // the update that brought us here (and any before it): not needed anymore
    // (right away: the first look for a newer one is 10 s off)
    std::thread::spawn(|| crate::packaged::sweep(None));
    if crate::packaged::is_packaged() {
        crate::packaged::tidy_old();
    } else if let Some(d) = dir() {
        couch::beta::clean_old(&d);
        if VERSION != "0.0.0" {
            start_menu(&d);
            crate::uninstall::register(VERSION);
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
            // the signed package for it, when there is one: installed couch
            // updates with it; a loose-folder couch moves over to it
            let msix = crate::packaged::fetch(&crate::packaged::asset_url(&v, "couch.msix"), &format!("couch-{v}.msix")).await;
            let via = match (msix, crate::packaged::is_packaged()) {
                (Some(m), true) => Some(Via::Package(m)),
                (Some(m), false) => crate::packaged::fetch(&crate::packaged::asset_url(&v, "couch.cer"), "couch.cer").await.map(|c| Via::MoveTo { msix: m, cer: c }),
                (None, true) => None,
                (None, false) => {
                    let ok = couch::rt::spawn(async move { couch::beta::download_into(&core, &dir).await }).await.map(|r| r.is_ok()).unwrap_or(false);
                    ok.then_some(Via::Files)
                }
            };
            let ok = via.is_some();
            *VIA.lock() = via;
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
    let player = couch::state::get().player.clone();
    let _ = couch::rt::block_on(async move { player.stop().await });
    match VIA.lock().clone() {
        Some(Via::Package(m)) => {
            crate::packaged::apply_update(&m);
            cx.quit();
        }
        Some(Via::MoveTo { msix, cer }) => {
            // trust couch once (the admin prompt), install it quietly, and only
            // then close: if anything says no, this couch stays open and says why
            crate::Nav::toast("installing… say yes if Windows asks".into(), cx);
            cx.spawn(async move |cx| {
                let r = couch::rt::spawn_blocking(move || {
                    if !crate::packaged::trusted() && !crate::packaged::trust(&cer) {
                        return Err("couch needs that yes to install itself. try again from the card".to_string());
                    }
                    crate::packaged::install(&msix)?;
                    crate::packaged::launch_installed();
                    Ok(())
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
                let _ = cx.update(|cx| match r {
                    Ok(()) => cx.quit(),
                    Err(e) => crate::Nav::toast(format!("couldn't install: {e}"), cx),
                });
            })
            .detach();
        }
        _ => {
            let Some(d) = dir() else { return };
            if couch::beta::launch_with(&d.join("couch.exe"), &d, &["--after-update"]).is_ok() {
                cx.quit();
            }
        }
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
            .w(px(360.))
            .p(px(18.))
            .rounded(px(20.))
            // lit, so it stands out from any page: the neon at the edge, a glow under it
            .bg(gpui::linear_gradient(160., gpui::linear_color_stop(gpui::rgba(0x2a1a2cf7), 0.), gpui::linear_color_stop(gpui::rgba(0x150d17f7), 1.)))
            .border_1()
            .border_color(crate::theme::neon())
            .shadow(vec![
                gpui::BoxShadow { color: crate::theme::black(0.6), offset: gpui::point(px(0.), px(16.)), blur_radius: px(40.), spread_radius: px(0.), inset: false },
                gpui::BoxShadow { color: gpui::Hsla::from(crate::theme::neon()).opacity(0.45), offset: gpui::point(px(0.), px(0.)), blur_radius: px(28.), spread_radius: px(0.), inset: false },
            ])
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().flex().items_center().gap(px(12.)).child(div().size(px(40.)).flex_none().rounded_full().bg(crate::theme::neon()).flex().items_center().justify_center().child(crate::ui::icon("download", 20.))).child(div().flex_1().min_w_0().flex().flex_col().gap(px(2.)).child(div().text_size(px(17.)).font_weight(FontWeight::BLACK).child(format!("couch {v} is ready"))).child(div().w_full().text_size(px(13.)).line_height(px(18.)).text_color(crate::theme::label2()).child(if matches!(*VIA.lock(), Some(Via::MoveTo { .. })) {
                "couch now installs like a proper app: Windows asks once to trust it, then its installer opens"
            } else {
                "restart to get it, or it's there next time you open couch"
            }))))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(btn("upd-now", if matches!(*VIA.lock(), Some(Via::MoveTo { .. })) { "install now" } else { "restart now" }, true).on_click(|_, _, cx| restart(cx)))
                    .child(btn("upd-later", "later", false).on_click(|_, _, cx| {
                        cx.set_global(Ready(None));
                        cx.refresh_windows();
                    })),
            ),
    )
}
