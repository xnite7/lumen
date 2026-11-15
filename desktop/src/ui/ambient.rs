//! The light behind everything: the artwork you're on, enormous and blurred,
//! glowing down from the top of the window and cross-fading when it changes.
use gpui::{canvas, div, prelude::*, px, App, Bounds, Corners, Pixels, Window};
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Instant;

#[derive(Default)]
struct State {
    want: String,
    cur: Option<Arc<gpui::Image>>,
    prev: Option<Arc<gpui::Image>>,
    at: Option<Instant>,
}

thread_local! {
    static S: RefCell<State> = RefCell::new(State::default());
    /// where the glow was painted this frame (the window)
    static AREA: std::cell::Cell<Option<Bounds<Pixels>>> = const { std::cell::Cell::new(None) };
}

const FADE: f32 = 0.7;

/// Tiny, soft and dim: stretched over the window it reads as a glow.
fn glow(bytes: &[u8]) -> Option<Arc<gpui::Image>> {
    let im = image::load_from_memory(bytes).ok()?;
    let small = image::imageops::resize(&im.to_rgba8(), 48, 27, image::imageops::FilterType::Triangle);
    let mut soft = image::imageops::blur(&small, 2.2);
    let h = soft.height() as f32;
    for (_, y, p) in soft.enumerate_pixels_mut() {
        // brightest along the top, gone by the bottom
        let fall = (1. - y as f32 / (h - 1.)).powf(1.35);
        let k = 0.46 * fall;
        // a little more colour than the picture has (dim pictures still glow)
        let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
        let l = 0.299 * r + 0.587 * g + 0.114 * b;
        let sat = |c: f32| (l + (c - l) * 1.35).clamp(0., 255.);
        p[0] = (sat(r) * k) as u8;
        p[1] = (sat(g) * k) as u8;
        p[2] = (sat(b) * k) as u8;
        p[3] = 255;
    }
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(soft).write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, out.into_inner())))
}

thread_local! {
    static HOME: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn art(m: &crate::api::Meta) -> Option<String> {
    m.background.clone().or(m.poster.clone()).map(|u| crate::api::sized(&u, "w342"))
}

/// Home's billboard title: Home glows with it.
pub fn set_home(m: &crate::api::Meta, cx: &mut App) {
    let a = art(m);
    HOME.with(|h| *h.borrow_mut() = a.clone());
    if matches!(crate::Nav::route(cx), None | Some(crate::Route::Home)) {
        set(a, cx);
    }
}

/// A page opened: a title's page glows with that title, Home with its billboard;
/// browsing pages keep the last glow until you point at something.
pub fn follow(route: &crate::Route, cx: &mut App) {
    match route {
        crate::Route::Detail(m) => set(art(m), cx),
        crate::Route::Home => set(HOME.with(|h| h.borrow().clone()), cx),
        _ => {}
    }
}

/// Pointing at a title: only on the pages for browsing (search, lists, rows, people, studios…).
pub fn hover(m: &crate::api::Meta, cx: &mut App) {
    use crate::Route::*;
    if matches!(crate::Nav::route(cx), Some(Search | Lists | Catalog(_) | Person(..) | Studio(..) | Following | Vibe | Member(_) | Calendar | Shows | Movies)) {
        set(art(m), cx);
    }
}

/// This is the artwork in front of you now.
pub fn set(url: Option<String>, cx: &mut App) {
    let Some(url) = url.filter(|u| !u.is_empty()) else { return };
    let same = S.with(|s| {
        let mut s = s.borrow_mut();
        if s.want == url {
            return true;
        }
        s.want = url.clone();
        false
    });
    if same {
        return;
    }
    cx.spawn(async move |cx| {
        // a moment, so sweeping across a row doesn't load every poster
        crate::timer::after(std::time::Duration::from_millis(140)).await;
        if S.with(|s| s.borrow().want != url) {
            return;
        }
        let u = url.clone();
        let pic = couch::rt::spawn(async move {
            let core = couch::state::get().core.clone();
            couch::images::load(&core, &u).await.ok().and_then(|(b, _)| glow(&b))
        })
        .await
        .ok()
        .flatten();
        let Some(pic) = pic else { return };
        let ok = S.with(|s| {
            let mut s = s.borrow_mut();
            if s.want != url {
                return false;
            }
            s.prev = s.cur.take();
            s.cur = Some(pic);
            s.at = Some(Instant::now());
            true
        });
        if ok {
            let _ = cx.update(|cx| cx.refresh_windows());
        }
    })
    .detach();
}

fn piece(pic: Arc<gpui::Image>) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, cx| {
            AREA.with(|a| a.set(Some(bounds)));
            let Some(data) = pic.clone().use_render_image(window, cx) else { return };
            // never over a playing clip (it shows through a hole in the window)
            let parts: Vec<Bounds<Pixels>> = match crate::ui::video::hole() {
                None => vec![bounds],
                Some(r) => {
                    let (o, s) = (bounds.origin, bounds.size);
                    vec![
                        Bounds::new(o, gpui::size(s.width, r.top() - o.y)),
                        Bounds::new(gpui::point(o.x, r.bottom()), gpui::size(s.width, o.y + s.height - r.bottom())),
                        Bounds::new(gpui::point(o.x, r.top()), gpui::size(r.left() - o.x, r.size.height)),
                        Bounds::new(gpui::point(r.right(), r.top()), gpui::size(o.x + s.width - r.right(), r.size.height)),
                    ]
                }
            };
            for q in parts {
                if q.size.width > px(0.) && q.size.height > px(0.) {
                    let _ = window.paint_image(q, bounds, Corners::default(), data.clone(), 0, false);
                }
            }
        },
    )
    .absolute()
    .size_full()
}

