#![allow(dead_code)] // a design system: every token is there to be reached for
//! How couch moves. Every curve, every duration and the house effects live
//! here, so the whole app moves like one room instead of forty animations.
//!
//! The room at 2 AM has four kinds of motion, and nothing else:
//!
//! - **lamp**: light warming up. Things *arrive* by fading up from a little
//!   below on a long, soft out-curve. No bounce, no blur, no swoosh.
//! - **spring**: something you touched answers with weight (cards, toasts,
//!   a poster dropped on the pile). The only place anything overshoots.
//! - **channel**: a change of context dips through dark (tabs, Wrapped cards,
//!   the player opening). The old picture folds to a bright line, the new one
//!   opens out of it.
//! - **neon**: the sign switching on. Two or three stutters, then steady.
//!   Once per screen, on the one thing that matters.
//!
//! Plus the room's texture, which never stops: film grain at 24 fps, the
//! mascot's line boil at 12 fps, and (in broadcast moments) scanlines.
//!
//! Durations are in seconds. Curves take t in 0..1 and return 0..1 (a spring
//! goes a little past 1 and comes back).
use gpui::{canvas, prelude::*, px, Hsla, Window};
use std::time::Instant;

/// How long things take. Short for what answers you, long for what sets a mood.
pub mod dur {
    /// a press answering
    pub const TAP: f32 = 0.12;
    /// hovers, small state changes
    pub const QUICK: f32 = 0.2;
    /// a page turning
    pub const TURN: f32 = 0.32;
    /// a channel change (fold to a line, open the next)
    pub const CHANNEL: f32 = 0.46;
    /// something settling into place
    pub const SETTLE: f32 = 0.56;
    /// light warming up (entrances)
    pub const LAMP: f32 = 0.8;
    /// one beat of a bump: the pause between lines that land one by one
    pub const BEAT: f32 = 0.55;
    /// the neon sign switching on
    pub const NEON: f32 = 1.3;
    /// the room changing its light (artwork glow, the night warming)
    pub const ROOM: f32 = 4.0;
}

/// Seconds since `t0`.
pub fn secs(t0: Instant) -> f32 {
    t0.elapsed().as_secs_f32()
}

/// How far along a move is: `delay` seconds of waiting, then 0..1 over `dur`.
pub fn along(t: f32, delay: f32, dur: f32) -> f32 {
    ((t - delay) / dur.max(0.001)).clamp(0., 1.)
}

// ---------------------------------------------------------------- curves

/// fast start, long soft landing: the default for anything arriving
pub fn out3(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}
/// softer still: light warming up
pub fn lamp(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(4)
}
/// slow, fast, slow: things that leave and come back (the intro, a fold)
pub fn in_out3(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    if t < 0.5 { 4. * t * t * t } else { 1. - (-2. * t + 2.).powi(3) / 2. }
}
/// the gentlest in-out (cross-fades)
pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
/// accelerating away: things leaving
pub fn in3(t: f32) -> f32 {
    t.clamp(0., 1.).powi(3)
}

/// A real damped spring (mass 1), from rest at 0 to 1, `t` in seconds.
/// stiffness 260 / damping 22 is couch's "bouncy" (a touch of overshoot);
/// 170 / 26 is "smooth" (none). Same springs as the webview's motion.ts.
pub fn spring(t: f32, stiffness: f32, damping: f32) -> f32 {
    if t <= 0. {
        return 0.;
    }
    let w0 = stiffness.sqrt();
    let zeta = damping / (2. * stiffness.sqrt());
    if zeta < 1. {
        let wd = w0 * (1. - zeta * zeta).sqrt();
        1. - (-zeta * w0 * t).exp() * ((wd * t).cos() + (zeta * w0 / wd) * (wd * t).sin())
    } else {
        1. - (-w0 * t).exp() * (1. + w0 * t)
    }
}
/// couch's bouncy spring (touch, drops, toasts)
pub fn bouncy(t: f32) -> f32 {
    spring(t, 260., 22.)
}
/// couch's smooth spring (no overshoot, settles quickly)
pub fn glide(t: f32) -> f32 {
    spring(t, 170., 26.)
}

/// A line of a bump landing on its beat: (opacity, y offset in px). Line `n`
/// waits `n` beats, then warms up from `rise` px below.
pub fn beat(t: f32, n: usize, rise: f32) -> (f32, f32) {
    let k = lamp(along(t, n as f32 * dur::BEAT, dur::LAMP));
    (k, (1. - k) * rise)
}

// ------------------------------------------------------------ house effects

/// The neon sign switching on, `t` seconds after it was told to: brightness
/// 0..1 with the stutters of a real tube (dark, a flash, dark, nearly, a dip,
/// then steady). Brighter than 1 on the first catch.
pub fn neon(t: f32) -> f32 {
    let k = t / dur::NEON;
    match k {
        k if k < 0.08 => 0.,
        k if k < 0.11 => 1.15,
        k if k < 0.18 => 0.12,
        k if k < 0.22 => 0.85,
        k if k < 0.30 => 0.08,
        k if k < 0.36 => 1.1,
        k if k < 0.42 => 0.55,
        _ => 1.,
    }
}

/// A steady tube still breathes a little (and buzzes, now and then).
pub fn hum(t: f32) -> f32 {
    let buzz = if (t * 0.37).fract() < 0.012 { 0.7 } else { 1. };
    (0.94 + 0.06 * (t * 2.1).sin()) * buzz
}

/// An unset VCR clock: on for most of a second, off for the rest.
pub fn blink(t: f32) -> bool {
    t.fract() < 0.62
}

