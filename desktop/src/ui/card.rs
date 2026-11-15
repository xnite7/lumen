//! A landscape title card for the rows: the backdrop, a soft shade at the
//! bottom, and the title's logo (or its name when there's no logo).
use crate::api::{sized, Meta};
use crate::theme;
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, ObjectFit, SharedString};

pub const W: f32 = 300.0;
pub const H: f32 = 169.0;

thread_local! {
    /// each card's slot: its left edge on screen, last frame
    static SLOTS: std::cell::RefCell<std::collections::HashMap<String, f32>> = std::cell::RefCell::new(std::collections::HashMap::new());
    /// each card: is the pointer on it, since when (it grows while you rest on it)
    static GROW: std::cell::RefCell<std::collections::HashMap<String, (bool, std::time::Instant, f32)>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

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
    let zkey = id.to_string();
    let lkey = zkey.clone();
    // resting on it: the card grows a little, over its neighbours
    let hkey = zkey.clone();
    // only after you've rested on it (the same wait as the preview); each card waits on its own
    let (hov, since, from) = GROW.with(|g| g.borrow().get(&zkey).copied()).unwrap_or((false, std::time::Instant::now() - std::time::Duration::from_secs(5), 0.));
    let dwell = crate::ui::video::dwell_secs();
    let (grow, moving) = if hov {
        let x = ((since.elapsed().as_secs_f32() - dwell) / 0.32).clamp(0., 1.);
        (crate::motion::out3(x), x < 1.)
    } else {
        // shrinking back from however far it had grown
        let x = (since.elapsed().as_secs_f32() / 0.24).min(1.);
        (from * (1. - x * x * (3. - 2. * x)), x < 1. && from > 0.)
    };
    let s = 1. + 0.07 * grow;
    // on whole screen pixels, growing two at a time (one each side): edges that
    // land between pixels got rounded differently every frame and shivered
    let sf = crate::ui::scale().max(1.);
    let even = |v: f32, base: f32| base + (((v - base) * sf / 2.).round() * 2.) / sf;
    let (gw, gh) = (even(W * s, W), even(H * s, H));
    // grown, it keeps clear of the window's edges (and comes fully into view
    // when it was half off the side)
    let dx = if grow > 0. {
        SLOTS.with(|m| m.borrow().get(&zkey).copied()).map_or(0., |sx: f32| {
            let vw = crate::ui::view_w();
            let margin = 28.;
            let left = sx - (gw - W) / 2.;
            let right = left + gw;
            let want = if left < margin { margin - left } else if right > vw - margin { vw - margin - right } else { 0. };
            ((want * grow) * sf).round() / sf
        })
    } else {
        0.
    };
    let skey = zkey.clone();
    // seen it already: the card steps back a little (not while it previews)
    let watched = clip.is_none() && progress.is_none() && crate::seen::status(&m.id).0;
    let (zid, zart) = (m.id.clone(), art.clone());
    let zk2 = zkey.clone();
    let el = div()
        .id(id)
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| crate::ui::zoom::pressed(&zid, &zk2, zart.clone(), 12.))
        .child(crate::ui::zoom::track(zkey))
        .absolute()
        .left(px(-(gw - W) / 2. + dx))
        .top(px(-(gh - H) / 2.))
        .w(px(gw))
        .h(px(gh))
        .rounded(px(12.))
        .overflow_hidden()
        .when(grow > 0.01, |d| d.shadow(vec![gpui::BoxShadow { color: theme::black(0.55 * grow), offset: gpui::point(px(0.), px(12. * grow)), blur_radius: px(30.), spread_radius: px(0.), inset: false }]))
        // keeps the frames coming while it grows or shrinks
        .when(moving || (hov && grow < 1.), |d| d.child(gpui::canvas(|_, _, _| (), |_, _, w, _| w.request_animation_frame()).absolute().size_0()))
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
                GROW.with(|g| {
                    let mut g = g.borrow_mut();
                    // leaving: remember how far it had grown, to shrink from there
                    let from = g.get(&hkey).map_or(0., |&(h, t, _)| if h { ((t.elapsed().as_secs_f32() - crate::ui::video::dwell_secs()) / 0.32).clamp(0., 1.) } else { 0. });
                    g.insert(hkey.clone(), (*on, std::time::Instant::now(), if *on { 0. } else { crate::motion::out3(from) }));
                });
                cx.refresh_windows();
                if *on {
                    crate::views::detail::prefetch(&hover_meta, cx);
                    crate::ui::ambient::hover(&hover_meta, cx);
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
        .when(watched, |d| {
            d.child(div().absolute().top_0().left_0().size_full().bg(theme::black(0.5))).child(
                div()
                    .absolute()
                    .top(px(10.))
                    .right(px(10.))
                    .size(px(24.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme::black(0.55))
                    .child(crate::ui::icon("check", 14.).text_color(theme::white(0.85))),
            )
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
                    // (remembered, so it can fly into the page's logo)
                    Some(u) => div()
                        .relative()
                        .child(crate::ui::zoom::track_logo(lkey.clone(), u.clone()))
                        .child(
                            img(u)
                                .h(px(54.))
                                .max_w(px(170.))
                                .object_fit(ObjectFit::Contain)
                                .with_fallback({
                                    let name = m.name.clone();
                                    move || title_text(&name).into_any_element()
                                }),
                        )
                        .into_any_element(),
                    None => title_text(&m.name).into_any_element(),
                }),
        )
        // sound on/off while it previews
        // sound on/off: bottom right, showing only when the pointer comes near it
        .children(clip.is_some().then(|| {
            div()
                .absolute()
                .right_0()
                .bottom_0()
                .w(px(120.))
                .h(px(96.))
                .group("snd-zone")
                .flex()
                .items_end()
                .justify_end()
                .p(px(8.))
                .child(div().opacity(0.).group_hover("snd-zone", |s| s.opacity(1.)).child(crate::ui::video::sound_button("card:".into(), 34.)))
        }))
        .into_any_element();
    // its slot in the row stays the same size; the card itself is drawn over
    // the neighbours (deferred) while it's grown
    let el = div()
        .relative()
        .flex_none()
        .w(px(W))
        .h(px(H))
        // where its slot is on screen (for keeping the grown card in view)
        .child(gpui::canvas(move |b, _, _| SLOTS.with(|m| { m.borrow_mut().insert(skey.clone(), f32::from(b.origin.x)); }), |_, _, _, _| {}).absolute().size_full())
        .child(if grow > 0.001 { gpui::deferred(el).with_priority(1).into_any_element() } else { el }).into_any_element();
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
    let zkey = id.to_string();
    let (zid, zart, zk2) = (m.id.clone(), art.clone(), zkey.clone());
    div()
        .id(id)
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| crate::ui::zoom::pressed(&zid, &zk2, zart.clone(), 10.))
        .child(crate::ui::zoom::track(zkey))
        .on_hover(move |on, _, cx| {
            if *on {
                crate::views::detail::prefetch(&pre, cx);
                crate::ui::ambient::hover(&pre, cx);
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
