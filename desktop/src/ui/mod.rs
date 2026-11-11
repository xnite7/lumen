pub mod card;
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
