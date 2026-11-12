//! couch's look, from the webview's base.css: OLED black, warm white text,
//! neon pink and lamp amber.
use gpui::{rgb, rgba, Hsla, Rgba};

pub const FONT: &str = "Satoshi";

/// How text is drawn: "sharp" (ClearType, colour fringes on some screens),
/// "smooth" (grayscale) or "auto" (what Windows is set to).
pub fn text_mode() -> String {
    couch::state::try_get().and_then(|s| s.core.db.get::<String>("gpui_text_mode")).unwrap_or_else(|| "auto".into())
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
