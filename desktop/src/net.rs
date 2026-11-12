//! Images: GPUI asks for a URL, couch's image cache answers (the same disk
//! cache, DoH and headers the engine uses everywhere).
use gpui::http_client::{http, AsyncBody, HttpClient, Response, Url};
use std::sync::Arc;

pub struct CouchHttp {
    core: couch::core::CoreRef,
}

impl CouchHttp {
    pub fn new(core: couch::core::CoreRef) -> Arc<Self> {
        Arc::new(Self { core })
    }
}

impl HttpClient for CouchHttp {
    fn user_agent(&self) -> Option<&http::HeaderValue> {
        None
    }
    fn proxy(&self) -> Option<&Url> {
        None
    }
    fn send(&self, req: http::Request<AsyncBody>) -> futures::future::BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let (url, fit) = split_fit(&req.uri().to_string());
        let core = self.core.clone();
        Box::pin(async move {
            let got = couch::rt::spawn(async move {
                match couch::images::load(&core, &url).await {
                    Ok(x) => Ok(x),
                    // metahub has no logo for it: TMDB often does
                    Err(e) => match metahub_logo_imdb(&url) {
                        Some(imdb) => match couch::integrations::tmdb::logo_for_imdb(&core, &imdb).await {
                            Some(u) => couch::images::load(&core, &u).await.map_err(|e| e.to_string()),
                            None => Err(e.to_string()),
                        },
                        None => Err(e.to_string()),
                    },
                }
            })
            .await?
            .map_err(anyhow::Error::msg)?;
            let (mut bytes, mut ct) = got;
            if let Some((w, k)) = fit {
                // GPUI samples a picture with no mip levels, so a big picture drawn
                // small shimmers on its edges: shrink it here, properly, once
                if let Ok(Some(png)) = couch::rt::spawn_blocking({
                    let b = bytes.clone();
                    move || shrink(&b, w, k)
                })
                .await
                {
                    bytes = png;
                    ct = "image/png";
                }
            }
            Ok(Response::builder().status(200).header("content-type", ct).body(AsyncBody::from(bytes))?)
        })
    }
}

const FIT: &str = "couchimg=";

/// A picture URL that comes back no wider than `w` logical pixels (on this
/// screen's scale), scaled down with a good filter.
pub fn fit(url: &str, w: f32) -> String {
    tag(url, w, 'f')
}

/// A title logo: like `fit`, with the empty space on its left, above and under
/// it cut away, so it lines up with the text beside and below it.
pub fn logo(url: &str, w: f32) -> String {
    tag(url, w, 't')
}

/// A profile picture: a circle `w` across whose edge fades out softly.
pub fn face(url: &str, w: f32) -> String {
    tag(url, w, 'a')
}

fn tag(url: &str, w: f32, k: char) -> String {
    let px = (w * crate::ui::scale()).ceil() as u32;
    if url.starts_with("data:") || url.ends_with(".svg") || px == 0 {
        return url.to_string();
    }
    format!("{url}{}{FIT}{k}{px}", if url.contains('?') { '&' } else { '?' })
}

/// (the real url, max width, what to do: 'f' fit, 't' trim a logo, 'a' a face)
fn split_fit(url: &str) -> (String, Option<(u32, char)>) {
    let Some(i) = url.rfind(FIT) else { return (url.to_string(), None) };
    let v = &url[i + FIT.len()..];
    let k = v.chars().next().unwrap_or('f');
    let w = v.get(1..).and_then(|n| n.parse().ok());
    (url[..i - 1].to_string(), w.map(|w| (w, k)))
}

fn face_png(bytes: &[u8], w: u32) -> Option<Vec<u8>> {
    let im = image::load_from_memory(bytes).ok()?;
    let side = im.width().min(im.height()).max(1);
    let sq = im.crop_imm((im.width() - side) / 2, (im.height() - side) / 2, side, side);
    let w = w.clamp(8, 512);
    let mut px = sq.resize_exact(w, w, image::imageops::FilterType::Lanczos3).to_rgba8();
    let r = w as f32 / 2.;
    // a soft rim: the outer ~14% of the radius fades to nothing
    let feather = (r * 0.14).max(1.5);
    for (x, y, p) in px.enumerate_pixels_mut() {
        let d = ((x as f32 + 0.5 - r).powi(2) + (y as f32 + 0.5 - r).powi(2)).sqrt();
        let t = ((r - d) / feather).clamp(0., 1.);
        let a = t * t * (3. - 2. * t);
        p[3] = (p[3] as f32 * a).round() as u8;
    }
    let mut out = Vec::new();
    let enc = image::codecs::png::PngEncoder::new_with_quality(&mut out, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::Sub);
    image::DynamicImage::ImageRgba8(px).write_with_encoder(enc).ok()?;
    Some(out)
}

fn shrink(bytes: &[u8], w: u32, k: char) -> Option<Vec<u8>> {
    if k == 'a' {
        return face_png(bytes, w);
    }
    let trim = k == 't';
    let mut im = image::load_from_memory(bytes).ok()?;
    let mut changed = false;
    if trim {
        let rgba = im.to_rgba8();
        let (iw, ih) = rgba.dimensions();
        let solid = |x: u32, y: u32| rgba.get_pixel(x, y)[3] > 12;
        let left = (0..iw).find(|&x| (0..ih).any(|y| solid(x, y))).unwrap_or(0);
        let top = (0..ih).find(|&y| (0..iw).any(|x| solid(x, y))).unwrap_or(0);
        let bottom = (0..ih).rev().find(|&y| (0..iw).any(|x| solid(x, y))).map_or(ih, |y| y + 1).max(top + 1);
        if left > 0 || top > 0 || bottom < ih {
            im = im.crop_imm(left, top, iw - left, bottom - top);
            changed = true;
        }
    }
    if im.width() > w + w / 8 {
        let h = ((im.height() as u64 * w as u64) / im.width() as u64).max(1) as u32;
        im = im.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
        changed = true;
    }
    if !changed {
        return None;
    }
    let mut out = Vec::new();
    let enc = image::codecs::png::PngEncoder::new_with_quality(&mut out, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::Sub);
    im.write_with_encoder(enc).ok()?;
    Some(out)
}

/// "https://images.metahub.space/logo/medium/tt123/img" → "tt123"
fn metahub_logo_imdb(url: &str) -> Option<String> {
    let rest = url.split("images.metahub.space/logo/").nth(1)?;
    rest.split('/').find(|p| p.starts_with("tt")).map(String::from)
}
