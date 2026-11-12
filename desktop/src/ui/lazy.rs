//! Only draw what's on screen. Each section of a long page is measured the
//! first time it's drawn; while it's well off screen, a blank of the same
//! height stands in for it (so the page doesn't jump), which keeps each
//! frame light enough to scroll at the screen's full refresh rate.
use gpui::{canvas, prelude::*, px, AnyElement, Pixels, ScrollHandle};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// section → (top within the page, height), from the last time it was drawn
    static SEEN: RefCell<HashMap<String, (f32, f32)>> = RefCell::new(HashMap::new());
}

/// how far past the edges of the screen sections are still drawn
const MARGIN: f32 = 900.;

/// Draw `build()` if `key` is on (or near) the screen, else a blank of its size.
pub fn section(key: String, scroll: &ScrollHandle, viewport_h: Pixels, build: impl FnOnce() -> gpui::Div) -> AnyElement {
    let off = -f32::from(scroll.offset().y);
    let vh = f32::from(viewport_h);
    if let Some((top, h)) = SEEN.with(|s| s.borrow().get(&key).copied()) {
        let visible = top + h > off - MARGIN && top < off + vh + MARGIN;
        if !visible {
            return gpui::div().w_full().h(px(h)).into_any_element();
        }
    }
    let s = scroll.clone();
    build()
        .relative()
        .child(
            canvas(
                move |bounds, _, _| {
                    // where it sits in the page (not on screen): add the scroll back
                    let page_top = f32::from(s.bounds().origin.y);
                    let top = f32::from(bounds.origin.y) - page_top - f32::from(s.offset().y);
                    let h = f32::from(bounds.size.height);
                    SEEN.with(|m| m.borrow_mut().insert(key, (top, h)));
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .into_any_element()
}
