//! Keeping the new couch up to date: the crew server hands out new builds;
//! couch downloads one in the background, swaps its files (the running
//! .exe is moved aside) and the next launch is the new version.
use crate::Nav;
use gpui::App;
use std::time::Duration;

pub const VERSION: &str = match option_env!("COUCH_VERSION") {
    Some(v) => v,
    None => "0.0.0",
};

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

pub fn start(cx: &mut App) {
    let dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    if let Some(d) = &dir {
        couch::beta::clean_old(d);
    }
    // a dev build (no version) doesn't update itself
    if VERSION == "0.0.0" {
        return;
    }
    cx.spawn(async move |cx| {
        let mut told = false;
        loop {
            gpui::Timer::after(Duration::from_secs(if told { 3600 } else { 20 })).await;
            let Some(dir) = dir.clone() else { break };
            let core = couch::state::get().core.clone();
            let latest = couch::rt::spawn({
                let core = core.clone();
                async move { couch::beta::latest(&core).await.ok() }
            })
            .await
            .ok()
            .flatten();
            let Some(v) = latest.and_then(|l| l["version"].as_str().map(|s| s.to_string())) else { continue };
            if !newer(&v, VERSION) || told {
                continue;
            }
            let ok = couch::rt::spawn(async move { couch::beta::download_into(&core, &dir).await }).await.map(|r| r.is_ok()).unwrap_or(false);
            if ok {
                told = true;
                let _ = cx.update(|cx| Nav::toast(format!("couch {v} is ready — it opens next time you start couch"), cx));
            }
        }
    })
    .detach();
}
