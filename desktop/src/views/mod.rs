pub mod home;
pub mod wrapped;
pub mod studio;
pub mod member;
pub mod planner;
pub mod invite;
pub mod person;
pub mod signin;
pub mod settings;
pub mod couch;
pub mod calendar;
pub mod library;
pub mod search;
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
