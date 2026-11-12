//! Rows of cards: drag them sideways (with a little momentum when you let
//! go), or use the round arrows at either end. Up/down on the wheel still
//! scrolls the page (see `smooth`).
use crate::theme;
use gpui::{canvas, div, point, prelude::*, px, AnyElement, DispatchPhase, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollHandle, Stateful, Div};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::Instant;

struct Drag {
    key: String,
    start_x: f32,
    start_off: f32,
    last_x: f32,
    last_t: Instant,
    vel: f32,
    moved: bool,
}

thread_local! {
    static HANDLES: RefCell<HashMap<String, ScrollHandle>> = RefCell::new(HashMap::new());
    /// where each row is gliding to (arrows, momentum)
    static TARGET: RefCell<HashMap<String, (f32, Instant)>> = RefCell::new(HashMap::new());
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
    /// the press that just ended was a drag: it isn't a click on a card
    static DRAGGED: Cell<bool> = const { Cell::new(false) };
}

/// Was the last press a drag? (cards ignore the click then)
pub fn dragged() -> bool {
    DRAGGED.with(|d| d.get())
}

fn handle(key: &str) -> ScrollHandle {
    HANDLES.with(|h| h.borrow_mut().entry(key.to_string()).or_default().clone())
}

/// current sideways scroll (positive = moved right) and how far it can go
fn pos(h: &ScrollHandle) -> (f32, f32) {
    (-f32::from(h.offset().x), f32::from(h.max_offset().x).max(0.))
}

fn glide_to(key: &str, to: f32) {
    TARGET.with(|t| t.borrow_mut().insert(key.to_string(), (to, Instant::now())));
}

/// Wrap a row (a `div().id(..).overflow_x_scroll()` with its cards).
pub fn scroller(key: impl ToString, inner: Stateful<Div>) -> AnyElement {
    let key = key.to_string();
    let h = handle(&key);
    let (cur, max) = pos(&h);
    let (k1, k2, k3) = (key.clone(), key.clone(), key.clone());
    let (h1, h2, h3) = (h.clone(), h.clone(), h.clone());
    let arrow = |id: &'static str, left: bool, show: bool, key: String, h: ScrollHandle| {
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .when(left, |d| d.left(px(8.)))
            .when(!left, |d| d.right(px(8.)))
            .flex()
            .items_center()
            .opacity(0.)
            .group_hover("row", |s| s.opacity(if show { 1. } else { 0. }))
            .child(
                div()
                    .id(gpui::SharedString::from(format!("{id}-{key}")))
                    .size(px(44.))
                    .rounded_full()
                    .bg(theme::black(0.72))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::black(0.88)))
                    .when(show, |d| {
                        d.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(move |_, window, _| {
                            let (cur, max) = pos(&h);
                            let page = (f32::from(h.bounds().size.width) * 0.8).max(300.);
                            let base = TARGET.with(|t| t.borrow().get(&key).map(|v| v.0)).unwrap_or(cur);
                            glide_to(&key, (base + if left { -page } else { page }).clamp(0., max));
                            window.refresh();
                        })
                    })
                    .child(crate::ui::icon("chevron", 22.).when(left, |s| s.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI))))),
            )
    };
    div()
        .relative()
        .group("row")
        .child(
            inner
                .track_scroll(&h)
                .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, _, _| {
                    DRAGGED.with(|d| d.set(false));
                    TARGET.with(|t| t.borrow_mut().remove(&k1));
                    let x = f32::from(e.position.x);
                    DRAG.with(|d| *d.borrow_mut() = Some(Drag { key: k1.clone(), start_x: x, start_off: pos(&h1).0, last_x: x, last_t: Instant::now(), vel: 0., moved: false }));
                }),
        )
        // the drag itself, the momentum after it, and the arrows' glide
        .child(
            canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    if let Some((to, since)) = TARGET.with(|t| t.borrow().get(&k2).copied()) {
                        let (cur, max) = pos(&h2);
                        let to = to.clamp(0., max);
                        let dt = since.elapsed().as_secs_f32().clamp(0.001, 0.05);
                        TARGET.with(|t| t.borrow_mut().insert(k2.clone(), (to, Instant::now())));
                        let d = to - cur;
                        if d.abs() < 0.5 {
                            h2.set_offset(point(px(-to), h2.offset().y));
                            TARGET.with(|t| t.borrow_mut().remove(&k2));
                        } else {
                            let k = 1. - (-dt / 0.09).exp();
                            h2.set_offset(point(px(-(cur + d * k)), h2.offset().y));
                            window.request_animation_frame();
                        }
                    }
                    let h = h3.clone();
                    let key = k3.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _| {
                        if phase != DispatchPhase::Bubble {
                            return;
                        }
                        DRAG.with(|d| {
                            let mut d = d.borrow_mut();
                            let Some(g) = d.as_mut().filter(|g| g.key == key) else { return };
                            if e.pressed_button != Some(MouseButton::Left) {
                                *d = None;
                                return;
                            }
                            let x = f32::from(e.position.x);
                            if (x - g.start_x).abs() > 6. {
                                g.moved = true;
                            }
                            if g.moved {
                                let now = Instant::now();
                                let dt = now.duration_since(g.last_t).as_secs_f32().max(0.001);
                                g.vel = 0.7 * g.vel + 0.3 * ((x - g.last_x) / dt);
                                g.last_x = x;
                                g.last_t = now;
                                let max = pos(&h).1;
                                h.set_offset(point(px(-(g.start_off - (x - g.start_x)).clamp(0., max)), h.offset().y));
                                window.refresh();
                            }
                        });
                    });
                    let key = k3.clone();
                    // let go: a drag isn't a click; it carries on a little
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, window, _| {
                        if phase != DispatchPhase::Capture {
                            return;
                        }
                        DRAG.with(|d| {
                            let mut d = d.borrow_mut();
                            if d.as_ref().map_or(false, |g| g.key == key) {
                                let g = d.take().unwrap();
                                if g.moved {
                                    DRAGGED.with(|x| x.set(true));
                                    let cur = -g.start_off + 0.; // unused
                                    let _ = cur;
                                    let fresh = g.last_t.elapsed().as_secs_f32() < 0.08;
                                    if fresh && g.vel.abs() > 200. {
                                        let now_off = (g.start_off - (g.last_x - g.start_x)).max(0.);
                                        glide_to(&key, now_off - g.vel * 0.22);
                                        window.refresh();
                                    }
                                }
                            }
                        });
                    });
                },
            )
            .absolute()
            .size_0(),
        )
        .child(arrow("rl", true, cur > 1., key.clone(), h.clone()))
        .child(arrow("rr", false, cur < max - 1., key.clone(), h.clone()))
        .into_any_element()
}
