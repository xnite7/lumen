//! One SQLite file for everything that has to survive a restart: settings,
//! secrets, the library (in Stremio's own LibraryItem shape so it syncs both
//! ways), and a small HTTP response cache.
//!
//! Every query here is a single indexed lookup or write, so holding a mutex
//! for its duration is cheaper than shipping it to a blocking pool.

use crate::error::AppResult;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;

pub struct Db {
    conn: Mutex<Connection>,
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;
            CREATE TABLE IF NOT EXISTS kv (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS library (
                id    TEXT PRIMARY KEY,
                json  TEXT NOT NULL,
                mtime INTEGER NOT NULL,
                dirty INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS library_mtime ON library(mtime DESC);
            CREATE TABLE IF NOT EXISTS http_cache (
                url     TEXT PRIMARY KEY,
                body    BLOB NOT NULL,
                expires INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS recents (
                q  TEXT PRIMARY KEY,
                at INTEGER NOT NULL
            );
            "#,
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    // ---------------------------------------------------------------- kv

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let c = self.conn.lock();
        let raw: Option<String> = c
            .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        raw.and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> AppResult<()> {
        let s = serde_json::to_string(value)?;
        self.conn.lock().execute(
            "INSERT INTO kv(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, s],
        )?;
        Ok(())
    }

    pub fn del(&self, key: &str) -> AppResult<()> {
        self.conn.lock().execute("DELETE FROM kv WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Secrets live in the same table under a prefix. This is a single-user
    /// app on its owner's own machine; the OS keychain would add a native
    /// dependency per platform to protect against an attacker who already
    /// has the user's files.
    pub fn secret(&self, name: &str) -> Option<String> {
        self.get::<String>(&format!("secret:{name}")).filter(|s| !s.is_empty())
    }

    pub fn set_secret(&self, name: &str, value: Option<&str>) -> AppResult<()> {
        let key = format!("secret:{name}");
        match value.filter(|v| !v.is_empty()) {
            Some(v) => self.set(&key, &v.to_string()),
            None => self.del(&key),
        }
    }

    // ----------------------------------------------------------- library

    pub fn library_all(&self) -> AppResult<Vec<serde_json::Value>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT json FROM library ORDER BY mtime DESC")?;
        let rows = st
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect();
        Ok(rows)
    }

    pub fn library_get(&self, id: &str) -> Option<serde_json::Value> {
        let c = self.conn.lock();
        c.query_row("SELECT json FROM library WHERE id = ?1", [id], |r| r.get::<_, String>(0))
            .optional()
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn library_put(&self, id: &str, item: &serde_json::Value, mtime: i64, dirty: bool) -> AppResult<()> {
        self.conn.lock().execute(
            "INSERT INTO library(id, json, mtime, dirty) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET json = excluded.json, mtime = excluded.mtime,
                                           dirty = MAX(library.dirty, excluded.dirty)",
            params![id, item.to_string(), mtime, dirty as i64],
        )?;
        Ok(())
    }

    pub fn library_dirty(&self) -> AppResult<Vec<serde_json::Value>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT json FROM library WHERE dirty = 1")?;
        let rows = st
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect();
        Ok(rows)
    }

    pub fn library_mark_clean(&self, ids: &[String]) -> AppResult<()> {
        let c = self.conn.lock();
        for id in ids {
            c.execute("UPDATE library SET dirty = 0 WHERE id = ?1", [id])?;
        }
        Ok(())
    }

    // -------------------------------------------------------- http cache

    pub fn cache_get(&self, url: &str) -> Option<Vec<u8>> {
        let c = self.conn.lock();
        c.query_row(
            "SELECT body FROM http_cache WHERE url = ?1 AND expires > ?2",
            params![url, now_ms()],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .optional()
        .ok()
        .flatten()
    }

    pub fn cache_put(&self, url: &str, body: &[u8], ttl_ms: i64) {
        let _ = self.conn.lock().execute(
            "INSERT INTO http_cache(url, body, expires) VALUES(?1, ?2, ?3)
             ON CONFLICT(url) DO UPDATE SET body = excluded.body, expires = excluded.expires",
            params![url, body, now_ms() + ttl_ms],
        );
    }

    pub fn cache_prune(&self) {
        let _ = self.conn.lock().execute("DELETE FROM http_cache WHERE expires < ?1", [now_ms()]);
    }

    pub fn cache_clear(&self) -> AppResult<()> {
        self.conn.lock().execute("DELETE FROM http_cache", [])?;
        Ok(())
    }

    // ----------------------------------------------------------- recents

    pub fn recents(&self, limit: usize) -> Vec<String> {
        let c = self.conn.lock();
        let Ok(mut st) = c.prepare("SELECT q FROM recents ORDER BY at DESC LIMIT ?1") else { return vec![] };
        st.query_map([limit as i64], |r| r.get::<_, String>(0))
            .map(|it| it.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    pub fn recent_add(&self, q: &str) {
        let _ = self.conn.lock().execute(
            "INSERT INTO recents(q, at) VALUES(?1, ?2) ON CONFLICT(q) DO UPDATE SET at = excluded.at",
            params![q, now_ms()],
        );
    }

    pub fn recents_clear(&self) {
        let _ = self.conn.lock().execute("DELETE FROM recents", []);
    }
}
