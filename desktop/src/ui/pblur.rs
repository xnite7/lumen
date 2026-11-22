//! A blur that fades in gradually: the picture sharp at the top, softer and
//! softer towards the bottom (Apple TV's "variable blur"). Made once per
//! picture, in the background, and kept.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

static CACHE: parking_lot::Mutex<Option<HashMap<String, Option<(Arc<gpui::Image>, Instant)>>>> = parking_lot::Mutex::new(None);

/// The softened picture for `url` once it's ready, and how far it has faded in (0..1).
pub fn get(url: &str, cx: &mut gpui::App) -> Option<(Arc<gpui::Image>, f32)> {
    {
        let mut c = CACHE.lock();
        let c = c.get_or_insert_with(HashMap::new);
        match c.get(url) {
            Some(Some((im, at))) => return Some((im.clone(), (at.elapsed().as_secs_f32() / 0.5).min(1.))),
            Some(None) => return None,
            None => {
                if c.len() > 24 {
                    c.clear();
                }
                c.insert(url.to_string(), None);
            }
        }
    }
    let u = url.to_string();
    cx.spawn(async move |cx| {
        let u2 = u.clone();
        let made = couch::rt::spawn(async move {
            let core = couch::state::get().core.clone();
            let (bytes, _) = couch::images::load(&core, &u2).await.ok()?;
            couch::rt::spawn_blocking(move || make(&bytes)).await.ok().flatten()
        })
        .await
        .ok()
        .flatten();
        if let Some(im) = made {
            CACHE.lock().get_or_insert_with(HashMap::new).insert(u, Some((Arc::new(im), Instant::now())));
            cx.update(|cx| cx.refresh_windows());
        }
    })
    .detach();
    None
}

fn make(bytes: &[u8]) -> Option<gpui::Image> {
    use image::imageops::{blur, resize, FilterType};
    let im = image::load_from_memory(bytes).ok()?;
    let w = im.width().min(1600);
    let h = (im.height() as f32 * w as f32 / im.width() as f32).round() as u32;
    let sharp = resize(&im.to_rgba8(), w, h, FilterType::Triangle);
    // three strengths, the strong ones blurred small and scaled back up (fast, smooth)
    let soft = |div: u32, sigma: f32| {
        let small = resize(&sharp, (w / div).max(1), (h / div).max(1), FilterType::Triangle);
        resize(&blur(&small, sigma), w, h, FilterType::Triangle)
    };
    let levels = [sharp.clone(), soft(2, 2.5), soft(4, 4.), soft(8, 5.)];
    let mut out = sharp.clone();
    for y in 0..h {
        // sharp down to 42% of the height, then easing into the strongest blur at the bottom
        let p = ((y as f32 / h as f32 - 0.42) / 0.58).clamp(0., 1.);
        let p = p * p * (3. - 2. * p);
        let lv = p * 3.;
        let (i, f) = (lv.floor() as usize, lv.fract());
        let (a, b) = (&levels[i.min(3)], &levels[(i + 1).min(3)]);
        for x in 0..w {
            let (pa, pb) = (a.get_pixel(x, y), b.get_pixel(x, y));
            let px = out.get_pixel_mut(x, y);
            for c in 0..3 {
                px[c] = (pa[c] as f32 + (pb[c] as f32 - pa[c] as f32) * f) as u8;
            }
            px[3] = 255;
        }
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(out).to_rgb8().write_to(&mut buf, image::ImageFormat::Jpeg).ok()?;
    Some(gpui::Image::from_bytes(gpui::ImageFormat::Jpeg, buf.into_inner()))
}
