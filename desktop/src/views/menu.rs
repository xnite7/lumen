//! Right-click on any card or poster: the quick things (play, add to, mark
//! watched, watch party, copy link).
use crate::api::{self, Meta};
use crate::theme;
use crate::ui::icon;
use crate::Nav;
use gpui::{div, prelude::*, px, ClipboardItem, Context, FontWeight, Pixels, Point, SharedString, Window};
use serde_json::{json, Value};

pub struct Menu {
    pub open: Option<(Meta, Point<Pixels>)>,
    /// the title is on "continue watching" (so it can be taken off)
    in_cw: bool,
    /// when it opened, and when it started closing (it fades and drops away)
    at: std::time::Instant,
    closing: Option<std::time::Instant>,
}

/// a menu is up: cards don't grow or play under it
static OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn is_open() -> bool {
    OPEN.load(std::sync::atomic::Ordering::Relaxed)
}

thread_local! {
    static DISMISSED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}
/// titles taken off "continue watching" this session (Home drops them right away)
pub fn dismissed() -> Vec<String> {
    DISMISSED.with(|d| d.borrow().clone())
}

impl Menu {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { open: None, in_cw: false, at: std::time::Instant::now(), closing: None }
    }
    /// fade away, then gone
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open.is_none() || self.closing.is_some() {
            return;
        }
        self.closing = Some(std::time::Instant::now());
        cx.spawn(async move |this, cx| {
            crate::timer::after(std::time::Duration::from_millis(130)).await;
            let _ = this.update(cx, |m, cx| {
                if m.closing.is_some() {
                    m.open = None;
                    m.closing = None;
                    OPEN.store(false, std::sync::atomic::Ordering::Relaxed);
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
    pub fn show(&mut self, m: Meta, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.in_cw = couch::commands::continue_watching(couch::state::get())
            .map(|l| l.iter().any(|i| i["_id"].as_str().or(i["id"].as_str()) == Some(m.id.as_str())))
            .unwrap_or(false)
            && !dismissed().contains(&m.id);
        self.open = Some((m, at));
        self.at = std::time::Instant::now();
        self.closing = None;
        OPEN.store(true, std::sync::atomic::Ordering::Relaxed);
        cx.notify();
    }
}

impl Render for Menu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((m, at)) = self.open.clone() else {
            OPEN.store(false, std::sync::atomic::Ordering::Relaxed);
            return div().absolute().into_any_element();
        };
        // in quickly from just above the pointer; out quicker
        let k = match self.closing {
            Some(c) => 1. - (c.elapsed().as_secs_f32() / 0.12).min(1.),
            None => {
                let x = (self.at.elapsed().as_secs_f32() / 0.16).min(1.);
                1. - (1. - x).powi(3)
            }
        };
        if k > 0. && k < 1. || self.closing.is_some() {
            window.request_animation_frame();
        }
        let vs = window.viewport_size();
        let (w, h) = (290., if self.in_cw { 345. } else { 300. });
        let x = f32::from(at.x).min(f32::from(vs.width) - w - 12.);
        let y = f32::from(at.y).min(f32::from(vs.height) - h - 12.);
        let (seen, listed) = crate::seen::status(&m.id);
        let mut items: Vec<(&'static str, &'static str, Box<dyn Fn(&Meta, &mut gpui::App)>)> = vec![
            ("play", "play", Box::new(|m, cx| Nav::play(m, cx))),
            ("info", "open", Box::new(|m, cx| Nav::detail(m, cx))),
            if listed { ("check", "added", Box::new(|m, cx| Nav::add(m, cx))) } else { ("plus", "add to…", Box::new(|m, cx| Nav::add(m, cx))) },
            (
                if seen { "eyeOff" } else { "eye" },
                if seen { "unwatch" } else { "mark watched" },
                Box::new(move |m, cx| {
                    let on = !seen;
                    let (kind, id, name) = (m.kind.clone(), m.id.clone(), m.name.clone());
                    cx.spawn(async move |cx| {
                        let full = api::call::<Value>("meta_get", json!({ "kind": kind, "id": id, "hint": Value::Null })).await.unwrap_or_default();
                        let today = couch::integrations::tmdb::today_str();
                        let all: Option<Vec<String>> = (full["type"] == "series").then(|| {
                            crate::playback::ordered(&full)
                                .iter()
                                .filter(|v| v["season"].as_i64().unwrap_or(0) > 0 && v["released"].as_str().map_or(true, |r| &r[..r.len().min(10)] <= today.as_str()))
                                .filter_map(|v| v["id"].as_str().map(String::from))
                                .collect()
                        });
                        let r = api::call::<Value>("library_set_watched", json!({ "meta": full, "videoId": Value::Null, "watched": on, "allVideos": all })).await;
                        crate::views::detail::library_changed();
                        let _ = cx.update(|cx| cx.refresh_windows());
                        let _ = cx.update(|cx| Nav::toast(match r {
                            Ok(_) if on => format!("marked {name} watched"),
                            Ok(_) => format!("{name}: unwatched"),
                            Err(e) => e,
                        }, cx));
                    })
                    .detach();
                }),
            ),
            ("couch", "throw a watch party", Box::new(|m, cx| Nav::plan_party(m, cx))),
            (
                "link",
                "copy link",
                Box::new(|m, cx| {
                    let url = couch::commands::share_link(m.kind.clone(), m.id.clone(), m.name.clone());
                    cx.write_to_clipboard(ClipboardItem::new_string(url));
                    Nav::toast("link copied".into(), cx);
                }),
            ),
        ];
        if self.in_cw {
            items.push((
                "close",
                "remove from up next",
                Box::new(|m, cx| {
                    let id = m.id.clone();
                    DISMISSED.with(|d| d.borrow_mut().push(id.clone()));
                    cx.refresh_windows();
                    cx.spawn(async move |cx| {
                        if let Err(e) = api::call::<Value>("cw_dismiss", json!({ "id": id })).await {
                            let _ = cx.update(|cx| Nav::toast(e, cx));
                        }
                    })
                    .detach();
                }),
            ));
        }
        let rows = items.into_iter().enumerate().map(|(i, (ic, label, f))| {
            let m = m.clone();
            div()
                .id(("ctx", i))
                .h(px(42.))
                .px(px(12.))
                .flex()
                .items_center()
                .gap(px(12.))
                .rounded(px(10.))
                .cursor_pointer()
                .hover(|s| s.bg(theme::white(0.1)))
                .on_click(cx.listener(move |me, _, _, cx| {
                    me.close(cx);
                    f(&m, cx);
                }))
                .child(icon(ic, 19.))
                .child(div().flex_1().min_w_0().text_size(px(15.)).font_weight(FontWeight::MEDIUM).truncate().child(label))
        });
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                div()
                    .id("ctx-scrim")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|me, _, _, cx| {
                        cx.stop_propagation();
                        me.close(cx);
                    }))
                    .on_mouse_down(gpui::MouseButton::Right, cx.listener(|me, _, _, cx| {
                        me.close(cx);
                    })),
            )
            .child(
                div()
                    .id("ctx-menu")
                    .absolute()
                    .left(px(x))
                    .top(px(y - 8. * (1. - k)))
                    .opacity(k)
                    .w(px(w))
                    .p(px(6.))
                    .rounded(px(16.))
                    .bg(theme::surface())
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .child(div().px(px(12.)).pt(px(6.)).pb(px(6.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).truncate().child(SharedString::from(m.name.clone())))
                    .children(rows),
            )
            .into_any_element()
    }
}
