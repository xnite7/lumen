//! Philips Hue.
//!
//! Three things, all optional:
//! - **Dim**: lights fade down when playback starts, back up when you pause
//!   or stop (the room's previous state is restored, not guessed).
//! - **Ambilight**: live colour sync from the actual picture through the
//!   Entertainment API (DTLS stream to the bridge, ~25 fps), sampling the
//!   video region with a cheap GPU-side downscale.
//! - **Artwork mood**: while browsing, the room takes on the palette of the
//!   title you're looking at -- slow, soft transitions.

#[cfg(desktop)]
mod entertainment;
/// Screen-synced lights need the desktop screen sampler; phones get dimming
/// and artwork moods only.
#[cfg(mobile)]
mod entertainment {
    use crate::{core::CoreRef, error::{AppError, AppResult}};
    pub struct StreamHandle;
    impl StreamHandle {
        pub fn stop(&self) {}
    }
    pub async fn start(_: CoreRef, _: String, _: String, _: String, _: String, _: Vec<(u8, f32, f32)>) -> AppResult<StreamHandle> {
        Err(AppError::msg("Screen sync is only available on PC"))
    }
}
#[cfg(desktop)]
mod sampler;

use crate::core::CoreRef;
use crate::error::{AppError, AppResult};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub struct Hue {
    core: CoreRef,
    /// Self-signed bridge certificate: a separate client that accepts it,
    /// used for the bridge on the LAN and nothing else.
    lan: reqwest::Client,
    saved_state: Mutex<Option<Vec<(String, Value)>>>,
    playing: AtomicBool,
    stream: Mutex<Option<entertainment::StreamHandle>>,
}

