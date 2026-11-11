pub mod home;
pub mod player;
pub mod detail;

use crate::theme;
use gpui::{div, prelude::*, px, AnyElement, FontWeight};

/// A screen that hasn't been rebuilt in GPUI yet.
pub fn soon(name: &str) -> AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .child(div().text_size(px(34.)).font_weight(FontWeight::BLACK).child(name.to_string()))
        .child(div().text_size(px(16.)).text_color(theme::label2()).child("being rebuilt for the new app"))
        .into_any_element()
}
