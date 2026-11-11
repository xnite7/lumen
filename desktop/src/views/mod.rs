pub mod home;

use crate::api::{sized, Meta};
use crate::theme;
use gpui::{div, img, prelude::*, px, AnyElement, FontWeight, ObjectFit};

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

/// Title page placeholder until Detail is ported.
pub fn detail_stub(m: &Meta) -> AnyElement {
    let art = m.background.clone().or(m.poster.clone()).map(|u| sized(&u, "original"));
    div()
        .size_full()
        .relative()
        .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).opacity(0.5)))
        .child(
            div()
                .absolute()
                .left(px(theme::GUTTER))
                .bottom(px(80.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(div().text_size(px(56.)).font_weight(FontWeight::BLACK).child(m.name.clone()))
                .children(m.description.clone().map(|d| div().max_w(px(640.)).text_size(px(17.)).line_height(px(25.)).text_color(theme::white(0.85)).child(d)))
                .child(
                    div()
                        .id("detail-back")
                        .mt(px(10.))
                        .px(px(20.))
                        .py(px(12.))
                        .rounded(px(14.))
                        .bg(theme::white(0.16))
                        .cursor_pointer()
                        .w(px(110.))
                        .on_click(|_, _, cx| crate::Nav::back(cx))
                        .child("← back"),
                ),
        )
        .into_any_element()
}
