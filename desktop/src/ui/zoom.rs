//! Opening a title from a card: the card's picture grows into the title's
//! page (its backdrop), then the rest of the page fades in. Back shrinks it
//! into the card again.
use gpui::{canvas, div, img, prelude::*, px, AnyElement, App, Bounds, ObjectFit, Pixels, Window};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

pub const DUR: f32 = 0.42;
/// how long the page's words take to fade in after the picture lands
const AFTER: f32 = 0.3;

#[derive(Clone)]
struct Run {
    from: Bounds<Pixels>,
    radius: f32,
    small: Option<String>,
    big: Option<String>,
    at: Instant,
    back: bool,
    /// the card's logo: where it sat on the card, and its picture
    logo: Option<(Bounds<Pixels>, String)>,
}

#[derive(Default)]
struct S {
    /// each card's place on screen, by its key (painted this or last frame)
    places: HashMap<String, Bounds<Pixels>>,
    /// the card just pressed: (title id, its key, its picture, corner radius, when)
    pressed: Option<(String, String, Option<String>, f32, Instant)>,
    /// each card's logo picture, by the card's key
    logos: HashMap<String, String>,
    /// where the open page's title logo sits (the logo flies into it)
    target: Option<Bounds<Pixels>>,
    run: Option<Run>,
    /// titles opened from a card, newest last: back returns into that card
    opened: Vec<(String, Run)>,
}

thread_local! {
    static Z: RefCell<S> = RefCell::new(S::default());
}