impl Hue {
    pub fn new(core: CoreRef) -> Arc<Self> {
        let lan = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(6))
            .build()
            .expect("lan client");
        Arc::new(Self { core, lan, saved_state: Mutex::new(None), playing: AtomicBool::new(false), stream: Mutex::new(None) })
    }

    fn ip(&self) -> AppResult<String> {
        let ip = self.core.settings.read().hue.bridge_ip.clone();
        if ip.is_empty() {
            return Err(AppError::msg("No bridge selected"));
        }
        Ok(ip)
    }

    fn key(&self) -> AppResult<String> {
        self.core.db.secret("hue_username").ok_or_else(|| AppError::msg("Bridge not paired"))
    }

    pub fn enabled(&self) -> bool {
        let h = &self.core.settings.read().hue;
        h.enabled && !h.bridge_ip.is_empty() && self.core.db.secret("hue_username").is_some()
    }

    // ------------------------------------------------------------- setup

    pub async fn discover(&self) -> AppResult<Value> {
        let v: Value = self
            .core
            .http
            .get_json("https://discovery.meethue.com/", Duration::from_secs(8))
            .await
            .unwrap_or(json!([]));
        let mut out = Vec::new();
        for b in v.as_array().into_iter().flatten() {
            let ip = b.get("internalipaddress").and_then(|i| i.as_str()).unwrap_or("").to_string();
            if ip.is_empty() {
                continue;
            }
            let cfg: Value = match self.lan.get(format!("https://{ip}/api/0/config")).send().await {
                Ok(r) => r.json().await.unwrap_or(Value::Null),
                Err(_) => Value::Null,
            };
            out.push(json!({ "id": b.get("id"), "ip": ip, "name": cfg.get("name"), "model": cfg.get("modelid"), "swversion": cfg.get("swversion") }));
        }
        Ok(json!(out))
    }

    /// Press the bridge's link button, then call this within 30 s.
    pub async fn pair(&self, ip: &str) -> AppResult<Value> {
        let res: Value = self
            .lan
            .post(format!("https://{ip}/api"))
            .json(&json!({ "devicetype": "lumen#pc", "generateclientkey": true }))
            .send()
            .await?
            .json()
            .await?;
        let first = res.get(0).cloned().unwrap_or(Value::Null);
        if let Some(err) = first.get("error") {
            let t = err.get("type").and_then(|t| t.as_i64()).unwrap_or(0);
            return Err(AppError::msg(if t == 101 { "Press the round button on the bridge, then try again".to_string() } else { err.get("description").and_then(|d| d.as_str()).unwrap_or("Pairing failed").to_string() }));
        }
        let user = first.pointer("/success/username").and_then(|u| u.as_str()).ok_or_else(|| AppError::msg("Pairing failed"))?;
        let client_key = first.pointer("/success/clientkey").and_then(|u| u.as_str()).unwrap_or("");
        self.core.db.set_secret("hue_username", Some(user))?;
        self.core.db.set_secret("hue_clientkey", Some(client_key))?;
        {
            let mut s = self.core.settings.write();
            s.hue.bridge_ip = ip.to_string();
            s.hue.enabled = true;
        }
        let s = self.core.settings();
        self.core.db.set("settings", &s)?;
        Ok(json!({ "paired": true }))
    }

    async fn v2_get(&self, path: &str) -> AppResult<Value> {
        let (ip, key) = (self.ip()?, self.key()?);
        let v: Value = self.lan.get(format!("https://{ip}/clip/v2/resource/{path}")).header("hue-application-key", key).send().await?.json().await?;
        Ok(v.get("data").cloned().unwrap_or(json!([])))
    }

    async fn v2_put(&self, path: &str, body: Value) -> AppResult<()> {
        let (ip, key) = (self.ip()?, self.key()?);
        self.lan.put(format!("https://{ip}/clip/v2/resource/{path}")).header("hue-application-key", key).json(&body).send().await?;
        Ok(())
    }

    /// Rooms/zones (for dimming + moods) and entertainment areas (for sync).
    pub async fn overview(&self) -> AppResult<Value> {
        let (rooms, zones, areas, lights) =
            tokio::join!(self.v2_get("room"), self.v2_get("zone"), self.v2_get("entertainment_configuration"), self.v2_get("light"));
        let name = |v: &Value| v.pointer("/metadata/name").cloned().unwrap_or(Value::Null);
        let groups = |list: Value, kind: &str| -> Vec<Value> {
            list.as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| {
                    let gl = r.get("services")?.as_array()?.iter().find(|s| s["rtype"] == "grouped_light")?["rid"].clone();
                    Some(json!({ "id": r["id"], "name": name(r), "kind": kind, "groupedLight": gl }))
                })
                .collect()
        };
        let mut g = groups(rooms.unwrap_or_default(), "room");
        g.extend(groups(zones.unwrap_or_default(), "zone"));
        let areas: Vec<Value> = areas
            .unwrap_or_default()
            .as_array()
            .into_iter()
            .flatten()
            .map(|a| json!({ "id": a["id"], "name": name(a), "status": a["status"], "channels": a["channels"].as_array().map(|c| c.len()) }))
            .collect();
        let lights: Vec<Value> = lights
            .unwrap_or_default()
            .as_array()
            .into_iter()
            .flatten()
            .map(|l| json!({ "id": l["id"], "name": name(l), "on": l.pointer("/on/on"), "gradient": l.get("gradient").is_some() }))
            .collect();
        Ok(json!({ "groups": g, "areas": areas, "lights": lights }))
    }

    fn room_grouped_light(&self, overview: &Value) -> Option<String> {
        let room = self.core.settings.read().hue.room_id.clone();
        overview["groups"].as_array()?.iter().find(|g| g["id"] == room.as_str())?["groupedLight"].as_str().map(|s| s.to_string())
    }

    /// Lights that belong to the chosen room, with their current state so it
    /// can be put back exactly.
    async fn room_lights(&self) -> AppResult<Vec<(String, Value)>> {
        let room_id = self.core.settings.read().hue.room_id.clone();
        let rooms = [self.v2_get("room").await.unwrap_or_default(), self.v2_get("zone").await.unwrap_or_default()];
        let room = rooms.iter().flat_map(|r| r.as_array().cloned().unwrap_or_default()).find(|r| r["id"] == room_id.as_str());
        let Some(room) = room else { return Ok(vec![]) };
        let devices: Vec<String> = room["children"].as_array().into_iter().flatten().filter_map(|c| c["rid"].as_str().map(|s| s.to_string())).collect();
        let lights = self.v2_get("light").await?;
        Ok(lights
            .as_array()
            .into_iter()
            .flatten()
            .filter(|l| {
                let owner = l.pointer("/owner/rid").and_then(|o| o.as_str()).unwrap_or("");
                let id = l["id"].as_str().unwrap_or("");
                devices.iter().any(|d| d == owner || d == id)
            })
            .map(|l| {
                (
                    l["id"].as_str().unwrap_or("").to_string(),
                    json!({ "on": l["on"], "dimming": l["dimming"], "color": l.get("color").map(|c| json!({ "xy": c["xy"] })), "color_temperature": l.get("color_temperature").and_then(|c| c.get("mirek").filter(|m| !m.is_null())).map(|m| json!({ "mirek": m })) }),
                )
            })
            .collect())
    }

    async fn save_room(&self) {
        if self.saved_state.lock().is_some() {
            return;
        }
        if let Ok(s) = self.room_lights().await {
            *self.saved_state.lock() = Some(s);
        }
    }

    async fn restore_room(&self) {
        let saved = self.saved_state.lock().take();
        if let Some(lights) = saved {
            for (id, mut st) in lights {
                if let Some(o) = st.as_object_mut() {
                    o.retain(|_, v| !v.is_null());
                    o.insert("dynamics".into(), json!({ "duration": 1200 }));
                }
                let _ = self.v2_put(&format!("light/{id}"), st).await;
            }
        }
    }

    // ---------------------------------------------------------- behaviour

    pub async fn on_playback(&self, playing: bool) {
        if !self.enabled() {
            return;
        }
        let prev = self.playing.swap(playing, Ordering::SeqCst);
        if prev == playing {
            return;
        }
        let h = self.core.settings.read().hue.clone();
        match (h.playback_mode.as_str(), playing) {
            ("dim", true) => {
                self.save_room().await;
                if let Ok(ov) = self.overview().await {
                    if let Some(gl) = self.room_grouped_light(&ov) {
                        let _ = self.v2_put(&format!("grouped_light/{gl}"), json!({ "dimming": { "brightness": h.dim_brightness }, "dynamics": { "duration": 1500 } })).await;
                    }
                }
            }
            ("ambilight", true) => {
                self.save_room().await;
                if let Err(e) = self.start_sync().await {
                    log::warn!("hue sync: {e}");
                    self.core.emit("toast", json!({ "kind": "warn", "text": format!("Hue sync: {e}") }));
                }
            }
            (_, false) => {
                self.stop_sync().await;
                if h.restore_after {
                    self.restore_room().await;
                }
            }
            _ => {}
        }
    }

    async fn start_sync(&self) -> AppResult<()> {
        let area = self.core.settings.read().hue.area_id.clone();
        if area.is_empty() {
            return Err(AppError::msg("Pick an entertainment area in Settings → Hue"));
        }
        self.stop_sync().await;
        let ip = self.ip()?;
        let user = self.key()?;
        let psk = self.core.db.secret("hue_clientkey").ok_or_else(|| AppError::msg("Re-pair the bridge to enable sync"))?;
        // Channel positions decide which part of the picture each light shows.
        let areas = self.v2_get("entertainment_configuration").await?;
        let cfg = areas.as_array().into_iter().flatten().find(|a| a["id"] == area.as_str()).cloned().ok_or_else(|| AppError::msg("Entertainment area not found"))?;
        let channels: Vec<(u8, f32, f32)> = cfg["channels"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| {
                (
                    c["channel_id"].as_u64().unwrap_or(0) as u8,
                    c.pointer("/position/x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                    c.pointer("/position/z").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                )
            })
            .collect();
        self.v2_put(&format!("entertainment_configuration/{area}"), json!({ "action": "start" })).await?;
        let handle = entertainment::start(self.core.clone(), ip, user, psk, area, channels).await?;
        *self.stream.lock() = Some(handle);
        Ok(())
    }

    async fn stop_sync(&self) {
        let h = self.stream.lock().take();
        if let Some(h) = h {
            h.stop();
            let area = self.core.settings.read().hue.area_id.clone();
            let _ = self.v2_put(&format!("entertainment_configuration/{area}"), json!({ "action": "stop" })).await;
        }
    }

    /// Slow, soft room tint from the artwork palette (browsing only).
    pub async fn mood(&self, colors: Vec<[u8; 3]>) -> AppResult<()> {
        let h = self.core.settings.read().hue.clone();
        if !self.enabled() || !h.artwork_mood || self.playing.load(Ordering::SeqCst) || colors.is_empty() {
            return Ok(());
        }
        let lights = self.room_lights().await?;
        for (i, (id, _)) in lights.iter().enumerate() {
            let c = colors[i % colors.len()];
            let (x, y) = rgb_to_xy(c);
            let _ = self
                .v2_put(&format!("light/{id}"), json!({ "on": { "on": true }, "dimming": { "brightness": h.mood_brightness }, "color": { "xy": { "x": x, "y": y } }, "dynamics": { "duration": 2500 } }))
                .await;
        }
        Ok(())
    }

    pub async fn identify(&self) -> AppResult<()> {
        let lights = self.room_lights().await?;
        for (id, _) in lights {
            let _ = self.v2_put(&format!("light/{id}"), json!({ "identify": { "action": "identify" } })).await;
        }
        Ok(())
    }
}

/// sRGB -> CIE xy (Hue's colour space), gamma-corrected.
pub fn rgb_to_xy(c: [u8; 3]) -> (f32, f32) {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v > 0.04045 {
            ((v + 0.055) / 1.055).powf(2.4)
        } else {
            v / 12.92
        }
    };
    let (r, g, b) = (lin(c[0]), lin(c[1]), lin(c[2]));
    let x = r * 0.664_511 + g * 0.154_324 + b * 0.162_028;
    let y = r * 0.283_881 + g * 0.668_433 + b * 0.047_685;
    let z = r * 0.000_088 + g * 0.072_310 + b * 0.986_039;
    let s = x + y + z;
    if s <= 0.0 {
        (0.3127, 0.3290)
    } else {
        (x / s, y / s)
    }
}
