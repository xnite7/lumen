use serde::{Serialize, Serializer};

/// Every command returns this. It serialises to a plain string so the
/// frontend can show it in a toast without unwrapping anything.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl AppError {
    pub fn msg(s: impl Into<String>) -> Self {
        AppError::Msg(s.into())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[macro_export]
macro_rules! bail {
    ($($t:tt)*) => { return Err($crate::error::AppError::Msg(format!($($t)*))) };
}