/// Inside a card: remembers where it is.
pub fn track(key: String) -> impl IntoElement {
    canvas(
        move |b, _, _| {
            Z.with(|z| {
                z.borrow_mut().places.insert(key.clone(), b);
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

/// Around a card's logo: remembers where it is and what it shows.
pub fn track_logo(key: String, url: String) -> impl IntoElement {
    canvas(
        move |b, _, _| {
            Z.with(|z| {
                let mut z = z.borrow_mut();
                z.places.insert(format!("{key}#logo"), b);
                z.logos.insert(key.clone(), url.clone());
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

/// Around the open page's title logo: where the flying logo lands.
pub fn track_target() -> impl IntoElement {
    canvas(
        move |b, _, _| {
            Z.with(|z| z.borrow_mut().target = Some(b));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

/// A card was pressed (just before its click opens the page).
pub fn pressed(id: &str, key: &str, pic: Option<String>, radius: f32) {
    Z.with(|z| z.borrow_mut().pressed = Some((id.to_string(), key.to_string(), pic, radius, Instant::now())));
}

/// Opening `m`'s page: from the card that was just pressed, if it was this title's.
pub fn open(m: &crate::api::Meta, big: Option<String>) {
    Z.with(|z| {
        let mut z = z.borrow_mut();
        let Some((id, key, small, radius, at)) = z.pressed.take() else { return };
        if id != m.id || at.elapsed().as_secs_f32() > 1. {
            return;
        }
        let Some(from) = z.places.get(&key).copied() else { return };
        let logo = z.places.get(&format!("{key}#logo")).copied().zip(z.logos.get(&key).cloned());
        z.target = None;
        let run = Run { from, radius, small, big, at: Instant::now(), back: false, logo };
        z.opened.retain(|o| o.0 != m.id);
        z.opened.push((m.id.clone(), run.clone()));
        if z.opened.len() > 20 {
            z.opened.remove(0);
        }
        z.run = Some(run);
    });
}

/// Leaving `id`'s page by going back: shrink into the card it came from.
pub fn back(id: &str) {
    Z.with(|z| {
        let mut z = z.borrow_mut();
        let Some(i) = z.opened.iter().rposition(|o| o.0 == id) else { return };
        let (_, mut run) = z.opened.remove(i);
        run.back = true;
        run.at = Instant::now();
        z.run = Some(run);
    });
}

/// Leaving any other way: no zoom to come back into.
pub fn forget(id: &str) {
    Z.with(|z| z.borrow_mut().opened.retain(|o| o.0 != id));
}

/// How long the whole opening lasts: the picture, then the page settling in.
const SETTLE: f32 = 0.75;

/// The page opening from a card, this frame.
pub struct Opening {
    /// seconds since it started
    pub t: f32,
    /// the picture's eased progress, card (0) to backdrop (1)
    pub k: f32,
    pub from: Bounds<Pixels>,
    pub radius: f32,
    pub small: Option<String>,
    pub big: Option<String>,
    pub logo: Option<(Bounds<Pixels>, String)>,
    pub target: Option<Bounds<Pixels>>,
}

pub fn opening(window: &mut Window) -> Option<Opening> {
    Z.with(|z| {
        let z = z.borrow();
        let r = z.run.as_ref().filter(|r| !r.back)?;
        let t = r.at.elapsed().as_secs_f32();
        if t > DUR + SETTLE {
            return None;
        }
        window.request_animation_frame();
        Some(Opening { t, k: ease((t / DUR).min(1.)), from: r.from, radius: r.radius, small: r.small.clone(), big: r.big.clone(), logo: r.logo.clone(), target: z.target })
    })
}

/// Is a page opening from a card right now (the page turn stays still for it)?
pub fn is_opening() -> bool {
    Z.with(|z| z.borrow().run.as_ref().map_or(false, |r| !r.back && r.at.elapsed().as_secs_f32() < DUR + SETTLE))
}

/// The page's parts arrive one after another once the picture lands:
/// part `i` (0 first) -> (how much it shows, how far below its place it still is).
pub fn rise(window: &mut Window, i: usize) -> (f32, f32) {
    let Some(t) = Z.with(|z| z.borrow().run.as_ref().filter(|r| !r.back).map(|r| r.at.elapsed().as_secs_f32())) else { return (1., 0.) };
    if t > DUR + SETTLE {
        return (1., 0.);
    }
    window.request_animation_frame();
    let x = ((t - DUR * 0.7 - i as f32 * 0.06) / 0.38).clamp(0., 1.);
    let e = crate::motion::out3(x);
    (e, (1. - e) * 18.)
}

/// How much of a title page's words show (0 while its picture is still flying in).
pub fn content_alpha(window: &mut Window) -> f32 {
    rise(window, 6).0
}

fn ease(x: f32) -> f32 {
    // fast out, soft landing
    1. - (1. - x).powi(4)
}

/// The flying picture, over everything.
pub fn overlay(window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
    let run = Z.with(|z| {
        let mut z = z.borrow_mut();
        // places are from the frame before; start fresh each frame
        z.places.retain(|_, _| true);
        if z.run.as_ref().map_or(false, |r| r.at.elapsed().as_secs_f32() > if r.back { DUR + 0.12 } else { DUR + SETTLE }) {
            z.run = None;
        }
        z.run.clone().filter(|r| r.back)
    })?;
    window.request_animation_frame();
    let vs = window.viewport_size();
    let to = Bounds::new(gpui::point(px(0.), px(0.)), gpui::size(vs.width, vs.height * 0.92));
    let t = (run.at.elapsed().as_secs_f32() / DUR).min(1.);
    let k = if run.back { 1. - ease(t) } else { ease(t) };
    let lerp = |a: Pixels, b: Pixels| a + (b - a) * k;
    let (x, y) = (lerp(run.from.origin.x, to.origin.x), lerp(run.from.origin.y, to.origin.y));
    let (w, h) = (lerp(run.from.size.width, to.size.width), lerp(run.from.size.height, to.size.height));
    let r = run.radius * (1. - k);
    // the card's picture gives way to the backdrop on the way
    let big_a = ((k - 0.15) / 0.55).clamp(0., 1.);
    // the very end: it hands over to the page itself
    let end = ((run.at.elapsed().as_secs_f32() - DUR) / 0.12).clamp(0., 1.);
    let alpha = 1. - end;
    Some(
        div()
            .absolute()
            .left(x)
            .top(y)
            .w(w)
            .h(h)
            .rounded(px(r))
            .overflow_hidden()
            .opacity(alpha)
            .bg(gpui::black())
            .children(run.small.clone().map(|u| img(u).absolute().size_full().object_fit(ObjectFit::Cover).opacity(1. - big_a)))
            .children(run.big.clone().map(|u| img(u).absolute().size_full().object_fit(ObjectFit::Cover).opacity(big_a)))
            .into_any_element(),
    )
}

#[allow(dead_code)]
fn _unused() -> gpui::Div {
    div()
}
