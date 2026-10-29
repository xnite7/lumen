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
    pub scrobbled: bool,
}

impl Session {
    pub fn new(req: PlayRequest, url: String) -> Self {
        Self { id: NEXT_ID.fetch_add(1, Ordering::Relaxed), req, url, pos: 0.0, duration: 0.0, paused: true, scrobbled: false }
    }

    pub fn update(&mut self, pos: f64, duration: f64, paused: bool) {
        self.pos = pos;
        if duration > 0.0 {
            self.duration = duration;
        }
        self.paused = paused;
    }

    fn kind(&self) -> String {
        self.req.meta.get("type").and_then(|t| t.as_str()).unwrap_or("movie").to_string()
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

    pub async fn finish(&self, core: &Core) {
        self.save(core);
        if self.scrobbled {
            let th = core.settings.read().playback.watched_threshold as f64 / 100.0;
            // At/above the threshold Simkl marks it watched; below it,
            // `pause` shelves it instead of claiming you finished.
            let action = if self.progress() >= th { "stop" } else { "pause" };
            let _ = simkl::scrobble(core, action, &self.kind(), &self.req.video_id, self.progress()).await;
        }
        crate::state::hue(&core.app).on_playback(false).await;
    }
}

pub fn spawn_tracker(player: Arc<Player>) {
    let Some(sid) = player.session().map(|s| s.id) else { return };
    tauri::async_runtime::spawn(async move {
        let mut last_save = Instant::now();
        let mut was_paused: Option<bool> = None;
        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            // Pull fresh numbers from mpv (desktop) or use what the web
            // player reported (iOS).
            #[cfg(desktop)]
            {
                let st = player.backend.state();
                if let Some(s) = player.session.lock().as_mut().filter(|s| s.id == sid) {
                    s.update(
                        st["pos"].as_f64().unwrap_or(s.pos),
                        st["duration"].as_f64().unwrap_or(s.duration),
                        st["paused"].as_bool().unwrap_or(true) || st["buffering"].as_bool().unwrap_or(false),
                    );
                }
            }
            let Some(s) = player.session().filter(|s| s.id == sid) else { break };
            let core = &player.core;

            if was_paused != Some(s.paused) && s.duration > 0.0 {
                let action = if s.paused { "pause" } else { "start" };
                if action == "start" || s.scrobbled {
                    let _ = simkl::scrobble(core, action, &s.kind(), &s.req.video_id, s.progress()).await;
                    if action == "start" {
                        if let Some(x) = player.session.lock().as_mut().filter(|x| x.id == sid) {
                            x.scrobbled = true;
                        }
                    }
                }
                crate::state::hue(&core.app).on_playback(!s.paused).await;
                was_paused = Some(s.paused);
            }

            if last_save.elapsed() >= Duration::from_secs(5) && !s.paused {
                s.save(core);
                last_save = Instant::now();
            }
        }
    });
}
