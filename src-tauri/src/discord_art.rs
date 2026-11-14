//! The big picture on your Discord profile: the title's wide art on a cinema
//! screen, the projector's beam reaching it over a row of heads, a little
//! flicker and dust in the light. Made here once per title as a small looping
//! GIF and handed to the crew server, which serves it to Discord.

use crate::core::Core;
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, Rgba, RgbaImage};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};

const W: u32 = 256;
const H: u32 = 256;
const FRAMES: u32 = 12;

/// the screen, as a box on the card
const SX0: f32 = 22.;
const SX1: f32 = 234.;
const SY0: f32 = 30.;
const SY1: f32 = 149.;

static MADE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
static MAKING: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// The card's address for this title, if it's been made already.
pub fn url_for(key: &str) -> Option<String> {
    MADE.lock().as_ref().and_then(|m| m.get(key).cloned())
}

/// Make (and hand in) this title's card, once; `done` runs when it's up.
pub fn ensure(_core: &Core, key: String, art: String, done: impl FnOnce() + Send + 'static) {
    if url_for(&key).is_some() || !MAKING.lock().get_or_insert_with(HashSet::new).insert(key.clone()) {
        return;
    }
    crate::rt::spawn(async move {
        let Some(st) = crate::state::try_get() else { return };
        let made = make(&st.core, &key, &art).await;
        MAKING.lock().get_or_insert_with(HashSet::new).remove(&key);
        if let Some(u) = made {
            MADE.lock().get_or_insert_with(HashMap::new).insert(key, u);
            done();
        }
    });
}

async fn make(core: &Core, key: &str, art: &str) -> Option<String> {
    let (bytes, _) = crate::images::load(core, art).await.ok()?;
    let gif = crate::rt::spawn_blocking(move || card(&bytes)).await.ok()??;
    crate::crew::login(core).await.ok()?;
    let (base, auth) = crate::crew::server(core)?;
    let r = core
        .http
        .client()
        .put(format!("{}/v1/art/{}", base.trim_end_matches('/'), key))
        .header("authorization", auth)
        .header("content-type", "image/gif")
        .body(gif)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .ok()?;
    if !r.status().is_success() {
        return None;
    }
    let v: serde_json::Value = r.json().await.ok()?;
    v.get("url").and_then(|u| u.as_str()).map(String::from)
}

/// a small repeatable "random" number in 0..1
fn hash(a: u32, b: u32) -> f32 {
    let mut x = a.wrapping_mul(374_761_393) ^ b.wrapping_mul(668_265_263);
    x = (x ^ (x >> 13)).wrapping_mul(1_274_126_177);
    ((x ^ (x >> 16)) & 0xffff) as f32 / 65535.
}