/// The channel change, `t` seconds in: how open the picture is (1 = all of
/// it, 0 = a bright line across the middle) and how bright that line is.
/// Folds shut over the first 40%, holds dark a moment, opens over the rest.
/// `swap` says when to show the new channel (from the dark moment on).
pub struct Channel {
    pub open: f32,
    pub line: f32,
    pub swap: bool,
    pub snow: f32,
}
pub fn channel(t: f32) -> Channel {
    let k = (t / dur::CHANNEL).clamp(0., 1.);
    if k < 0.4 {
        let e = in3(k / 0.4);
        Channel { open: 1. - e, line: e, swap: false, snow: e * 0.6 }
    } else if k < 0.5 {
        Channel { open: 0., line: 1. - (k - 0.4) / 0.1 * 0.6, swap: true, snow: 0.9 }
    } else {
        let e = out3((k - 0.5) / 0.5);
        Channel { open: e, line: (1. - e) * 0.4, swap: true, snow: (1. - e) * 0.5 }
    }
}

/// Shutters for a channel change over an area `h` px tall: black bands
/// closing from the top and the bottom toward a glowing line.
pub fn shutters(c: &Channel, h: f32, glow: Hsla) -> impl IntoElement {
    let band = (h / 2.) * (1. - c.open);
    let mut line = glow;
    line.a = c.line.clamp(0., 1.);
    gpui::div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(gpui::div().absolute().top_0().left_0().w_full().h(px(band)).bg(gpui::black()))
        .child(gpui::div().absolute().bottom_0().left_0().w_full().h(px(band)).bg(gpui::black()))
        .when(c.line > 0.02, |d| {
            d.child(
                gpui::div()
                    .absolute()
                    .left(px(0.))
                    .w_full()
                    .top(px(h / 2. - 1.))
                    .h(px(2.))
                    .bg(line)
                    .shadow(vec![gpui::BoxShadow { color: line, offset: gpui::point(px(0.), px(0.)), blur_radius: px(18.), spread_radius: px(2.), inset: false }]),
            )
        })
}

/// cheap, stable noise for (x, y, frame)
pub fn hash(x: u32, y: u32, frame: u32) -> f32 {
    let mut v = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ frame.wrapping_mul(2246822519);
    v = (v ^ (v >> 13)).wrapping_mul(1274126177);
    ((v ^ (v >> 16)) & 1023) as f32 / 1023.
}

/// Film grain over an area: a scatter of light and dark specks, a new
/// scatter 24 times a second. `amount` 0..1 (0.15 is a whisper, 1 is snow).
pub fn grain(t: f32, amount: f32) -> impl IntoElement {
    let frame = (t * 24.) as u32;
    canvas(
        |_, _, _| (),
        move |b, _, window: &mut Window, _| {
            if amount <= 0.01 {
                return;
            }
            let cell = 3.;
            let (x0, y0) = (f32::from(b.origin.x), f32::from(b.origin.y));
            let (cols, rows) = ((f32::from(b.size.width) / cell / 4.) as u32, (f32::from(b.size.height) / cell / 4.) as u32);
            // only some cells get a speck: cheap enough for a whole window
            let keep = 0.18 + 0.5 * amount;
            for j in 0..rows {
                for i in 0..cols {
                    let h = hash(i, j, frame);
                    if h > keep {
                        continue;
                    }
                    let jx = hash(j, i, frame ^ 0x9e37) * cell * 4.;
                    let jy = hash(i ^ 77, j, frame) * cell * 4.;
                    let light = hash(i, j ^ 31, frame) > 0.5;
                    let a = amount * (0.05 + 0.25 * hash(j, i ^ 5, frame));
                    let c = if light { gpui::hsla(0.9, 0.3, 0.95, a) } else { gpui::hsla(0., 0., 0., a * 1.4) };
                    let at = gpui::point(px(x0 + i as f32 * cell * 4. + jx), px(y0 + j as f32 * cell * 4. + jy));
                    window.paint_quad(gpui::fill(gpui::Bounds::new(at, gpui::size(px(cell * 0.7), px(cell * 0.7))), c));
                }
            }
        },
    )
    .absolute()
    .size_full()
}

/// Faint scanlines (a broadcast, not a film): a dark line every few pixels,
/// drifting slowly downward.
pub fn scanlines(t: f32, alpha: f32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |b, _, window: &mut Window, _| {
            let gap = 4.;
            let (x0, y0, w, h) = (f32::from(b.origin.x), f32::from(b.origin.y), b.size.width, f32::from(b.size.height));
            let off = (t * 6.) % gap;
            let mut y = off;
            while y < h {
                window.paint_quad(gpui::fill(gpui::Bounds::new(gpui::point(px(x0), px(y0 + y)), gpui::size(w, px(1.5))), gpui::hsla(0., 0., 0., alpha)));
                y += gap;
            }
        },
    )
    .absolute()
    .size_full()
}

/// A soft pool of light (a lamp, a neon spill): a disc with a wide glow.
/// `x`, `y` are its centre, `r` its radius, `c` its colour and strength.
pub fn pool(x: f32, y: f32, r: f32, c: Hsla) -> impl IntoElement {
    // only the shadow is drawn: a disc blurred to nothing at its edge
    let q = r * 0.5;
    gpui::div()
        .absolute()
        .left(px(x - q))
        .top(px(y - q))
        .size(px(q * 2.))
        .rounded_full()
        .shadow(vec![gpui::BoxShadow { color: c, offset: gpui::point(px(0.), px(0.)), blur_radius: px(r), spread_radius: px(q * 0.4), inset: false }])
}
