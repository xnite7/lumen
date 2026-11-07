//! Discord Rich Presence (desktop): "on the couch · watching <title>" with
//! the Simkl poster, the midnite mark in the corner and a live progress bar.
//!
//! The rule: only titles Simkl knows ever show up. Every session is looked up
//! on Simkl first (by IMDb/TMDB id, or Simkl's own file-name match for raw
//! links); no match = no presence, and what Discord shows is Simkl's own
//! title, year and poster, never the stream's file name or the addon's text.
//! So a random link, a home video or anything Simkl doesn't catalogue stays
//! invisible.
//!
//! The IPC client lives on its own thread (it blocks), fed by a channel, and
//! reconnects by itself when Discord starts after couch.

use crate::core::Core;
use serde_json::Value;

/// couch's Discord application (its name is what friends see).
const DEFAULT_APP_ID: &str = "1568062298721886290";

#[derive(Debug, Clone, PartialEq)]
pub struct Known {
    pub title: String,
    pub year: Option<u64>,
    pub poster: Option<String>,
    pub link: Option<String>,
    /// "S2 · E4" / "S2 · E4 · The Episode" for shows, None for films
    pub episode: Option<String>,
}

/// What to show for a session: Simkl's title and poster when Simkl knows it,
/// otherwise the title's own metadata (name, year, poster).
pub async fn identify(core: &Core, meta: &Value, video_id: &str, episode_title: Option<&str>, ident: Option<&Value>) -> Option<Known> {
    if let Some(k) = from_simkl(core, meta, video_id, episode_title, ident).await {
        return Some(k);
    }
    from_meta(meta, video_id, episode_title)
}

fn from_meta(meta: &Value, video_id: &str, episode_title: Option<&str>) -> Option<Known> {
    let title = meta.get("name").and_then(|t| t.as_str()).filter(|t| !t.trim().is_empty())?.to_string();
    let parts: Vec<&str> = video_id.split(':').collect();
    let series = meta.get("type").and_then(|t| t.as_str()) == Some("series");
    let episode = if series && parts.len() >= 3 {
        let (s, e) = (parts[parts.len() - 2], parts[parts.len() - 1]);
        Some(match episode_title.filter(|t| !t.trim().is_empty() && !t.starts_with("Episode ")) {
            Some(t) => format!("S{s} · E{e} · {t}"),
            None => format!("S{s} · E{e}"),
        })
    } else {
        None
    };
    let year = meta
        .get("year")
        .and_then(|y| y.as_u64())
        .or_else(|| meta.get("releaseInfo").and_then(|y| y.as_str()).and_then(|y| y.get(0..4)).and_then(|y| y.parse().ok()));
    let id = parts.first().copied().unwrap_or("");
    Some(Known {
        title,
        year,
        poster: meta.get("poster").and_then(|p| p.as_str()).filter(|p| p.starts_with("http")).map(|p| p.to_string()),
        link: id.starts_with("tt").then(|| format!("https://www.imdb.com/title/{id}/")),
        episode,
    })
}

