#![allow(dead_code)] // a design system: every token is there to be reached for
//! couch's look: the living room at 2 AM. See DESIGN.md at the repo root.
//!
//! 90% dark room, 8% lamp, 2% neon. The room is OLED black with plum-black
//! surfaces lifted barely off it; text is warm white, never pure white; the
//! lamp (amber) is warmth and "now" (progress, ratings, what's playing);
//! the neon (pink) is precious: the logo, focus, the one thing on screen.
//!
//! This file is the only place colours, type sizes, radii and spacing are
//! defined. Motion lives in `motion.rs`.
use gpui::{rgb, rgba, Hsla, Rgba};

pub const FONT: &str = "Satoshi";

/// How text is drawn: "sharp" (ClearType, colour fringes on some screens),
/// "smooth" (grayscale, soft edges; the default) or "auto" (what Windows is set to).
pub fn text_mode() -> String {
    couch::state::try_get().and_then(|s| s.core.db.get::<String>("gpui_text_mode")).unwrap_or_else(|| "smooth".into())
}
pub fn apply_text_mode(cx: &mut gpui::App) {
    cx.set_text_rendering_mode(match text_mode().as_str() {
        "sharp" => gpui::TextRenderingMode::Subpixel,
        "smooth" => gpui::TextRenderingMode::Grayscale,
        _ => gpui::TextRenderingMode::PlatformDefault,
    });
}

pub fn bg() -> Rgba {
    rgb(0x000000)
}
pub fn surface() -> Rgba {
    rgb(0x0a0809)
}
pub fn surface2() -> Rgba {
    rgb(0x141113)
}
pub fn text() -> Rgba {
    rgb(0xf4eaf0)
}
pub fn text_dim() -> Rgba {
    rgb(0x9c8a96)
}
/// label-2 / label-3: warm white at 60% / 34%
pub fn label2() -> Rgba {
    rgba(0xf4eaf099)
}
pub fn label3() -> Rgba {
    rgba(0xf4eaf057)
}
pub fn neon() -> Rgba {
    rgb(0xe0309a)
}
pub fn neon_soft() -> Rgba {
    rgb(0xff6ec4)
}
pub fn lamp() -> Rgba {
    rgb(0xffb25c)
}
/// floating things (menus, popovers, toasts): a touch above surface2
pub fn surface3() -> Rgba {
    rgb(0x1d181b)
}
/// the fallback glow when there's no artwork: deep plum
pub fn plum() -> Rgba {
    rgb(0x7a3b5e)
}
/// label-4: warm white at 16% (dividers you feel more than see)
pub fn label4() -> Rgba {
    rgba(0xf4eaf029)
}
pub fn danger() -> Rgba {
    rgb(0xff5a5a)
}
pub fn good() -> Rgba {
    rgb(0x7fd6a0)
}

/// the lamp at some strength
pub fn lamp_a(a: f32) -> Hsla {
    with_a(lamp(), a)
}
/// the neon at some strength
pub fn neon_a(a: f32) -> Hsla {
    with_a(neon(), a)
}
pub fn with_a(c: Rgba, a: f32) -> Hsla {
    let mut h: Hsla = c.into();
    h.a = a.clamp(0., 1.);
    h
}

/// A neon tube's light round something (k 0..1: off..lit).
pub fn glow_neon(k: f32) -> Vec<gpui::BoxShadow> {
    let c = |a: f32, blur: f32, spread: f32| gpui::BoxShadow { color: neon_a(a * k), offset: gpui::point(gpui::px(0.), gpui::px(0.)), blur_radius: gpui::px(blur), spread_radius: gpui::px(spread), inset: false };
    vec![c(0.55, 18., -2.), c(0.4, 46., -6.)]
}
/// The lamp's warm light round something (k 0..1).
pub fn glow_lamp(k: f32) -> Vec<gpui::BoxShadow> {
    let c = |a: f32, blur: f32, spread: f32| gpui::BoxShadow { color: lamp_a(a * k), offset: gpui::point(gpui::px(0.), gpui::px(0.)), blur_radius: gpui::px(blur), spread_radius: gpui::px(spread), inset: false };
    vec![c(0.55, 22., -4.), c(0.35, 60., -10.)]
}

/// Type: one family (Satoshi), heavy at the top and quiet underneath.
/// Headings speak midnite lowercase; titles of films keep their own case.
pub mod ty {
    /// the year, a count: as big as the window allows
    pub const DISPLAY: f32 = 132.;
    /// a reveal ("a midnite creature")
    pub const HERO: f32 = 72.;
    /// a page title
    pub const TITLE1: f32 = 34.;
    /// a section, a row
    pub const TITLE2: f32 = 24.;
    pub const TITLE3: f32 = 20.;
    pub const HEAD: f32 = 17.;
    pub const BODY: f32 = 15.;
    pub const FOOT: f32 = 13.;
    pub const CAPTION: f32 = 12.;
}

/// Corners: soft, never sharp, never blobby.
pub mod radius {
    pub const XS: f32 = 8.;
    pub const SM: f32 = 12.;
    pub const MD: f32 = 16.;
    pub const LG: f32 = 22.;
    pub const XL: f32 = 30.;
    pub const POSTER: f32 = 12.;
    pub const CARD: f32 = 14.;
}

/// Spacing on a 4 px grid.
pub mod space {
    pub const S1: f32 = 4.;
    pub const S2: f32 = 8.;
    pub const S3: f32 = 12.;
    pub const S4: f32 = 16.;
    pub const S5: f32 = 20.;
    pub const S6: f32 = 24.;
    pub const S7: f32 = 32.;
    pub const S8: f32 = 44.;
    /// between shelves on a page
    pub const SHELF: f32 = 44.;
}

pub fn fill(a: f32) -> Hsla {
    let mut c: Hsla = rgb(0x786e76).into();
    c.a = a;
    c
}
pub fn white(a: f32) -> Hsla {
    let mut c: Hsla = text().into();
    c.a = a;
    c
}
pub fn black(a: f32) -> Hsla {
    let mut c: Hsla = rgb(0x000000).into();
    c.a = a;
    c
}

/// page side padding (the webview's --gutter on a wide window)
pub const GUTTER: f32 = 56.0;
