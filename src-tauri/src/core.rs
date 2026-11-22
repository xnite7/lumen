//! Shared services every subsystem needs. Handed around as `Arc<Core>` so a
//! background task can hold it without borrowing app state.

use crate::db::Db;
use crate::http::Http;
use crate::settings::Settings;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use crate::host::Host;

pub struct Core {
    pub host: Arc<dyn Host>,
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
        self.host.emit(event, serde_json::to_value(payload).unwrap_or(serde_json::Value::Null));
    }

    /// Keys with a sensible public default fall back to it so the app works
    /// on first launch; anything the user pastes in wins.
    pub fn tmdb_key(&self) -> Option<String> {
        self.key("tmdb").or_else(|| Some(DEFAULT_TMDB_KEY.to_string()))
    }

    /// A service key: one pasted on this machine wins, otherwise the one
    /// couch was built with (repo secrets COUCH_*_KEY), shared by everyone.
    pub fn key(&self, name: &str) -> Option<String> {
        self.db.secret(name).or_else(|| baked(name).map(|s| s.to_string()))
    }
}

/// Keys compiled in from the build's environment (never in the source).
pub fn baked(name: &str) -> Option<&'static str> {
    let v = match name {
        "tmdb" => option_env!("COUCH_TMDB_KEY"),
        "tidb" => option_env!("COUCH_TIDB_KEY"),
        "motn" => option_env!("COUCH_MOTN_KEY"),
        "ai" => option_env!("COUCH_AI_KEY"),
        _ => None,
    };
    v.filter(|s| !s.trim().is_empty())
}

/// The key your Clip Previews plugin already shipped with.
pub const DEFAULT_TMDB_KEY: &str = "446c74f0d96977fb4145e8e79fba6bb2";

pub type CoreRef = Arc<Core>;