/// Look a session up on Simkl. `ident` is the /search/file result for raw
/// links (already Simkl data).
async fn from_simkl(core: &Core, meta: &Value, video_id: &str, episode_title: Option<&str>, ident: Option<&Value>) -> Option<Known> {
    use crate::integrations::simkl;
    let kind = meta.get("type").and_then(|t| t.as_str()).unwrap_or("movie");
    let parts: Vec<&str> = video_id.split(':').collect();

    let (query, season, episode) = if let Some(found) = ident {
        let item = found.get("movie").or_else(|| found.get("show"))?;
        let sid = item.pointer("/ids/simkl").or_else(|| item.pointer("/ids/simkl_id"))?;
        let s = found.pointer("/episode/season").and_then(|v| v.as_u64());
        let e = found.pointer("/episode/number").and_then(|v| v.as_u64());
        (format!("simkl={}", sid.to_string().trim_matches('"')), s, e)
    } else {
        let (s, e) = if kind == "series" && parts.len() >= 3 {
            (parts[parts.len() - 2].parse().ok(), parts[parts.len() - 1].parse().ok())
        } else {
            (None, None)
        };
        let id = parts.first().copied().unwrap_or("");
        let q = if id.starts_with("tt") {
            format!("imdb={id}")
        } else if id == "tmdb" && parts.len() >= 2 {
            format!("tmdb={}&type={}", parts[1], if kind == "series" { "show" } else { "movie" })
        } else {
            return None; // not a catalogued id: the meta fallback takes it
        };
        (q, s, e)
    };

    if !simkl::connected(core) {
        return None; // not signed in to Simkl: the meta fallback takes it
    }
    let v = simkl::get(core, &format!("/search/id?{query}")).await.ok()?;
    let hit = v.as_array().and_then(|a| a.first()).or_else(|| v.as_object().map(|_| &v))?;
    let title = hit.get("title").and_then(|t| t.as_str()).filter(|t| !t.is_empty())?.to_string();
    let typ = hit.get("type").and_then(|t| t.as_str()).unwrap_or(if kind == "series" { "tv" } else { "movie" });
    let ids = hit.get("ids");
    let sid = ids.and_then(|i| i.get("simkl").or_else(|| i.get("simkl_id"))).map(|s| s.to_string().trim_matches('"').to_string());
    let slug = ids.and_then(|i| i.get("slug")).and_then(|s| s.as_str()).unwrap_or("");
    let section = match typ {
        "movie" => "movies",
        "anime" => "anime",
        _ => "tv",
    };
    let episode = match (season, episode) {
        (Some(s), Some(e)) => Some(match episode_title.filter(|t| !t.trim().is_empty() && !t.starts_with("Episode ")) {
            Some(t) => format!("S{s} · E{e} · {t}"),
            None => format!("S{s} · E{e}"),
        }),
        _ => None,
    };
    Some(Known {
        title,
        year: hit.get("year").and_then(|y| y.as_u64()),
        poster: hit.get("poster").and_then(|p| p.as_str()).map(|p| format!("https://simkl.in/posters/{p}_m.jpg")),
        link: sid.map(|id| format!("https://simkl.com/{section}/{id}/{slug}")),
        episode,
    })
}

#[derive(Debug, Clone)]
pub enum Msg {
    Show { known: Known, pos: f64, duration: f64, paused: bool },
    Clear,
}

