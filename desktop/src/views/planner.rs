//! "Throw a watch party": pick when, pick who, add a line. The little
//! ticket on the left is what they'll get.
use crate::api::{self, sized, Meta};
use crate::theme;
use crate::ui::input::TextInput;
use crate::Nav;
use chrono::{Datelike, Duration as CD, Local, TimeZone, Timelike};
use gpui::{div, img, prelude::*, px, Context, Entity, FontWeight, ObjectFit, SharedString, Window};
use serde_json::{json, Value};

pub struct Planner {
    pub meta: Option<Meta>,
    at: i64,
    everyone: bool,
    picked: Vec<String>,
    people: Vec<Value>,
    note: Entity<TextInput>,
    sending: bool,
}

fn slot(days: i64, hour: u32) -> chrono::DateTime<Local> {
    let d = Local::now() + CD::days(days);
    Local.with_ymd_and_hms(d.year(), d.month(), d.day(), hour, 0, 0).single().unwrap_or(d)
}
fn next_dow(dow: u32, hour: u32) -> chrono::DateTime<Local> {
    let today = Local::now().weekday().num_days_from_sunday();
    let add = ((dow + 7 - today) % 7) as i64;
    slot(if add == 0 { 7 } else { add }, hour)
}
fn picks() -> Vec<(&'static str, i64)> {
    let tonight = slot(0, 21);
    let first = if tonight.timestamp_millis() > Local::now().timestamp_millis() + 15 * 60_000 {
        ("tonight 9pm", tonight.timestamp_millis())
    } else {
        let t = Local::now() + CD::hours(1);
        let t = t.with_minute((t.minute() / 15) * 15).unwrap_or(t).with_second(0).unwrap_or(t);
        ("in an hour", t.timestamp_millis())
    };
    vec![first, ("tomorrow 9pm", slot(1, 21).timestamp_millis()), ("friday 10pm", next_dow(5, 22).timestamp_millis()), ("saturday 9pm", next_dow(6, 21).timestamp_millis())]
}

