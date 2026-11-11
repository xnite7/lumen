//! A landscape title card for the rows: the backdrop, a soft shade at the
//! bottom, and the title's logo (or its name when there's no logo).
use crate::api::{sized, Meta};
use crate::theme;
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, ObjectFit, SharedString};

pub const W: f32 = 300.0;
pub const H: f32 = 169.0;

pub fn card(id: SharedString, m: &Meta, on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static) -> AnyElement {
    card_with(id, m, None, None, None, on_click)
}

/// A card that previews on hover: `clip` is this card's frame + how far it
/// has faded in + how long it's been playing (seconds).
pub fn card_preview(
    id: SharedString,
    m: &Meta,
    clip: Option<(std::sync::Arc<gpui::RenderImage>, f32, f32)>,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    card_with(id, m, None, None, Some(clip), on_click)
}

/// A card with a progress bar and a line under it (Continue watching).
pub fn card_with(
    id: SharedString,
    m: &Meta,
    progress: Option<f32>,
    sub: Option<String>,
    preview: Option<Option<(std::sync::Arc<gpui::RenderImage>, f32, f32)>>,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let hover_meta = m.clone();
    let clip = preview.clone().flatten();
    // the logo steps back once the clip has played 2 s, and comes back on hover of the picture
    let logo_a = clip.as_ref().map_or(1., |c| 1. - ((c.2 - 2.) / 0.6).clamp(0., 1.));
    let art = m.background.clone().or(m.poster.clone()).map(|u| sized(&u, "w780"));
    let logo = m.logo.clone().map(|u| sized(&u, "w500"));
    let el = div()
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
        .when(preview.is_some(), |d| d.on_hover(move |on, _, cx| crate::ui::video::hover(&hover_meta, *on, cx)))
        .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
        .children(clip.map(|(f, a, _)| img(f).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).opacity(a)))
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
                .opacity(logo_a)
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
        .when_some(progress, |d, p| {
            d.child(div().absolute().left(px(14.)).right(px(14.)).bottom(px(6.)).h(px(4.)).rounded_full().bg(theme::white(0.25)).child(div().h_full().rounded_full().w(px((W - 28.) * p.clamp(0., 1.))).bg(theme::lamp())))
        })
        .into_any_element();
    match sub {
        Some(t) => div().flex_none().flex().flex_col().gap(px(6.)).child(el).child(div().text_size(px(13.)).text_color(theme::label2()).truncate().w(px(W)).child(t)).into_any_element(),
        None => el,
    }
}

fn title_text(name: &str) -> impl IntoElement {
    div().text_size(px(19.)).font_weight(gpui::FontWeight::BLACK).line_height(px(21.)).line_clamp(2).text_color(theme::text()).child(name.to_string())
}

/// A portrait poster (search results, lists).
pub fn poster(id: SharedString, m: &Meta, w: f32, on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static) -> AnyElement {
    let art = m.poster.clone().map(|u| sized(&u, "w342"));
    let name = m.name.clone();
    div()
        .id(id)
        .flex_none()
        .w(px(w))
        .h(px(w * 1.5))
        .rounded(px(10.))
        .overflow_hidden()
        .relative()
        .bg(theme::surface2())
        .cursor_pointer()
        .border_1()
        .border_color(theme::white(0.0))
        .hover(|s| s.border_color(theme::white(0.55)))
        .on_click(on_click)
        .child(div().absolute().top_0().left_0().size_full().p(px(10.)).flex().items_end().child(title_text(&name)))
        .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
        .into_any_element()
}
