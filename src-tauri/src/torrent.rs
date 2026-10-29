//! Built-in torrent streaming (librqbit), for addons that return
//! `infoHash` instead of a debrid URL.
//!
//! Only the chosen file is downloaded, pieces are fetched in playback order
//! because mpv reads through the local HTTP stream, and the cache is wiped
//! when you stop -- nothing accumulates in a downloads folder.

use crate::error::{AppError, AppResult};
use librqbit::api::Api;
use librqbit::http_api::{HttpApi, HttpApiOptions};
use librqbit::{AddTorrent, AddTorrentOptions, ManagedTorrent, Session, SessionOptions};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use librqbit_dualstack_sockets::{BindOpts, TcpListener};
use tokio::sync::OnceCell;

const VIDEO_EXT: &[&str] = &["mkv", "mp4", "avi", "m4v", "mov", "webm", "ts", "m2ts", "wmv", "flv"];

pub struct Engine {
    session: Arc<Session>,
    port: u16,
    current: Mutex<Option<Arc<ManagedTorrent>>>,
}

pub struct Torrents {
    engine: OnceCell<Engine>,
    dir: PathBuf,
}

impl Torrents {
    pub fn new(dir: PathBuf) -> Self {
        Self { engine: OnceCell::new(), dir }
    }

    /// Lazily started: most sessions are debrid-only and never pay for a
    /// DHT node.
    async fn engine(&self) -> AppResult<&Engine> {
        self.engine
            .get_or_try_init(|| async {
                let _ = tokio::fs::remove_dir_all(&self.dir).await;
                tokio::fs::create_dir_all(&self.dir).await?;
                let opts = SessionOptions { fastresume: false, persistence: None, ..Default::default() };
                let session = Session::new_with_opts(self.dir.clone(), opts).await.map_err(|e| AppError::msg(format!("torrent engine: {e:#}")))?;
                let listener = TcpListener::bind_tcp(
                    "127.0.0.1:0".parse().unwrap(),
                    BindOpts { request_dualstack: false, ..Default::default() },
                )
                .map_err(|e| AppError::msg(format!("torrent http: {e}")))?;
                let port = listener.socket().local_addr()?.port();
                let api = Api::new(session.clone(), None, None);
                let http = HttpApi::new(api, Some(HttpApiOptions { read_only: true, ..Default::default() }));
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = http.make_http_api_and_run(listener, None).await {
                        log::error!("torrent http: {e:#}");
                    }
                });
                Ok::<_, AppError>(Engine { session, port, current: Mutex::new(None) })
            })
            .await
    }

    /// Start streaming and return a local URL mpv can open.
    pub async fn stream(&self, info_hash: &str, file_idx: Option<usize>, sources: &[String]) -> AppResult<String> {
        let e = self.engine().await?;
        self.stop_current().await;

        let trackers: Vec<String> = sources.iter().filter_map(|s| s.strip_prefix("tracker:").map(|t| t.to_string())).collect();
        let mut magnet = format!("magnet:?xt=urn:btih:{info_hash}");
        for t in &trackers {
            magnet.push_str(&format!("&tr={}", urlencoding::encode(t)));
        }
        let opts = AddTorrentOptions {
            paused: true,
            overwrite: true,
            trackers: if trackers.is_empty() { None } else { Some(trackers.clone()) },
            ..Default::default()
        };
        let resp = e
            .session
            .add_torrent(AddTorrent::from_url(&magnet), Some(opts))
            .await
            .map_err(|err| AppError::msg(format!("Couldn't start torrent: {err:#}")))?;
        let handle = resp.into_handle().ok_or_else(|| AppError::msg("Torrent has no metadata"))?;

        // Pick the file: the addon's fileIdx if it gave one, otherwise the
        // largest video file (skipping samples and extras).
        let idx = match file_idx {
            Some(i) => i,
            None => handle
                .with_metadata(|m| {
                    m.file_infos
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| {
                            let name = f.relative_filename.to_string_lossy().to_lowercase();
                            VIDEO_EXT.iter().any(|x| name.ends_with(&format!(".{x}"))) && !name.contains("sample")
                        })
                        .max_by_key(|(_, f)| f.len)
                        .map(|(i, _)| i)
                        .unwrap_or(0)
                })
                .unwrap_or(0),
        };
        let only: HashSet<usize> = [idx].into_iter().collect();
        e.session.update_only_files(&handle, &only).await.map_err(|err| AppError::msg(format!("{err:#}")))?;
        e.session.unpause(&handle).await.map_err(|err| AppError::msg(format!("{err:#}")))?;
        let id = handle.id();
        *e.current.lock() = Some(handle);
        Ok(format!("http://127.0.0.1:{}/torrents/{id}/stream/{idx}", e.port))
    }

    pub async fn stop_current(&self) {
        let Some(e) = self.engine.get() else { return };
        let cur = e.current.lock().take();
        if let Some(h) = cur {
            let _ = e.session.delete(h.id().into(), true).await;
        }
    }

    pub fn stats(&self) -> Value {
        let Some(e) = self.engine.get() else { return Value::Null };
        let cur = e.current.lock().clone();
        match cur {
            Some(h) => {
                let s = h.stats();
                let live = s.live.as_ref();
                json!({
                    "progressBytes": s.progress_bytes,
                    "totalBytes": s.total_bytes,
                    "downloadMbps": live.map(|l| l.download_speed.mbps),
                    "uploadMbps": live.map(|l| l.upload_speed.mbps),
                    "peers": live.map(|l| l.snapshot.peer_stats.live),
                    "finished": s.finished,
                    "error": s.error,
                })
            }
            None => Value::Null,
        }
    }

}
