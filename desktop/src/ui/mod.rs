pub mod ambient;
pub mod zoom;
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

static SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(100);

/// The window's scale factor (set every frame).
pub fn scale() -> f32 {
    SCALE.load(std::sync::atomic::Ordering::Relaxed) as f32 / 100.
}
static VIEW_W: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1600);
/// The window's width in logical pixels (set every frame, in steps of 320 so a
/// resize doesn't refetch every picture).
pub fn view_w() -> f32 {
    VIEW_W.load(std::sync::atomic::Ordering::Relaxed) as f32
}
pub fn set_scale(s: f32, w: f32) {
    SCALE.store((s * 100.).round() as u32, std::sync::atomic::Ordering::Relaxed);
    VIEW_W.store(((w / 320.).ceil() * 320.) as u32, std::sync::atomic::Ordering::Relaxed);
}

/// A studio logo, readable on a dark picture: its black and dark-grey parts
/// turn white; coloured parts (Marvel's red box) keep their colour.
pub fn readable_logo(bytes: &[u8]) -> Option<std::sync::Arc<gpui::Image>> {
    let mut im = image::load_from_memory(bytes).ok()?.to_rgba8();
    let dark = |p: &image::Rgba<u8>| {
        let (r, g, b) = (p[0] as f32 / 255., p[1] as f32 / 255., p[2] as f32 / 255.);
        let lum = 0.299 * r + 0.587 * g + 0.114 * b;
        let sat = r.max(g).max(b) - r.min(g).min(b);
        lum < 0.4 && sat < 0.22
    };
    let (mut n, mut d, mut opaque) = (0u64, 0u64, 0u64);
    let total = (im.width() as u64 * im.height() as u64).max(1);
    for p in im.pixels() {
        if p[3] > 128 {
            n += 1;
            if dark(p) {
                d += 1;
            }
        }
        if p[3] > 250 {
            opaque += 1;
        }
    }
    // a logo on its own box (no transparency): leave it be
    let boxed = opaque as f64 / total as f64 > 0.9;
    if n > 0 && !boxed && d as f64 / n as f64 > 0.12 {
        for p in im.pixels_mut() {
            if p[3] > 0 && dark(p) {
                p[0] = 255;
                p[1] = 255;
                p[2] = 255;
            }
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(im).write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(std::sync::Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, out.into_inner())))
}

/// "S3 · E7 · The Strings That Bind" → ("S3:E7", "The Strings That Bind")
pub fn split_episode(s: &str) -> (Option<String>, Option<String>) {
    let mut it = s.splitn(3, " · ");
    let (a, b, c) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next());
    if a.starts_with('S') && b.starts_with('E') {
        (Some(format!("{a}:{b}")), c.map(|t| t.to_string()).filter(|t| !t.trim().is_empty()))
    } else {
        (None, Some(s.to_string()).filter(|t| !t.trim().is_empty()))
    }
}

/// a soft shadow, so faces and names read on any picture
pub fn lift() -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow { color: theme::black(0.55), offset: gpui::point(px(0.), px(2.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }]
}

/// "S3:E7", heavy, for the corner of a picture
pub fn ep_badge(t: String) -> impl IntoElement {
    gpui::div()
        .px(px(9.))
        .py(px(3.))
        .rounded(px(8.))
        .bg(theme::black(0.55))
        .shadow(lift())
        .text_size(px(14.))
        .font_weight(gpui::FontWeight::BLACK)
        .text_color(theme::text())
        .child(t)
}

/// faces at the top of the screen: their names show under them
pub fn faces_below(key: &str, people: Vec<(Option<String>, String)>, size: f32) -> impl IntoElement {
    faces_at(key, people, size, false)
}

/// The faces of who's watching, overlapping in a row; pointing at one shows the name.
pub fn faces(key: &str, people: Vec<(Option<String>, String)>, size: f32) -> impl IntoElement {
    faces_at(key, people, size, true)
}

fn faces_at(key: &str, people: Vec<(Option<String>, String)>, size: f32, above: bool) -> impl IntoElement {
    let key = key.to_string();
    gpui::div().flex().flex_row_reverse().children(people.into_iter().enumerate().map(move |(i, (pic, name))| {
        let group = gpui::SharedString::from(format!("face-{key}-{i}"));
        let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();
        gpui::div()
            .id(gpui::SharedString::from(format!("facei-{key}-{i}")))
            .group(group.clone())
            .relative()
            .when(i > 0, |d| d.mr(px(-size * 0.28)))
            .size(px(size))
            .flex_none()
            .rounded_full()
            .border_2()
            .border_color(theme::black(0.55))
            .shadow(lift())
            .bg(theme::neon())
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(size * 0.42))
            .font_weight(gpui::FontWeight::BLACK)
            .child(match pic {
                Some(u) => gpui::img(crate::net::face(&u, size)).size_full().rounded_full().into_any_element(),
                None => gpui::div().child(initial).into_any_element(),
            })
            // the name, above it, while pointed at
            .child(
                gpui::div()
                    .absolute()
                    .when(above, |d| d.bottom(px(size + 6.)))
                    .when(!above, |d| d.top(px(size + 6.)))
                    .right(px(-8.))
                    .px(px(10.))
                    .py(px(4.))
                    .rounded_full()
                    .bg(gpui::rgba(0x151114f0))
                    .border_1()
                    .border_color(theme::white(0.12))
                    .shadow(lift())
                    .whitespace_nowrap()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .child(name),
            )
    }))
}
