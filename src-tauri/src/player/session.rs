//! One playback session: progress saving, Simkl scrobbling, and telling the
//! lights what's happening. Runs as a 1 Hz task beside the player, so none
//! of it lives in the UI and none of it stops if the UI is busy animating.

use super::{PlayRequest, Player};
use crate::core::Core;
use crate::integrations::simkl;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct Session {
    pub id: u64,
    pub req: PlayRequest,
    #[allow(dead_code)]
    pub url: String,
    pub pos: f64,
    pub duration: f64,
    pub paused: bool,
    /// paused by the user (buffering is not a pause as far as Simkl knows)
    pub user_paused: bool,
    pub ended: bool,
    pub scrobbled: bool,
    /// a raw link/magnet identified by file name, for scrobbling
    pub ident: Option<serde_json::Value>,
}

impl Session {
    pub fn new(req: PlayRequest, url: String) -> Self {
        Self { id: NEXT_ID.fetch_add(1, Ordering::Relaxed), req, url, pos: 0.0, duration: 0.0, paused: true, user_paused: false, ended: false, scrobbled: false, ident: None }
    }

    pub fn update(&mut self, pos: f64, duration: f64, paused: bool) {
        self.pos = pos;
        if duration > 0.0 {
            self.duration = duration;
        }
        self.paused = paused;
        self.user_paused = paused;
    }

    fn progress(&self) -> f64 {
        if self.duration > 0.0 {
            self.pos / self.duration
        } else {
            0.0
        }
    }

    fn save(&self, core: &Core) {
        if self.duration <= 0.0 || self.pos <= 0.0 {
            return;
        }
        let th = core.settings.read().playback.watched_threshold;
        let _ = crate::library::progress(core, &self.req.meta, &self.req.video_id, (self.pos * 1000.0) as i64, (self.duration * 1000.0) as i64, th);
    }

    /// Closing the player (or moving to the next episode) is a `stop`:
    /// Simkl marks it watched at/above `min_progress` (the user's threshold,
    /// sent with the call) and saves a resume point below it. Played to the
    /// end counts as 100%.
    pub async fn finish(&self, core: &Core) {
        self.save(core);
        if self.scrobbled {
            let p = if self.ended || self.progress() > 0.985 { 1.0 } else { self.progress() };
            let _ = simkl::scrobble(core, "stop", &self.req.meta, &self.req.video_id, p, self.ident.as_ref()).await;
            let c = core.app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(3)).await;
                let core = crate::state::get(&c).core.clone();
                if crate::integrations::simkl_sync::sync(&core).await.unwrap_or(false) {
                    use tauri::Emitter;
                    let _ = c.emit("library-changed", ());
                }
            });
        }
        crate::state::hue(&core.app).on_playback(false).await;
    }
}

pub fn spawn_tracker(player: Arc<Player>) {
    let Some(sid) = player.session().map(|s| s.id) else { return };
    tauri::async_runtime::spawn(async move {
        let mut last_save = Instant::now();
        let mut was_paused: Option<bool> = None;
        let mut was_hue: Option<bool> = None;
        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            // Pull fresh numbers from mpv (desktop) or use what the web
            // player reported (iOS).
            #[cfg(desktop)]
            {
                let st = player.backend.state();
                if let Some(s) = player.session.lock().as_mut().filter(|s| s.id == sid) {
                    let user_paused = st["paused"].as_bool().unwrap_or(true);
                    s.update(
                        st["pos"].as_f64().unwrap_or(s.pos),
                        st["duration"].as_f64().unwrap_or(s.duration),
                        user_paused || st["buffering"].as_bool().unwrap_or(false),
                    );
                    s.user_paused = user_paused;
                    s.ended = st["ended"].as_bool().unwrap_or(false);
                }
            }
            let Some(s) = player.session().filter(|s| s.id == sid) else { break };
            let core = &player.core;

            // Simkl hears about real user actions only -- play and pause,
            // never buffering stalls, seeks or a timer (its guide: one call
            // per user action; there's a 20 s lock per user).
            if was_paused != Some(s.user_paused) && s.duration > 0.0 {
                let action = if s.user_paused { "pause" } else { "start" };
                if action == "start" || s.scrobbled {
                    let mut ident = s.ident.clone();
                    let raw = s.req.video_id.starts_with("magnet:") || s.req.video_id.starts_with("url:");
                    if raw && ident.is_none() && !s.scrobbled {
                        let file = s.req.stream.pointer("/behaviorHints/filename").and_then(|f| f.as_str()).map(|f| f.to_string()).unwrap_or_else(|| s.req.meta.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string());
                        ident = simkl::identify_file(core, &file).await;
                    }
                    if !raw || ident.is_some() {
                        let _ = simkl::scrobble(core, action, &s.req.meta, &s.req.video_id, s.progress(), ident.as_ref()).await;
                        if let Some(x) = player.session.lock().as_mut().filter(|x| x.id == sid) {
                            x.scrobbled = true;
                            x.ident = ident;
                        }
                    }
                }
                was_paused = Some(s.user_paused);
            }
            if was_hue != Some(s.paused) {
                crate::state::hue(&core.app).on_playback(!s.paused).await;
                was_hue = Some(s.paused);
            }

            if last_save.elapsed() >= Duration::from_secs(5) && !s.paused {
                s.save(core);
                last_save = Instant::now();
            }
        }
    });
}
