pub mod card;
pub mod badges;
pub mod emoji;
pub mod smooth;
pub mod lazy;
pub mod rows;
pub mod hero;
pub mod input;
pub mod video;

use crate::theme;
use gpui::{prelude::*, px, svg, Svg};

/// One of couch's icons (Material Symbols Rounded + couch's own marks).
pub fn icon(name: &str, size: f32) -> Svg {
    svg().path(format!("icons/{name}.svg")).size(px(size)).flex_none().text_color(theme::text())
}
pub fn icon_fill(name: &str, size: f32) -> Svg {
    let p = format!("icons/{name}-fill.svg");
    let p = if crate::assets::bytes(&p).is_some() { p } else { format!("icons/{name}.svg") };
    svg().path(p).size(px(size)).flex_none().text_color(theme::text())
}

/// Follow an actor, director or studio: their new releases land on Home.
pub fn follow_btn(kind: &'static str, id: u64, name: String, img: Option<String>) -> gpui::Stateful<gpui::Div> {
    let on = couch::state::get()
        .core
        .db
        .get::<Vec<serde_json::Value>>("follows")
        .unwrap_or_default()
        .iter()
        .any(|f| f["kind"] == kind && f["id"].as_u64() == Some(id));
    gpui::div()
        .id(gpui::SharedString::from(format!("follow-{kind}-{id}")))
        .flex_none()
        .px(px(18.))
        .h(px(40.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded_full()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .cursor_pointer()
        .when(on, |d| d.bg(theme::white(0.12)))
        .when(!on, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
        .hover(|s| s.opacity(0.85))
        .child(if on { "following" } else { "follow" })
        .on_click(move |_, window, cx| {
            let (name, img) = (name.clone(), img.clone());
            let _ = window;
            cx.spawn(async move |cx| {
                let _ = crate::api::call::<serde_json::Value>("follow_toggle", serde_json::json!({ "kind": kind, "id": id, "name": name, "img": img })).await;
                let _ = cx.update(|cx| {
                    crate::Nav::toast(if on { format!("unfollowed {name}") } else { format!("following {name} — their new releases show up on Home") }, cx);
                    cx.refresh_windows();
                });
            })
            .detach();
        })
}

/// A studio logo, readable on a dark picture: dark logos become white.
pub fn readable_logo(bytes: &[u8]) -> Option<std::sync::Arc<gpui::Image>> {
    let mut im = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (mut lum, mut n, mut opaque) = (0f64, 0u64, 0u64);
    let total = (im.width() as u64 * im.height() as u64).max(1);
    for p in im.pixels() {
        if p[3] > 128 {
            lum += (0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64) / 255.;
            n += 1;
        }
        if p[3] > 250 {
            opaque += 1;
        }
    }
    // a logo on its own box (no transparency): leave it be
    let boxed = opaque as f64 / total as f64 > 0.9;
    if n > 0 && !boxed && lum / (n as f64) < 0.5 {
        for p in im.pixels_mut() {
            p[0] = 255;
            p[1] = 255;
            p[2] = 255;
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(im).write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(std::sync::Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, out.into_inner())))
}
