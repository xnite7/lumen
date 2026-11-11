//! A studio's page: its logo and what it made (films and shows).
use crate::api::{self, Meta};
use crate::theme;
use crate::ui::card;
use gpui::{div, img, prelude::*, px, Context, FontWeight, ObjectFit, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

pub struct Studio {
    pub id: u64,
    name: String,
    data: Option<Value>,
    kind: &'static str,
    scroll: ScrollHandle,
}

impl Studio {
    pub fn new(id: u64, name: String, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("company_get", json!({ "id": id })).await.ok();
            let _ = this.update(cx, |s, cx| {
                s.data = r;
                cx.notify();
            });
        })
        .detach();
        Self { id, name, data: None, kind: "movies", scroll: ScrollHandle::new() }
    }
}

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let vw = f32::from(window.viewport_size().width);
        let cols = (((vw - g * 2.) + 14.) / (160. + 14.)).floor().max(2.);
        let w = ((vw - g * 2.) - (cols - 1.) * 14.) / cols;
        let d = self.data.clone().unwrap_or_default();
        let list: Vec<Meta> = d[self.kind].as_array().into_iter().flatten().filter_map(|x| serde_json::from_value(x.clone()).ok()).collect();
        let chip = |id: &'static str, label: &str, on: bool| {
            div()
                .id(id)
                .px(px(16.))
                .h(px(38.))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(15.))
                .font_weight(FontWeight::BOLD)
                .cursor_pointer()
                .when(on, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
                .when(!on, |d| d.bg(theme::white(0.1)).hover(|s| s.bg(theme::white(0.18))))
                .child(label.to_string())
        };
        div()
            .id("studio")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .bg(theme::bg())
            .pt(px(100.))
            .px(px(g))
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                div().flex_none().h(px(110.)).flex().items_center().child(match d["logo"].as_str() {
                    Some(u) => div().px(px(22.)).py(px(16.)).rounded(px(18.)).bg(gpui::rgb(0xf4eaf0)).child(img(u.to_string()).h(px(70.)).max_w(px(360.)).object_fit(ObjectFit::Contain)).into_any_element(),
                    None => div().text_size(px(48.)).font_weight(FontWeight::BLACK).child(d["name"].as_str().unwrap_or(&self.name).to_string()).into_any_element(),
                }),
            )
            .children(d["description"].as_str().map(|t| div().flex_none().max_w(px(760.)).text_size(px(16.)).text_color(theme::label2()).child(t.to_string())))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap(px(8.))
                    .child(chip("st-m", "Films", self.kind == "movies").on_click(cx.listener(|s, _, _, cx| {
                        s.kind = "movies";
                        cx.notify();
                    })))
                    .child(chip("st-s", "Shows", self.kind == "shows").on_click(cx.listener(|s, _, _, cx| {
                        s.kind = "shows";
                        cx.notify();
                    }))),
            )
            .child(div().flex_none().flex().flex_wrap().gap(px(14.)).children(list.into_iter().enumerate().map(|(i, m)| {
                let mm = m.clone();
                card::poster(SharedString::from(format!("st-{i}")), &m, w, move |_, _, cx| crate::views::detail::open_rec(&mm, cx))
            })))
            .child(div().h(px(80.)))
    }
}