#[cfg(desktop)]
mod imp {
    use super::{Known, Msg};
    use discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, StatusDisplayType, Timestamps};
    use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
    use once_cell::sync::OnceCell;
    use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
    use std::time::Duration;

    static TX: OnceCell<parking_lot::Mutex<Sender<(String, Msg)>>> = OnceCell::new();
    /// The midnite mark for the small corner image: the crew server's
    /// /logo.png when there is one, else an art asset named "midnite"
    /// uploaded to the Discord app.
    pub static LOGO: parking_lot::Mutex<String> = parking_lot::Mutex::new(String::new());

    pub fn send(app_id: &str, m: Msg) {
        let tx = TX.get_or_init(|| {
            let (tx, rx) = channel::<(String, Msg)>();
            std::thread::Builder::new()
                .name("discord-rpc".into())
                .spawn(move || run(rx))
                .ok();
            parking_lot::Mutex::new(tx)
        });
        let _ = tx.lock().send((app_id.to_string(), m));
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
    }

    fn run(rx: std::sync::mpsc::Receiver<(String, Msg)>) {
        let mut client: Option<(String, DiscordIpcClient)> = None;
        let mut pending: Option<(String, Msg)> = None;
        loop {
            // newest message wins; retry the last one every 15 s while Discord is closed
            let next = match rx.recv_timeout(Duration::from_secs(15)) {
                Ok(m) => Some(m),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            };
            if let Some(m) = next {
                pending = Some(m);
                while let Ok(m) = rx.try_recv() {
                    pending = Some(m);
                }
            }
            let Some((app_id, msg)) = pending.clone() else { continue };
            if client.as_ref().map_or(true, |(id, _)| id != &app_id) {
                if let Some((_, mut c)) = client.take() {
                    let _ = c.close();
                }
                let mut c = DiscordIpcClient::new(&app_id);
                if c.connect().is_err() {
                    *STATUS.lock() = "Discord isn't open".into();
                    continue; // Discord not running: try again later
                }
                client = Some((app_id.clone(), c));
            }
            let Some((_, c)) = client.as_mut() else { continue };
            // Discord answers every command: read the answer (an unread pipe
            // backs up and Discord drops us) and see whether it said no.
            let ok = match &msg {
                Msg::Clear => c.clear_activity().is_ok() && answer(c).is_ok(),
                Msg::Show { known, pos, duration, paused } => {
                    match c.set_activity(activity(known, *pos, *duration, *paused, true)).map_err(|e| e.to_string()).and_then(|_| answer(c)) {
                        Ok(()) => true,
                        // rejected: the plain version (no links, no extras)
                        Err(e) if e.starts_with("discord: ") => {
                            *STATUS.lock() = e;
                            c.set_activity(activity(known, *pos, *duration, *paused, false)).is_ok() && answer(c).is_ok()
                        }
                        Err(_) => false,
                    }
                }
            };
            if ok && !STATUS.lock().starts_with("discord: ") {
                *STATUS.lock() = "showing on your profile".into();
            }
            if ok {
                pending = None;
            } else {
                let _ = c.close();
                client = None;
            }
        }
    }

    /// What happened last (for Settings): connected or not, and any refusal.
    pub static STATUS: parking_lot::Mutex<String> = parking_lot::Mutex::new(String::new());

    fn answer(c: &mut DiscordIpcClient) -> Result<(), String> {
        let (_, v) = c.recv().map_err(|e| e.to_string())?;
        if v.get("evt").and_then(|e| e.as_str()) == Some("ERROR") {
            let m = v.pointer("/data/message").and_then(|m| m.as_str()).unwrap_or("refused");
            return Err(format!("discord: {m}"));
        }
        Ok(())
    }

    fn activity(k: &Known, pos: f64, duration: f64, paused: bool, rich: bool) -> Activity<'_> {
        let year = k.year.map(|y| y.to_string());
        let what = k.episode.clone().or(year).unwrap_or_else(|| "a film".into());
        let state = if paused { format!("on the couch · paused · {what}") } else { format!("on the couch · {what}") };
        let mut a = Activity::new().activity_type(ActivityType::Watching).details(k.title.clone()).state(state);
        if rich {
            a = a.status_display_type(StatusDisplayType::Details);
            if let Some(l) = &k.link {
                a = a.details_url(l.clone()).buttons(vec![Button::new("View on Simkl", l.clone())]);
            }
        }
        let logo = LOGO.lock().clone();
        let mut assets = Assets::new()
            .large_text(k.title.clone())
            .small_image(if logo.is_empty() { "midnite".to_string() } else { logo })
            .small_text("midnite™ couch");
        if let Some(p) = &k.poster {
            assets = assets.large_image(p.clone());
            if let (true, Some(l)) = (rich, &k.link) {
                assets = assets.large_url(l.clone());
            }
        }
        a = a.assets(assets);
        if !paused && duration > 0.0 {
            let start = now_ms() - (pos * 1000.0) as i64;
            a = a.timestamps(Timestamps::new().start(start).end(start + (duration * 1000.0) as i64));
        }
        a
    }
}

/// How Discord RPC is doing, for Settings.
pub fn status() -> String {
    #[cfg(desktop)]
    {
        return imp::STATUS.lock().clone();
    }
    #[cfg(not(desktop))]
    String::new()
}

/// Tell Discord. A no-op when presence is off, no app id is set, or on iOS.
pub fn update(core: &Core, m: Msg) {
    #[cfg(desktop)]
    {
        let (on, id) = {
            let s = core.settings.read();
            (s.integrations.discord_presence, s.integrations.discord_app_id.trim().to_string())
        };
        let id = if id.is_empty() { DEFAULT_APP_ID.to_string() } else { id };
        *imp::LOGO.lock() = format!("{}/logo.png", crate::crew::CREW_URL.trim_end_matches('/'));
        let m = if on { m } else { Msg::Clear };
        imp::send(&id, m);
    }
    #[cfg(not(desktop))]
    {
        let _ = (core, m);
    }
}
