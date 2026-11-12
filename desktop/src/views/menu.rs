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
}

impl Menu {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { open: None }
    }
    pub fn show(&mut self, m: Meta, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.open = Some((m, at));
        cx.notify();
    }
}

impl Render for Menu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((m, at)) = self.open.clone() else { return div().absolute().into_any_element() };
        let vs = window.viewport_size();
        let (w, h) = (250., 300.);
        let x = f32::from(at.x).min(f32::from(vs.width) - w - 12.);
        let y = f32::from(at.y).min(f32::from(vs.height) - h - 12.);
        let items: Vec<(&'static str, &'static str, Box<dyn Fn(&Meta, &mut gpui::App)>)> = vec![
            ("play", "play", Box::new(|m, cx| Nav::play(m, cx))),
            ("info", "open", Box::new(|m, cx| Nav::detail(m, cx))),
            ("plus", "add to…", Box::new(|m, cx| Nav::add(m, cx))),
            (
                "eye",
                "mark watched",
                Box::new(|m, cx| {
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
                        let r = api::call::<Value>("library_set_watched", json!({ "meta": full, "videoId": Value::Null, "watched": true, "allVideos": all })).await;
                        let _ = cx.update(|cx| Nav::toast(match r {
                            Ok(_) => format!("marked {name} watched"),
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
                    me.open = None;
                    cx.notify();
                    f(&m, cx);
                }))
                .child(icon(ic, 19.))
                .child(div().text_size(px(15.)).font_weight(FontWeight::MEDIUM).child(label))
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
                    .on_click(cx.listener(|me, _, _, cx| {
                        me.open = None;
                        cx.notify();
                    }))
                    .on_mouse_down(gpui::MouseButton::Right, cx.listener(|me, _, _, cx| {
                        me.open = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("ctx-menu")
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .p(px(6.))
                    .rounded(px(16.))
                    .bg(theme::surface())
                    .border_1()
                    .border_color(theme::white(0.12))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .child(div().px(px(12.)).pt(px(6.)).pb(px(6.)).text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(theme::label3()).truncate().child(SharedString::from(m.name.clone())))
                    .children(rows),
            )
            .into_any_element()
    }
}
