//! Smooth scrolling. GPUI jumps a whole notch at a time on a mouse wheel;
//! this catches the wheel over a page first, keeps a target, and glides the
//! page there over a few frames (like a browser). Up/down over a row of
//! cards scrolls the page, not the row; sideways still moves the row.
use gpui::{canvas, point, prelude::*, px, DispatchPhase, ScrollDelta, ScrollHandle, ScrollWheelEvent};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

thread_local! {
    static TARGET: RefCell<HashMap<&'static str, f32>> = RefCell::new(HashMap::new());
    /// when each page last moved a step (for the time-based glide)
    static LAST: RefCell<HashMap<&'static str, std::time::Instant>> = RefCell::new(HashMap::new());
    /// a sheet or menu is open over the page: leave the wheel to it
    pub static OVERLAY: Cell<bool> = const { Cell::new(false) };
    /// how far the page on screen is scrolled (the top bar darkens with it)
    pub static SCROLLED: Cell<f32> = const { Cell::new(0.) };
}

/// how far the page on screen is scrolled down, in pixels
pub fn scrolled() -> f32 {
    SCROLLED.with(|s| s.get())
}

/// how far one notch goes, and how quickly the page catches up: it closes
/// the gap with a time constant (same feel at 60, 144 or 240 Hz)
const NOTCH: f32 = 1.9;
const TAU: f32 = 0.085;

pub fn set_overlay(on: bool) {
    OVERLAY.with(|o| o.set(on));
}

/// Put inside a page that uses `.overflow_y_scroll().track_scroll(h)`.
pub fn wheel(h: &ScrollHandle, key: &'static str) -> impl IntoElement {
    let h = h.clone();
    canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            // glide toward the target
            let cur = -f32::from(h.offset().y);
            let max = f32::from(h.max_offset().y).max(0.);
            if (SCROLLED.with(|s| s.get()) - cur).abs() > 0.5 {
                SCROLLED.with(|s| s.set(cur));
                window.refresh();
            }
            if let Some(t) = TARGET.with(|m| m.borrow().get(key).copied()) {
                let t = t.clamp(0., max);
                let d = t - cur;
                let now = std::time::Instant::now();
                let dt = LAST.with(|l| l.borrow_mut().insert(key, now)).map_or(1. / 120., |p| now.duration_since(p).as_secs_f32()).clamp(0.001, 0.05);
                if d.abs() < 0.4 {
                    h.set_offset(point(h.offset().x, px(-t)));
                    TARGET.with(|m| m.borrow_mut().remove(key));
                    LAST.with(|l| l.borrow_mut().remove(key));
                } else {
                    let k = 1. - (-dt / TAU).exp();
                    h.set_offset(point(h.offset().x, px(-(cur + d * k))));
                    window.request_animation_frame();
                }
            }
            let h2 = h.clone();
            window.on_mouse_event(move |e: &ScrollWheelEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || OVERLAY.with(|o| o.get()) || !h2.bounds().contains(&e.position) {
                    return;
                }
                let d = e.delta.pixel_delta(window.line_height());
                let (dx, dy) = (f32::from(d.x), f32::from(d.y));
                // sideways (trackpad / shift): the row under the pointer handles it
                if dy.abs() <= dx.abs() || dy == 0. {
                    return;
                }
                let max = f32::from(h2.max_offset().y).max(0.);
                let cur = -f32::from(h2.offset().y);
                match e.delta {
                    // a mouse wheel: glide
                    ScrollDelta::Lines(_) => {
                        let base = TARGET.with(|m| m.borrow().get(key).copied()).unwrap_or(cur);
                        let t = (base - dy * NOTCH).clamp(0., max);
                        TARGET.with(|m| m.borrow_mut().insert(key, t));
                    }
                    // a trackpad already moves smoothly: follow it directly
                    ScrollDelta::Pixels(_) => {
                        TARGET.with(|m| m.borrow_mut().remove(key));
                        h2.set_offset(point(h2.offset().x, px(-(cur - dy).clamp(0., max))));
                    }
                }
                cx.stop_propagation();
                window.refresh();
            });
        },
    )
    .absolute()
    .size_0()
}
