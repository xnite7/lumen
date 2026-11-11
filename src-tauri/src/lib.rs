pub mod addons;
pub mod beta;
pub mod commands;
pub mod core;
pub mod db;
pub mod crew;
pub mod discord;
pub mod engine;
pub mod error;
pub mod host;
pub mod http;
pub mod hue;
pub mod images;
pub mod integrations;
pub mod library;
pub mod migrate;
pub mod updates;
pub mod player;
pub mod relay;
pub mod rt;
pub mod settings;
pub mod state;
pub mod streams;
#[cfg(feature = "tauri-app")]
pub mod tauri_host;
#[cfg(desktop)]
pub mod tobii;
pub mod torrent;
#[cfg(not(feature = "tauri-app"))]
pub mod dispatch;

#[cfg(feature = "tauri-app")]
mod app;
#[cfg(feature = "tauri-app")]
pub use app::run;
