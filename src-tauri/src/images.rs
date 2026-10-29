//! `img://` -- every poster and backdrop goes through here.
//!
//! - Disk-cached forever (content-addressed), so a second visit to any page
//!   paints from disk with zero network.
//! - Fetched with the shared client, so DoH applies to images too.
//! - On a block (DNS failure, 403/451, connection reset) it retries once
//!   through your image proxy -- the Glass Polish `proxyBlockedImages`
//!   behaviour, minus the onerror dance in the DOM.

use crate::core::Core;
use crate::error::{AppError, AppResult};
use base64::Engine;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Duration;

fn cache_path(core: &Core, url: &str) -> PathBuf {
    let h = hex::encode(Sha256::digest(url.as_bytes()));
    core.cache_dir.join("img").join(&h[..2]).join(&h[2..])
}

fn sniff(bytes: &[u8]) -> &'static str {
    match bytes {
        [0xFF, 0xD8, ..] => "image/jpeg",
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        [b'G', b'I', b'F', ..] => "image/gif",
        [0, 0, 0, _, b'f', b't', b'y', b'p', b'a', b'v', b'i', b'f', ..] => "image/avif",
        b if b.starts_with(b"<svg") || b.starts_with(b"<?xml") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

async fn fetch_direct(core: &Core, url: &str) -> Result<Vec<u8>, (bool, String)> {
    let res = core
        .http
        .client()
        .get(url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| (true, e.to_string()))?;
    let st = res.status();
    if st.as_u16() == 404 {
        // A genuine miss: the proxy can't invent an image.
        return Err((false, "404".into()));
    }
    if !st.is_success() {
        return Err((matches!(st.as_u16(), 403 | 451 | 429) || st.is_server_error(), st.to_string()));
    }
    let ct = res.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let bytes = res.bytes().await.map_err(|e| (true, e.to_string()))?;
    if !ct.starts_with("image/") && sniff(&bytes) == "application/octet-stream" {
        // Block pages come back as 200 text/html.
        return Err((true, format!("not an image ({ct})")));
    }
    Ok(bytes.to_vec())
}

pub async fn load(core: &Core, url: &str) -> AppResult<(Vec<u8>, &'static str)> {
    let path = cache_path(core, url);
    if let Ok(bytes) = tokio::fs::read(&path).await {
        let ct = sniff(&bytes);
        return Ok((bytes, ct));
    }
    let net = core.settings.read().network.clone();
    let bytes = match fetch_direct(core, url).await {
        Ok(b) => b,
        Err((retryable, why)) => {
            if !(retryable && net.proxy_failed_images && !net.image_proxy.is_empty()) {
                return Err(AppError::msg(why));
            }
            let bare = url.trim_start_matches("https://").trim_start_matches("http://");
            let proxied = format!("{}{}", net.image_proxy, urlencoding::encode(bare));
            fetch_direct(core, &proxied).await.map_err(|(_, w)| AppError::msg(w))?
        }
    };
    if let Some(dir) = path.parent() {
        let _ = tokio::fs::create_dir_all(dir).await;
    }
    let _ = tokio::fs::write(&path, &bytes).await;
    let ct = sniff(&bytes);
    Ok((bytes, ct))
}

pub fn decode_path(path: &str) -> Option<String> {
    let enc = path.trim_start_matches('/');
    let enc = enc.split('?').next().unwrap_or(enc);
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(enc).ok()?;
    String::from_utf8(raw).ok()
}

#[derive(Debug, Clone, Serialize)]
pub struct Palette {
    /// Most vibrant first.
    pub colors: Vec<[u8; 3]>,
    pub average: [u8; 3],
    pub dark: bool,
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (if h < 0.0 { h + 360.0 } else { h }, if max == 0.0 { 0.0 } else { d / max }, max)
}

/// A handful of colours that actually represent the artwork: tiny k-means
/// over a 64px thumbnail, then ranked by saturation*value so the accent is
/// the colour you'd point at, not the muddy average.
pub fn palette_of(bytes: &[u8]) -> AppResult<Palette> {
    let img = image::load_from_memory(bytes).map_err(|e| AppError::msg(e.to_string()))?;
    let small = img.thumbnail(64, 64).to_rgb8();
    let px: Vec<[f32; 3]> = small.pixels().map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]).collect();
    if px.is_empty() {
        return Err(AppError::msg("empty image"));
    }
    let k = 6usize;
    let mut centers: Vec<[f32; 3]> = (0..k).map(|i| px[i * px.len() / k]).collect();
    let mut assign = vec![0usize; px.len()];
    for _ in 0..8 {
        for (i, p) in px.iter().enumerate() {
            let mut best = (f32::MAX, 0);
            for (c, ctr) in centers.iter().enumerate() {
                let d = (p[0] - ctr[0]).powi(2) + (p[1] - ctr[1]).powi(2) + (p[2] - ctr[2]).powi(2);
                if d < best.0 {
                    best = (d, c);
                }
            }
            assign[i] = best.1;
        }
        let mut sums = vec![[0f32; 4]; k];
        for (i, p) in px.iter().enumerate() {
            let s = &mut sums[assign[i]];
            s[0] += p[0];
            s[1] += p[1];
            s[2] += p[2];
            s[3] += 1.0;
        }
        for (c, s) in sums.iter().enumerate() {
            if s[3] > 0.0 {
                centers[c] = [s[0] / s[3], s[1] / s[3], s[2] / s[3]];
            }
        }
    }
    let mut counts = vec![0usize; k];
    for a in &assign {
        counts[*a] += 1;
    }
    let mut ranked: Vec<([f32; 3], f32)> = centers
        .iter()
        .zip(counts.iter())
        .filter(|(_, n)| **n > px.len() / 40)
        .map(|(c, n)| {
            let (_, s, v) = rgb_to_hsv(c[0] / 255.0, c[1] / 255.0, c[2] / 255.0);
            // Vibrancy, nudged by how much of the image it covers.
            (*c, s * v * (0.6 + 0.4 * (*n as f32 / px.len() as f32).sqrt()))
        })
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let avg = px.iter().fold([0f32; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]]);
    let n = px.len() as f32;
    let average = [(avg[0] / n) as u8, (avg[1] / n) as u8, (avg[2] / n) as u8];
    let lum = 0.2126 * average[0] as f32 + 0.7152 * average[1] as f32 + 0.0722 * average[2] as f32;
    Ok(Palette {
        colors: ranked.iter().map(|(c, _)| [c[0] as u8, c[1] as u8, c[2] as u8]).collect(),
        average,
        dark: lum < 110.0,
    })
}

pub async fn palette(core: &Core, url: &str) -> AppResult<Palette> {
    let (bytes, _) = load(core, url).await?;
    tokio::task::spawn_blocking(move || palette_of(&bytes)).await.map_err(|e| AppError::msg(e.to_string()))?
}

pub async fn cache_size(core: &Core) -> u64 {
    fn walk(p: &std::path::Path) -> u64 {
        std::fs::read_dir(p)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| {
                        let m = e.metadata().ok();
                        if m.as_ref().map_or(false, |m| m.is_dir()) {
                            walk(&e.path())
                        } else {
                            m.map_or(0, |m| m.len())
                        }
                    })
                    .sum()
            })
            .unwrap_or(0)
    }
    let dir = core.cache_dir.join("img");
    tokio::task::spawn_blocking(move || walk(&dir)).await.unwrap_or(0)
}

pub async fn clear(core: &Core) -> AppResult<()> {
    let _ = tokio::fs::remove_dir_all(core.cache_dir.join("img")).await;
    Ok(())
}
