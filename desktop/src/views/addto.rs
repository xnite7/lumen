//! The + on a title: My List, any of your couch lists, or a new one.
use crate::api::{self, Meta};
use crate::theme;
use crate::ui::{icon, input::{Submitted, TextInput}};
use crate::Nav;
use gpui::{div, prelude::*, px, Context, Entity, FontWeight, SharedString, Window};
use serde_json::{json, Value};

/// couch's own lists, straight from the database
pub fn load() -> Vec<Value> {
    couch::state::get().core.db.get::<Vec<Value>>("couch_lists").unwrap_or_default()
}
pub fn save(lists: &[Value]) {
    let _ = couch::state::get().core.db.set("couch_lists", &Value::Array(lists.to_vec()));
}
fn slim(m: &Meta) -> Value {
    json!({ "id": m.id, "type": m.kind, "name": m.name, "poster": m.poster, "background": m.background, "logo": m.logo, "releaseInfo": m.release_info })
}
fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
pub fn new_list(name: &str, first: Option<&Meta>) -> Option<Value> {
    let n: String = name.trim().chars().take(60).collect();
    if n.is_empty() {
        return None;
    }
    let id: String = (0..8).map(|_| char::from(b"abcdefghijklmnopqrstuvwxyz0123456789"[rand::random::<u8>() as usize % 36])).collect();
    let l = json!({ "id": id, "name": n, "items": first.map(|m| vec![slim(m)]).unwrap_or_default(), "at": now() });
    let mut all = load();
    all.insert(0, l.clone());
    save(&all);
    Some(l)
}

pub struct AddTo {
    pub meta: Option<Meta>,
    in_library: bool,
    lists: Vec<Value>,
    name: Entity<TextInput>,
}

impl AddTo {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| TextInput::new("new list… (type a name, press enter)", cx));
        cx.subscribe(&name, |a, i, _: &Submitted, cx| {
            let n = i.read(cx).content.clone();
            if let Some(m) = a.meta.clone() {
                if new_list(&n, Some(&m)).is_some() {
                    Nav::toast(format!("made {} with {} in it", n.trim(), m.name), cx);
                    a.meta = None;
                }
            }
            a.name.update(cx, |i, cx| i.set_text("", cx));
            cx.notify();
        })
        .detach();
        Self { meta: None, in_library: false, lists: vec![], name }
    }

    pub fn open(&mut self, m: Meta, cx: &mut Context<Self>) {
        self.lists = load();
        self.in_library = false;
        let id = m.id.clone();
        self.meta = Some(m);
        cx.spawn(async move |this, cx| {
            let item = api::call::<Value>("library_item", json!({ "id": id })).await.unwrap_or_default();
            let owned = !item.is_null() && item["removed"].as_bool() != Some(true) && item["temp"].as_bool() != Some(true);
            let _ = this.update(cx, |a, cx| {
                a.in_library = owned;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn toggle_library(&mut self, cx: &mut Context<Self>) {
        let Some(m) = self.meta.clone() else { return };
        let on = !self.in_library;
        self.in_library = on;
        cx.notify();
        let meta = serde_json::to_value(&m).unwrap_or_default();
        cx.spawn(async move |_, cx| {
            let r = if on {
                api::call::<Value>("library_add", json!({ "meta": meta })).await
            } else {
                api::call::<Value>("library_remove", json!({ "id": m.id, "kind": m.kind })).await
            };
            let _ = cx.update(|cx| Nav::toast(match r {
                Err(e) => e,
                Ok(_) if on => format!("added {} to My List", m.name),
                Ok(_) => format!("removed {} from My List", m.name),
            }, cx));
        })
        .detach();
    }

    fn toggle_list(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(m) = self.meta.clone() else { return };
        let Some(l) = self.lists.get_mut(i) else { return };
        let items = l["items"].as_array().cloned().unwrap_or_default();
        let has = items.iter().any(|x| x["id"].as_str() == Some(m.id.as_str()));
        let name = l["name"].as_str().unwrap_or("").to_string();
        l["items"] = if has { Value::Array(items.into_iter().filter(|x| x["id"].as_str() != Some(m.id.as_str())).collect()) } else { Value::Array(std::iter::once(slim(&m)).chain(items).collect()) };
        l["at"] = json!(now());
        save(&self.lists);
        Nav::toast(if has { format!("removed from {name}") } else { format!("added to {name}") }, cx);
        cx.notify();
    }
}

impl Render for AddTo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(m) = self.meta.clone() else { return div().absolute().into_any_element() };
        let row = |id: SharedString, label: String, sub: String, on: bool| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(16.))
                .h(px(54.))
                .rounded(px(12.))
                .cursor_pointer()
                .hover(|s| s.bg(theme::white(0.08)))
                .child(div().size(px(26.)).flex_none().rounded_full().flex().items_center().justify_center().when(on, |d| d.bg(theme::text())).when(!on, |d| d.border_2().border_color(theme::white(0.3))).children(on.then(|| icon("check", 16.).text_color(gpui::rgb(0x000000)))))
                .child(div().flex_1().flex().flex_col().child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(label)).children((!sub.is_empty()).then(|| div().text_size(px(13.)).text_color(theme::label3()).child(sub))))
        };
        let mut rows: Vec<gpui::AnyElement> = vec![row("at-lib".into(), "My List".into(), String::new(), self.in_library).on_click(cx.listener(|a, _, _, cx| a.toggle_library(cx))).into_any_element()];
        for (i, l) in self.lists.iter().enumerate() {
            let n = l["items"].as_array().map_or(0, |a| a.len());
            let on = l["items"].as_array().into_iter().flatten().any(|x| x["id"].as_str() == Some(m.id.as_str()));
            rows.push(row(SharedString::from(format!("at-{i}")), l["name"].as_str().unwrap_or("").to_string(), format!("{n} titles"), on).on_click(cx.listener(move |a, _, _, cx| a.toggle_list(i, cx))).into_any_element());
        }
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div().id("at-scrim").absolute().top_0().left_0().size_full().bg(theme::black(0.55)).on_click(cx.listener(|a, _, _, cx| {
                    a.meta = None;
                    cx.notify();
                })),
            )
            .child(
                div()
                    .id("at-sheet")
                    .relative()
                    .w(px(440.))
                    .max_h(px(560.))
                    .overflow_y_scroll()
                    .p(px(14.))
                    .rounded(px(24.))
                    .bg(theme::surface())
                    .border_1()
                    .border_color(theme::white(0.1))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(div().px(px(16.)).pt(px(6.)).pb(px(8.)).flex().flex_col().child(div().text_size(px(13.)).text_color(theme::label3()).child("add to")).child(div().text_size(px(20.)).font_weight(FontWeight::BLACK).child(m.name.clone())))
                    .children(rows)
                    .child(div().mt(px(6.)).mx(px(8.)).h(px(46.)).px(px(14.)).flex().items_center().gap(px(10.)).rounded(px(12.)).bg(theme::white(0.08)).child(icon("plus", 18.)).child(div().flex_1().child(self.name.clone()))),
            )
            .into_any_element()
    }
}