impl Planner {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let note = cx.new(|cx| TextInput::new("bring snacks. no phones.", cx));
        Self { meta: None, at: picks()[0].1, everyone: true, picked: vec![], people: vec![], note, sending: false }
    }

    pub fn open(&mut self, m: Meta, cx: &mut Context<Self>) {
        self.meta = Some(m);
        self.at = picks()[0].1;
        self.everyone = true;
        self.picked.clear();
        self.sending = false;
        self.note.update(cx, |n, cx| n.set_text("", cx));
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/people", "body": Value::Null })).await.unwrap_or_default();
            let me = couch::state::get().core.db.get::<Value>("discord_user").and_then(|u| u["id"].as_str().map(|s| s.to_string())).unwrap_or_default();
            let _ = this.update(cx, |p, cx| {
                p.people = r["people"].as_array().cloned().unwrap_or_default().into_iter().filter(|x| x["id"].as_str() != Some(me.as_str())).collect();
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn send(&mut self, cx: &mut Context<Self>) {
        let Some(m) = self.meta.clone() else { return };
        if self.sending {
            return;
        }
        self.sending = true;
        let invite: Value = if self.everyone { json!("all") } else { json!(self.picked) };
        let note = self.note.read(cx).content.trim().to_string();
        let title = json!({ "id": m.id, "type": m.kind, "name": m.name, "poster": m.poster, "background": m.background, "logo": m.logo, "year": m.year() });
        let at = self.at;
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("crew_social", json!({ "method": "POST", "path": "/v1/party", "body": { "title": title, "at": at, "invite": invite, "note": note } })).await;
            let _ = this.update(cx, |p, cx| {
                p.sending = false;
                match r {
                    Ok(_) => {
                        p.meta = None;
                        let when = Local.timestamp_millis_opt(at).single().map(|t| t.format("%a %-I:%M %p").to_string().to_lowercase()).unwrap_or_default();
                        cx.defer(move |cx| Nav::toast(format!("invites sent. {when} it is"), cx));
                    }
                    Err(e) => cx.defer(move |cx| Nav::toast(e, cx)),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn chip(id: SharedString, label: String, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(14.))
        .h(px(36.))
        .flex()
        .items_center()
        .rounded_full()
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .when(on, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
        .when(!on, |d| d.bg(theme::white(0.08)).hover(|s| s.bg(theme::white(0.16))))
        .child(label)
}

impl Render for Planner {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(m) = self.meta.clone() else { return div().into_any_element() };
        let when = Local.timestamp_millis_opt(self.at).single();
        let note = self.note.read(cx).content.trim().to_string();
        let ink = gpui::rgb(0x3b1029);
        let valid = self.everyone || !self.picked.is_empty();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(div().id("plan-scrim").absolute().top_0().left_0().size_full().bg(theme::black(0.6)).on_click(cx.listener(|p, _, _, cx| {
                p.meta = None;
                cx.notify();
            })))
            .child(
                div()
                    .id("plan-sheet")
                    .relative()
                    .w(px(760.))
                    .p(px(22.))
                    .rounded(px(26.))
                    .bg(gpui::rgba(0x0c090bf5))
                    .border_1()
                    .border_color(theme::white(0.1))
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(div().text_size(px(22.)).font_weight(FontWeight::BLACK).child("throw a watch party"))
                    .child(
                        div()
                            .flex()
                            .gap(px(22.))
                            // the ticket they'll get
                            .child(
                                div()
                                    .w(px(210.))
                                    .flex_none()
                                    .rounded(px(8.))
                                    .overflow_hidden()
                                    .bg(gpui::rgb(0xf6ecd8))
                                    .text_color(ink)
                                    .children(m.background.clone().or(m.poster.clone()).map(|u| img(sized(&u, "w780")).w_full().h(px(130.)).object_fit(ObjectFit::Cover)))
                                    .child(
                                        div()
                                            .p(px(12.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(div().text_size(px(9.)).font_weight(FontWeight::BLACK).text_color(gpui::rgb(0xc4206f)).child("ADMIT ONE"))
                                            .child(div().text_size(px(18.)).line_height(px(19.)).font_weight(FontWeight::BLACK).child(m.name.to_uppercase()))
                                            .children(when.map(|t| div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).child(t.format("%a %b %-d · %-I:%M %p").to_string().to_uppercase())))
                                            .when(!note.is_empty(), |d| d.child(div().text_size(px(12.)).italic().text_color(gpui::rgb(0x2a2a55)).child(format!("“{note}”")))),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(10.))
                                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label2()).child("when"))
                                    .child(div().flex().flex_wrap().gap(px(8.)).children(picks().into_iter().map(|(label, t)| {
                                        chip(SharedString::from(format!("pk-{label}")), label.to_string(), self.at == t).on_click(cx.listener(move |p, _, _, cx| {
                                            p.at = t;
                                            cx.notify();
                                        }))
                                    })))
                                    .child(div().mt(px(6.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label2()).child("who"))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap(px(8.))
                                            .child(chip("pp-all".into(), "everyone".into(), self.everyone).on_click(cx.listener(|p, _, _, cx| {
                                                p.everyone = true;
                                                p.picked.clear();
                                                cx.notify();
                                            })))
                                            .children(self.people.iter().map(|x| {
                                                let id = x["id"].as_str().unwrap_or("").to_string();
                                                let on = !self.everyone && self.picked.contains(&id);
                                                chip(SharedString::from(format!("pp-{id}")), x["name"].as_str().unwrap_or("").to_string(), on).on_click(cx.listener(move |p, _, _, cx| {
                                                    p.everyone = false;
                                                    if let Some(i) = p.picked.iter().position(|y| *y == id) {
                                                        p.picked.remove(i);
                                                    } else {
                                                        p.picked.push(id.clone());
                                                    }
                                                    cx.notify();
                                                }))
                                            })),
                                    )
                                    .child(div().mt(px(6.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label2()).child("a line on the ticket (optional)"))
                                    .child(div().h(px(42.)).px(px(12.)).flex().items_center().rounded(px(12.)).bg(theme::white(0.08)).text_size(px(15.)).child(self.note.clone())),
                            ),
                    )
                    .child(
                        div()
                            .id("plan-send")
                            .h(px(50.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(gpui::rgb(0xd9257f))
                            .font_weight(FontWeight::BLACK)
                            .text_size(px(16.))
                            .cursor_pointer()
                            .when(!valid || self.sending, |d| d.opacity(0.45))
                            .on_click(cx.listener(move |p, _, _, cx| {
                                if valid {
                                    p.send(cx)
                                }
                            }))
                            .child(if self.sending { "sending…" } else { "send the invites" }),
                    ),
            )
            .into_any_element()
    }
}
