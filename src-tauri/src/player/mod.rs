//! Playback.
//!
//! On desktop, video is decoded and presented by libmpv straight into the
//! main window, *underneath* a transparent webview. The UI (controls,
//! subtitles, skip buttons, up-next) is ordinary HTML floating on top of the
//! real video surface -- no <video> element, no re-encoding, every codec and
//! container mpv supports, HDR passthrough, and the GPU scaler.
//!
//! On iOS there is no libmpv, so `open` hands the resolved URL back and the
//! UI plays it in a native-backed <video> instead.

#[cfg(desktop)]
mod mpv;
mod session;

use crate::core::CoreRef;
use crate::error::{AppError, AppResult};
use crate::torrent::Torrents;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[allow(unused_imports)]
use serde_json::json;
use std::sync::Arc;
pub use session::Session;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayRequest {
    pub stream: Value,
    /// Meta preview of the title (id, type, name, poster, background, logo).
    pub meta: Value,
    /// `tt123` for films, `tt123:1:4` for episodes.
    pub video_id: String,
    #[serde(default)]
    pub start_at: Option<f64>,
    #[serde(default)]
    pub episode_title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenResult {
    /// "native" (mpv is playing) | "web" (UI should play `url`) |
    /// "youtube" | "external"
    pub mode: &'static str,
    pub url: Option<String>,
    pub yt_id: Option<String>,
}

pub struct Player {
    core: CoreRef,
    pub torrents: Arc<Torrents>,
    #[cfg(desktop)]
    backend: mpv::Mpv,
    session: parking_lot::Mutex<Option<Session>>,
}

impl Player {
    pub fn new(core: CoreRef, torrents: Arc<Torrents>) -> Arc<Self> {
        Arc::new(Self {
            #[cfg(desktop)]
            backend: mpv::Mpv::new(core.clone()),
            core,
            torrents,
            session: parking_lot::Mutex::new(None),
        })
    }

    pub fn native(&self) -> bool {
        cfg!(desktop)
    }

    pub fn session(&self) -> Option<Session> {
        self.session.lock().clone()
    }

    async fn resolve_url(&self, stream: &Value) -> AppResult<Option<String>> {
        if let Some(u) = stream.get("url").and_then(|u| u.as_str()) {
            return Ok(Some(u.to_string()));
        }
        if let Some(h) = stream.get("infoHash").and_then(|h| h.as_str()) {
            let idx = stream.get("fileIdx").and_then(|i| i.as_u64()).map(|i| i as usize);
            let sources: Vec<String> = stream
                .get("sources")
                .and_then(|s| s.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            return Ok(Some(self.torrents.stream(h, idx, &sources).await?));
        }
        Ok(None)
    }

    pub async fn open(self: &Arc<Self>, req: PlayRequest) -> AppResult<OpenResult> {
        if let Some(yt) = req.stream.get("ytId").and_then(|y| y.as_str()) {
            return Ok(OpenResult { mode: "youtube", url: None, yt_id: Some(yt.to_string()) });
        }
        if req.stream.get("url").is_none() && req.stream.get("infoHash").is_none() {
            if let Some(ext) = req.stream.get("externalUrl").and_then(|u| u.as_str()) {
                return Ok(OpenResult { mode: "external", url: Some(ext.to_string()), yt_id: None });
            }
        }
        let url = self.resolve_url(&req.stream).await?.ok_or_else(|| AppError::msg("This stream has nothing playable"))?;

        // A new title ends the previous session properly (progress + scrobble).
        self.finish_session().await;
        let session = Session::new(req.clone(), url.clone());
        *self.session.lock() = Some(session.clone());

        let start = req.start_at.or_else(|| self.resume_point(&req));

        #[cfg(desktop)]
        {
            let headers = req
                .stream
                .pointer("/behaviorHints/proxyHeaders/request")
                .and_then(|h| h.as_object())
                .map(|h| h.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string()))).collect::<Vec<_>>())
                .unwrap_or_default();
            self.backend.load(&url, start, &headers)?;
            // Subtitles from addons arrive a beat later and are added as
            // external tracks; mpv picks them up without a reload.
            let this = self.clone();
            let kind = req.meta.get("type").and_then(|t| t.as_str()).unwrap_or("movie").to_string();
            let vid = req.video_id.clone();
            let filename = req.stream.pointer("/behaviorHints/filename").and_then(|f| f.as_str()).map(|s| s.to_string());
            let size = req.stream.pointer("/behaviorHints/videoSize").and_then(|s| s.as_u64());
            tauri::async_runtime::spawn(async move {
                let mut extra = vec![];
                if let Some(f) = filename {
                    extra.push(("filename".to_string(), f));
                }
                if let Some(s) = size {
                    extra.push(("videoSize".to_string(), s.to_string()));
                }
                let addons = crate::state::addons(&this.core.app);
                let subs = addons.subtitles(&kind, &vid, &extra).await;
                this.backend.add_subtitles(&subs);
                this.core.emit("player://subtitles", subs);
            });
            session::spawn_tracker(self.clone());
            Ok(OpenResult { mode: "native", url: Some(url), yt_id: None })
        }
        #[cfg(not(desktop))]
        {
            let _ = start;
            session::spawn_tracker(self.clone());
            Ok(OpenResult { mode: "web", url: Some(url), yt_id: None })
        }
    }

    fn resume_point(&self, req: &PlayRequest) -> Option<f64> {
        let id = req.meta.get("id").and_then(|i| i.as_str())?;
        let item = crate::library::get(&self.core, id)?;
        let st = &item["state"];
        if st["video_id"].as_str() != Some(req.video_id.as_str()) {
            return None;
        }
        let t = st["timeOffset"].as_f64()? / 1000.0;
        let d = st["duration"].as_f64().unwrap_or(0.0) / 1000.0;
        // Don't resume into the credits or from the first few seconds.
        if t < 15.0 || (d > 0.0 && t > d * 0.95) {
            None
        } else {
            Some(t)
        }
    }

    /// Persist the last position and tell Simkl we stopped.
    pub async fn finish_session(&self) {
        let s = self.session.lock().take();
        if let Some(s) = s {
            s.finish(&self.core).await;
        }
        self.torrents.stop_current().await;
    }

    pub async fn stop(&self) -> AppResult<()> {
        #[cfg(desktop)]
        self.backend.stop();
        self.finish_session().await;
        Ok(())
    }

    /// Generic control surface: the UI sends small typed commands.
    pub fn command(&self, cmd: &str, args: &Value) -> AppResult<Value> {
        #[cfg(desktop)]
        {
            return self.backend.command(cmd, args);
        }
        #[cfg(not(desktop))]
        {
            let _ = (cmd, args);
            Ok(Value::Null)
        }
    }

    /// Web-mode players report their own position so progress/scrobble work
    /// the same on iOS.
    pub fn report(&self, pos: f64, duration: f64, paused: bool) {
        if let Some(s) = self.session.lock().as_mut() {
            s.update(pos, duration, paused);
        }
    }

    pub fn state(&self) -> Value {
        #[cfg(desktop)]
        {
            return self.backend.state();
        }
        #[cfg(not(desktop))]
        json!(null)
    }

    pub fn tracks(&self) -> Value {
        #[cfg(desktop)]
        {
            return self.backend.tracks();
        }
        #[cfg(not(desktop))]
        json!([])
    }
}
