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
    clip: Option<(crate::ui::video::Pic, f32, f32)>,
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
    preview: Option<Option<(crate::ui::video::Pic, f32, f32)>>,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let hover_meta = m.clone();
    let menu_meta = m.clone();
    let clip = preview.clone().flatten();
    // the logo steps back once the clip has played 2 s, and comes back on hover of the picture
    let logo_a = clip.as_ref().map_or(1., |c| 1. - ((c.2 - 2.) / 0.6).clamp(0., 1.));
    let art = m.background.clone().or(m.poster.clone()).map(|u| crate::net::fit(&sized(&u, "w780"), W));
    // no logo given: metahub's (and TMDB's behind it) by the IMDb id
    let logo = m.logo.clone().or_else(|| m.id.starts_with("tt").then(|| format!("https://images.metahub.space/logo/medium/{}/img", m.id.split(':').next().unwrap_or(&m.id)))).map(|u| crate::net::logo(&sized(&u, "w500"), 170.));
    let el = div()
        .id(id)
        .flex_none()
        .w(px(W))
        .h(px(H))
        .rounded(px(12.))
        .overflow_hidden()
        .relative()
        // a GPU clip shows through this card: nothing of the card under it
        .when(!matches!(clip, Some((crate::ui::video::Pic::Native, _, _))), |d| d.bg(theme::surface2()))
        .cursor_pointer()
        .on_click(move |e, w, cx| {
            // the end of a sideways drag isn't a click
            if !crate::ui::rows::dragged() {
                on_click(e, w, cx)
            }
        })
        .on_mouse_down(gpui::MouseButton::Right, move |e, _, cx| {
            cx.stop_propagation();
            crate::Nav::menu(&menu_meta, e.position, cx)
        })
        // hovering a card starts fetching its page, so opening it is instant
        .on_hover({
            let previews = preview.is_some();
            move |on, _, cx| {
                if *on {
                    crate::views::detail::prefetch(&hover_meta, cx);
                }
                if previews {
                    crate::ui::video::hover(&hover_meta, *on, cx);
                }
            }
        })
        .children(match &clip {
            Some((p, a, _)) => crate::ui::video::layers(art.clone(), p, *a, 12.),
            None => art.clone().map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).into_any_element()).into_iter().collect(),
        })
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
        // sound on/off while it previews
        // sound on/off: bottom left, showing only when the pointer comes near it
        .children(clip.is_some().then(|| {
            div()
                .absolute()
                .left_0()
                .bottom_0()
                .w(px(120.))
                .h(px(96.))
                .group("snd-zone")
                .flex()
                .items_end()
                .p(px(8.))
                .child(div().opacity(0.).group_hover("snd-zone", |s| s.opacity(1.)).child(crate::ui::video::sound_button("card:".into(), 34.)))
        }))
        .into_any_element();
    // under the picture: the progress bar, then "S1 E2" on the left and the
    // time left on the right (a tab in `sub` splits the two)
    // a square-ended white bar, 60% of the card's width
    let bw = W * 0.6;
    let bar = progress.map(|p| div().w(px(W)).flex().justify_center().child(div().w(px(bw)).h(px(4.)).bg(theme::white(0.18)).child(div().h_full().w(px(bw * p.clamp(0., 1.))).bg(gpui::white()))));
    let line = sub.map(|t| {
        let (l, r) = t.split_once('\t').map_or((t.clone(), String::new()), |(a, b)| (a.to_string(), b.to_string()));
        div()
            .w(px(W))
            .flex()
            .justify_between()
            .gap(px(10.))
            .text_size(px(13.))
            .child(div().min_w_0().truncate().text_color(theme::label2()).child(l))
            .child(div().flex_none().text_color(theme::label3()).child(r))
    });
    if bar.is_none() && line.is_none() {
        return el;
    }
    div().flex_none().flex().flex_col().gap(px(8.)).child(el).children(bar).children(line).into_any_element()
}

fn title_text(name: &str) -> impl IntoElement {
    // fits where the logo goes (170 wide, 54 high): two lines at most, then …
    div().w(px(170.)).text_size(px(19.)).font_weight(gpui::FontWeight::BLACK).line_height(px(22.)).line_clamp(2).text_ellipsis().text_color(theme::text()).child(name.to_string())
}

/// A portrait poster (search results, lists).
pub fn poster(id: SharedString, m: &Meta, w: f32, on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static) -> AnyElement {
    let art = m.poster.clone().map(|u| crate::net::fit(&sized(&u, "w342"), w));
    let name = m.name.clone();
    let menu_meta = m.clone();
    let pre = m.clone();
    div()
        .id(id)
        .on_hover(move |on, _, cx| {
            if *on {
                crate::views::detail::prefetch(&pre, cx);
            }
        })
        .on_mouse_down(gpui::MouseButton::Right, move |e, _, cx| {
            cx.stop_propagation();
            crate::Nav::menu(&menu_meta, e.position, cx)
        })
        .flex_none()
        .w(px(w))
        .h(px(w * 1.5))
        .rounded(px(10.))
        .overflow_hidden()
        .relative()
        .bg(theme::surface2())
        .cursor_pointer()
        .on_click(move |e, w, cx| {
            // the end of a sideways drag isn't a click
            if !crate::ui::rows::dragged() {
                on_click(e, w, cx)
            }
        })
        .child(div().absolute().top_0().left_0().size_full().p(px(10.)).flex().items_end().child(title_text(&name)))
        .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
        .into_any_element()
}