/// The glow, under the page.
pub fn layer(window: &mut Window) -> impl IntoElement {
    let (cur, prev, t) = S.with(|s| {
        let s = s.borrow();
        let t = s.at.map_or(1., |a| (a.elapsed().as_secs_f32() / FADE).min(1.));
        (s.cur.clone(), s.prev.clone(), t)
    });
    if t < 1. {
        window.request_animation_frame();
    }
    // ease in and out
    let e = t * t * (3. - 2. * t);
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .children(prev.filter(|_| t < 1.).map(|p| div().absolute().size_full().opacity(1. - e).child(piece(p))))
        .children(cur.map(|c| div().absolute().size_full().opacity(e).child(piece(c))))
}

/// The bottom of a big picture (a billboard, a title's backdrop) melting into
/// the page: the glow that's behind the page painted over it, more and more
/// of it toward the bottom, so there's no edge where the picture stops.
/// Put it absolute, at the bottom of the picture, as tall as the melt.
pub fn melt() -> impl IntoElement {
    // many thin strips, each on whole screen pixels: no seams (two strips
    // overlapping by a pixel drew a darker line) and steps too small to see
    const STEPS: usize = 72;
    let cur = S.with(|s| s.borrow().cur.clone());
    let strip = |i: usize| {
        let cur = cur.clone();
        canvas(
            |_, _, _| (),
            move |b, _, window, cx| {
                let sf = window.scale_factor();
                let snap = |y: Pixels| px((f32::from(y) * sf).round() / sf);
                let h = b.size.height / STEPS as f32;
                let (y0, y1) = (snap(b.origin.y + h * i as f32), snap(b.origin.y + h * (i + 1) as f32));
                if y1 <= y0 {
                    return;
                }
                let r = Bounds::new(gpui::point(b.origin.x, y0), gpui::size(b.size.width, y1 - y0));
                let area = AREA.with(|a| a.get()).unwrap_or(r);
                // under the glow: the page's black
                window.paint_quad(gpui::fill(r, gpui::black()));
                if let Some(data) = cur.clone().and_then(|p| p.use_render_image(window, cx)) {
                    let _ = window.paint_image(r, area, Corners::default(), data, 0, false);
                }
            },
        )
        .absolute()
        .size_full()
    };
    div().absolute().left_0().bottom_0().w_full().h_full().children((0..STEPS).map(move |i| {
        // eased: barely there at the top, the page itself at the bottom
        let x = (i as f32 + 0.5) / STEPS as f32;
        let a = x * x * (3. - 2. * x);
        div().absolute().size_full().opacity(a).child(strip(i))
    }))
}
