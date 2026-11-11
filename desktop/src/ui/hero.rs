//! The billboard: one title per visit, full-bleed backdrop, the title's
//! logo, a line of facts, the blurb, and Play / More info / +.
use crate::api::{sized, Meta};
use crate::theme;
use crate::ui::{icon, icon_fill};
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, App, ClickEvent, FontWeight, ObjectFit, Pixels, Window};

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

pub fn hero(m: &Meta, height: Pixels, on_play: Click, on_info: Click, on_add: Click) -> AnyElement {
    let art = m.background.clone().or(m.poster.clone()).map(|u| sized(&u, "original"));
    let logo = m.logo.clone().map(|u| sized(&u, "large"));
    let mut facts: Vec<String> = vec![if m.kind == "series" { "Series".into() } else { "Movie".into() }];
    if let Some(g) = m.genres.as_ref().and_then(|g| g.first()) {
        facts.push(g.clone());
    }
    if let Some(y) = m.year() {
        facts.push(y);
    }
    let name = m.name.clone();
    div()
        .relative()
        .w_full()
        .h(height)
        .overflow_hidden()
        .bg(theme::bg())
        .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
        // shade: left for the words, bottom into the rows
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(linear_gradient(90., linear_color_stop(theme::black(0.78), 0.), linear_color_stop(theme::black(0.0), 0.62))),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .bottom_0()
                .w_full()
                .h(height * 0.55)
                .bg(linear_gradient(180., linear_color_stop(theme::black(0.0), 0.), linear_color_stop(theme::black(1.0), 1.))),
        )
        .child(
            div()
                .absolute()
                .left(px(theme::GUTTER))
                .bottom(px(170.))
                .w(px(620.))
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(match logo {
                    Some(u) => img(u)
                        .max_w(px(460.))
                        .max_h(px(150.))
                        .object_fit(ObjectFit::Contain)
                        .with_fallback(move || big_title(&name).into_any_element())
                        .into_any_element(),
                    None => big_title(&m.name).into_any_element(),
                })
                .child(div().flex().gap(px(10.)).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(theme::text()).children(
                    facts.into_iter().enumerate().flat_map(|(i, f)| {
                        let dot = (i > 0).then(|| div().text_color(theme::label3()).child("·"));
                        dot.into_iter().map(|d| d.into_any_element()).chain(std::iter::once(div().child(f).into_any_element()))
                    }),
                ))
                .children(m.description.clone().map(|d| {
                    div().max_w(px(560.)).text_size(px(17.)).line_height(px(25.)).line_clamp(3).text_ellipsis().text_color(theme::white(0.88)).child(d)
                }))
                .child(
                    div()
                        .flex()
                        .gap(px(10.))
                        .mt(px(6.))
                        .child(
                            div()
                                .id("hero-play")
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .h(px(52.))
                                .px(px(28.))
                                .rounded(px(14.))
                                .bg(theme::text())
                                .text_color(gpui::rgb(0x000000))
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(18.))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.88))
                                .on_click(on_play)
                                .child(icon_fill("play", 20.).text_color(gpui::rgb(0x000000)))
                                .child("Play"),
                        )
                        .child(
                            div()
                                .id("hero-info")
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .h(px(52.))
                                .px(px(22.))
                                .rounded(px(14.))
                                .bg(theme::white(0.16))
                                .border_1()
                                .border_color(theme::white(0.14))
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(17.))
                                .text_color(theme::text())
                                .cursor_pointer()
                                .hover(|s| s.bg(theme::white(0.24)))
                                .on_click(on_info)
                                .child(icon("info", 20.))
                                .child("More info"),
                        )
                        .child(
                            div()
                                .id("hero-add")
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(52.))
                                .rounded_full()
                                .bg(theme::white(0.16))
                                .border_1()
                                .border_color(theme::white(0.14))
                                .cursor_pointer()
                                .hover(|s| s.bg(theme::white(0.24)))
                                .on_click(on_add)
                                .child(icon("plus", 22.)),
                        ),
                ),
        )
        .into_any_element()
}

fn big_title(name: &str) -> impl IntoElement {
    div().text_size(px(64.)).line_height(px(66.)).font_weight(FontWeight::BLACK).text_color(theme::text()).child(name.to_string())
}