/// The looping card from the title's wide art.
pub fn card(art: &[u8]) -> Option<Vec<u8>> {
    let src = image::load_from_memory(art).ok()?;
    let (sw, sh) = ((SX1 - SX0) as u32, (SY1 - SY0) as u32);
    let shot = src.resize_to_fill(sw, sh, image::imageops::FilterType::Lanczos3).to_rgba8();
    // the screen's overall colour lights the room
    let (mut ar, mut ag, mut ab) = (0f32, 0f32, 0f32);
    for p in shot.pixels() {
        ar += p[0] as f32;
        ag += p[1] as f32;
        ab += p[2] as f32;
    }
    let n = (sw * sh) as f32;
    let glow = [ar / n, ag / n, ab / n];
    let (px, py) = (W as f32 / 2., H as f32 + 30.); // the projector, behind you
    let mut frames = vec![];
    for f in 0..FRAMES {
        let flick = 0.9 + 0.1 * hash(f, 7);
        let beam_flick = 0.85 + 0.15 * hash(f, 11);
        let mut img = RgbaImage::new(W, H);
        for y in 0..H {
            for x in 0..W {
                let (xf, yf) = (x as f32 + 0.5, y as f32 + 0.5);
                // the dark room
                let g = 1. - yf / H as f32;
                let mut c = [10. + 6. * g, 8. + 4. * g, 12. + 6. * g];
                let on_screen = xf >= SX0 && xf < SX1 && yf >= SY0 && yf < SY1;
                if on_screen {
                    let p = shot.get_pixel((xf - SX0) as u32, (yf - SY0) as u32);
                    c = [p[0] as f32 * flick, p[1] as f32 * flick, p[2] as f32 * flick];
                } else {
                    // the screen's light spilling round its edges
                    let dx = (SX0 - xf).max(xf - SX1).max(0.);
                    let dy = (SY0 - yf).max(yf - SY1).max(0.);
                    let d = (dx * dx + dy * dy).sqrt();
                    let k = (1. - d / 26.).max(0.).powi(2) * 0.45 * flick;
                    for i in 0..3 {
                        c[i] += glow[i] * k;
                    }
                    // the beam, from the projector to the screen
                    if yf > SY1 {
                        let t = (py - yf) / (py - SY1); // 0 at the projector, 1 at the screen
                        let half = (SX1 - SX0) / 2. * t;
                        let off = (xf - px).abs();
                        if t > 0. && off < half {
                            let edge = 1. - off / half;
                            let k = 0.22 * edge.powf(0.7) * (0.45 + 0.55 * t) * beam_flick;
                            c[0] += 255. * k;
                            c[1] += 244. * k;
                            c[2] += 222. * k;
                        }
                    }
                }
                img.put_pixel(x, y, Rgba([c[0].min(255.) as u8, c[1].min(255.) as u8, c[2].min(255.) as u8, 255]));
            }
        }
        // dust drifting through the light
        for d in 0..46u32 {
            let t = 0.15 + 0.85 * hash(d, 1);
            let drift = (hash(d, 2) - 0.5) * 2.;
            let yy = py - t * (py - SY1) - (f as f32) * (0.6 + hash(d, 3)) * 0.9;
            let half = (SX1 - SX0) / 2. * ((py - yy) / (py - SY1));
            let xx = px + drift * half * 0.85 + (f as f32) * (hash(d, 4) - 0.5) * 0.8;
            if yy <= SY1 || yy >= H as f32 || xx < 0. || xx >= W as f32 {
                continue;
            }
            let b = 110. * (0.4 + 0.6 * hash(d, f + 20));
            let p = img.get_pixel_mut(xx as u32, yy as u32);
            for i in 0..3 {
                p[i] = (p[i] as f32 + b).min(255.) as u8;
            }
        }
        // a row of heads in front of you, lit at the top by the screen
        for (row, (cy, r, step, shift)) in [(H as f32 - 26., 15., 34., 0.), (H as f32 - 4., 19., 44., 22.)].into_iter().enumerate() {
            let mut cx = -shift;
            while cx < W as f32 + r {
                for y in ((cy - r) as i32).max(0)..(H as i32) {
                    for x in ((cx - r) as i32).max(0)..((cx + r) as i32).min(W as i32) {
                        let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                        // a head, and shoulders below it
                        let inside = dx * dx + dy * dy <= r * r || (dy > r * 0.6 && dx.abs() < r * 1.5);
                        if inside {
                            let rim = ((r * r - (dx * dx + dy * dy)).max(0.).sqrt() < 2.2 && dy < -r * 0.3) as i32 as f32;
                            let k = 0.18 * rim * flick;
                            let base = if row == 0 { 9. } else { 5. };
                            img.put_pixel(x as u32, y as u32, Rgba([(base + glow[0] * k) as u8, (base + glow[1] * k) as u8, (base + 2. + glow[2] * k) as u8, 255]));
                        }
                    }
                }
                cx += step;
            }
        }
        frames.push(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(110, 1)));
    }
    let mut out = Vec::new();
    {
        let mut enc = GifEncoder::new_with_speed(&mut out, 12);
        enc.set_repeat(Repeat::Infinite).ok()?;
        enc.encode_frames(frames).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn sample_card() {
        let Ok(p) = std::env::var("CARD_SAMPLE") else { return };
        let b = std::fs::read(&p).unwrap();
        let gif = super::card(&b).unwrap();
        std::fs::write("/home/claude/card.gif", &gif).unwrap();
        println!("gif bytes: {}", gif.len());
    }
}
