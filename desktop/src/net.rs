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
            let got = couch::rt::spawn(async move { couch::images::load(&core, &url).await.map_err(|e| e.to_string()) }).await?.map_err(anyhow::Error::msg)?;
            let (mut bytes, mut ct) = got;
            if let Some(w) = fit {
                // GPUI samples a picture with no mip levels, so a big picture drawn
                // small shimmers on its edges: shrink it here, properly, once
                if let Ok(Some(png)) = couch::rt::spawn_blocking({
                    let b = bytes.clone();
                    move || shrink(&b, w)
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

const FIT: &str = "couchfit=";

/// A picture URL that comes back no wider than `w` logical pixels (on this
/// screen's scale), scaled down with a good filter.
pub fn fit(url: &str, w: f32) -> String {
    let px = (w * crate::ui::scale()).ceil() as u32;
    if url.starts_with("data:") || url.ends_with(".svg") || px == 0 {
        return url.to_string();
    }
    format!("{url}{}{FIT}{px}", if url.contains('?') { '&' } else { '?' })
}

fn split_fit(url: &str) -> (String, Option<u32>) {
    let Some(i) = url.rfind(FIT) else { return (url.to_string(), None) };
    let w = url[i + FIT.len()..].parse().ok();
    (url[..i - 1].to_string(), w)
}

fn shrink(bytes: &[u8], w: u32) -> Option<Vec<u8>> {
    let im = image::load_from_memory(bytes).ok()?;
    if im.width() <= w + w / 8 {
        return None;
    }
    let h = ((im.height() as u64 * w as u64) / im.width() as u64).max(1) as u32;
    let small = im.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
    let mut out = Vec::new();
    let enc = image::codecs::png::PngEncoder::new_with_quality(&mut out, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::Sub);
    small.write_with_encoder(enc).ok()?;
    Some(out)
}
