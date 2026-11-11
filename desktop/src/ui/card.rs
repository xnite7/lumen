//! A landscape title card for the rows: the backdrop, a soft shade at the
//! bottom, and the title's logo (or its name when there's no logo).
use crate::api::{sized, Meta};
use crate::theme;
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, ObjectFit, SharedString};

pub const W: f32 = 300.0;
pub const H: f32 = 169.0;

pub fn card(id: SharedString, m: &Meta, on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static) -> AnyElement {
    let art = m.background.clone().or(m.poster.clone()).map(|u| sized(&u, "w780"));
    let logo = m.logo.clone().map(|u| sized(&u, "w500"));
    div()
        .id(id)
        .flex_none()
        .w(px(W))
        .h(px(H))
        .rounded(px(12.))
        .overflow_hidden()
        .relative()
        .bg(theme::surface2())
        .cursor_pointer()
        .border_1()
        .border_color(theme::white(0.0))
        .hover(|s| s.border_color(theme::white(0.55)))
        .on_click(on_click)
        .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
        .child(
            div()
                .absolute()
                .left_0()
                .bottom_0()
                .w_full()
                .h(px(H * 0.6))
                .bg(linear_gradient(180., linear_color_stop(theme::black(0.0), 0.), linear_color_stop(theme::black(0.85), 1.))),
        )
        .child(
            div()
                .absolute()
                .left(px(14.))
                .right(px(14.))
                .bottom(px(12.))
                .h(px(54.))
                .flex()
                .items_end()
                .child(match logo {
                    Some(u) => img(u)
                        .h(px(54.))
                        .max_w(px(170.))
                        .object_fit(ObjectFit::Contain)
                        .with_fallback({
                            let name = m.name.clone();
                            move || title_text(&name).into_any_element()
                        })
                        .into_any_element(),
                    None => title_text(&m.name).into_any_element(),
                }),
        )
        .into_any_element()
}

fn title_text(name: &str) -> impl IntoElement {
    div().text_size(px(19.)).font_weight(gpui::FontWeight::BLACK).line_height(px(21.)).line_clamp(2).text_color(theme::text()).child(name.to_string())
}
