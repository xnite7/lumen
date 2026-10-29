//! Shared services every subsystem needs. Handed around as `Arc<Core>` so a
//! background task can hold it without borrowing app state.

use crate::db::Db;
use crate::http::Http;
use crate::settings::Settings;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub struct Core {
    pub app: AppHandle,
    pub db: Arc<Db>,
    pub http: Arc<Http>,
    pub settings: RwLock<Settings>,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl Core {
    pub fn settings(&self) -> Settings {
        self.settings.read().clone()
    }

    pub fn emit<S: serde::Serialize + Clone>(&self, event: &str, payload: S) {
        let _ = self.app.emit(event, payload);
    }

    /// Keys with a sensible public default fall back to it so the app works
    /// on first launch; anything the user pastes in wins.
    pub fn tmdb_key(&self) -> Option<String> {
        self.db.secret("tmdb").or_else(|| Some(DEFAULT_TMDB_KEY.to_string()))
    }
}

/// The key your Clip Previews plugin already shipped with.
pub const DEFAULT_TMDB_KEY: &str = "446c74f0d96977fb4145e8e79fba6bb2";

pub type CoreRef = Arc<Core>;
