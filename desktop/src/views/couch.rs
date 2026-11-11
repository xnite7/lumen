//! The Couch tab: who's watching right now, upcoming watch parties, and the
//! crew.
use crate::api::{self, sized};
use crate::theme;
use crate::Nav;
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, Context, FontWeight, ObjectFit, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};
use std::time::Duration;

pub struct Couch {
    members: Vec<Value>,
    people: Vec<Value>,
    parties: Vec<Value>,
    me: String,
    setup: Option<bool>,
    scroll: ScrollHandle,
}

impl Couch {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let me = Self { members: vec![], people: vec![], parties: vec![], me: String::new(), setup: None, scroll: ScrollHandle::new() };
        // refresh every 20 s while it's around
        cx.spawn(async move |this, cx| loop {
            let crew = api::call::<Value>("crew_list", json!({})).await.unwrap_or(json!({ "setup": false }));
            let people = api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/people", "body": Value::Null })).await.unwrap_or_default();
            let parties = api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/parties", "body": Value::Null })).await.unwrap_or_default();
            let me = api::call::<Value>("discord_me", json!({})).await.unwrap_or_default();
            let ok = this
                .update(cx, |c, cx| {
                    c.setup = Some(crew["setup"].as_bool() == Some(true));
                    c.members = crew["members"].as_array().cloned().unwrap_or_default();
                    c.people = people["people"].as_array().cloned().unwrap_or_default();
                    c.parties = parties["parties"].as_array().cloned().unwrap_or_default();
                    c.me = me["id"].as_str().unwrap_or("").to_string();
                    cx.notify();
                })
                .is_ok();
            if !ok {
                break;
            }
            gpui::Timer::after(Duration::from_secs(20)).await;
        })
        .detach();
        me
    }
}

fn avatar(url: Option<&str>, name: &str, size: f32) -> impl IntoElement {
    div()
        .size(px(size))
        .flex_none()
        .rounded_full()
        .overflow_hidden()
        .bg(theme::neon())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size * 0.42))
        .font_weight(FontWeight::BLACK)
        .child(match url {
            Some(u) => img(u.to_string()).size_full().object_fit(ObjectFit::Cover).into_any_element(),
            None => div().child(name.chars().next().unwrap_or('?').to_uppercase().to_string()).into_any_element(),
        })
}

fn btn(id: SharedString, label: &str, primary: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(16.))
        .h(px(36.))
        .flex()
        .items_center()
        .rounded_full()
        .text_size(px(14.))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .when(primary, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)).hover(|s| s.opacity(0.88)))
        .when(!primary, |d| d.bg(theme::white(0.12)).hover(|s| s.bg(theme::white(0.2))))
        .child(label.to_string())
}

impl Render for Couch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let vw = f32::from(window.viewport_size().width);
        let mut page = div()
            .id("couch")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .bg(theme::bg())
            .pt(px(96.))
            .px(px(g))
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(div().flex_none().text_size(px(40.)).font_weight(FontWeight::BLACK).child("couch"))
            .child(div().flex_none().text_size(px(16.)).text_color(theme::label2()).child("see what your friends from the server are watching right now."));
        match self.setup {
            None => return page.child(div().text_color(theme::label2()).child("finding the remote…")),
            Some(false) => return page.child(div().text_color(theme::label2()).child("sign in with Discord in the old app first — the new one picks it up.")),
            _ => {}
        }

        // ------------------------------------------------------------ live
        let live: Vec<&Value> = self.members.iter().filter(|m| m["watching"]["title"].is_string()).collect();
        if live.is_empty() {
            page = page.child(div().flex_none().py(px(20.)).text_size(px(18.)).text_color(theme::label2()).child("nobody's on the couch."));
        } else {
            let w = ((vw - g * 2. - 14.) / 2.).min(520.);
            page = page.child(div().flex_none().flex().flex_wrap().gap(px(14.)).children(live.into_iter().map(|m| {
                let wt = &m["watching"];
                let art = wt["background"].as_str().or(wt["poster"].as_str()).map(|u| sized(u, "w780"));
                let id = m["id"].as_str().unwrap_or("").to_string();
                let name = m["name"].as_str().unwrap_or("").to_string();
                let frac = match (m["pos"].as_f64(), m["duration"].as_f64()) {
                    (Some(p), Some(d)) if d > 0. => (p / d).clamp(0., 1.) as f32,
                    _ => 0.,
                };
                let line = [wt["title"].as_str().unwrap_or(""), wt["episode"].as_str().unwrap_or("")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
                let to = id.clone();
                let nm = name.clone();
                div()
                    .w(px(w))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .h(px(w * 9. / 16.))
                            .rounded(px(16.))
                            .overflow_hidden()
                            .bg(theme::surface2())
                            .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
                            .child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(180., linear_color_stop(theme::black(0.0), 0.4), linear_color_stop(theme::black(0.8), 1.))))
                            .child(div().absolute().left(px(12.)).top(px(12.)).flex().items_center().gap(px(8.)).child(avatar(m["avatar"].as_str(), &name, 28.)).child(div().font_weight(FontWeight::BOLD).child(name.clone())))
                            .child(div().absolute().right(px(12.)).top(px(12.)).text_size(px(12.)).text_color(theme::label2()).child(if m["paused"].as_bool() == Some(true) { "paused" } else { "" }))
                            .child(
                                div()
                                    .absolute()
                                    .right(px(12.))
                                    .bottom(px(18.))
                                    .child(btn(SharedString::from(format!("join-{id}")), "ask to join", true).on_click(move |_, _, cx| {
                                        let (to, nm) = (to.clone(), nm.clone());
                                        cx.spawn(async move |cx| {
                                            let r = api::call::<Value>("crew_ask", json!({ "to": to })).await;
                                            let _ = cx.update(|cx| Nav::toast(match r {
                                                Ok(_) => format!("asked {nm} — waiting for them to let you in"),
                                                Err(e) => e,
                                            }, cx));
                                        })
                                        .detach();
                                    })),
                            )
                            .child(div().absolute().left(px(12.)).right(px(12.)).bottom(px(8.)).h(px(4.)).rounded_full().bg(theme::white(0.25)).child(div().h_full().rounded_full().w(px((w - 24.) * frac)).bg(theme::lamp()))),
                    )
                    .child(div().text_size(px(16.)).font_weight(FontWeight::BOLD).truncate().child(line))
            })));
        }

        // --------------------------------------------------------- parties
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as f64;
        let parties: Vec<&Value> = self.parties.iter().filter(|p| p["at"].as_f64().unwrap_or(0.) > now - 3. * 3600_000.).collect();
        if !parties.is_empty() {
            page = page.child(div().flex_none().mt(px(10.)).text_size(px(24.)).font_weight(FontWeight::BLACK).child("watch parties")).child(div().flex_none().flex().flex_wrap().gap(px(14.)).children(
                parties.into_iter().map(|p| {
                    let pid = p["id"].as_str().unwrap_or("").to_string();
                    let at = p["at"].as_f64().unwrap_or(0.);
                    let mins = ((at - now) / 60_000.).round() as i64;
                    let when = if mins <= 0 { "on now".to_string() } else if mins < 60 { format!("in {mins} min") } else if mins < 24 * 60 { format!("in {} h", (mins as f64 / 60.).round()) } else { format!("in {} days", (mins as f64 / 1440.).round()) };
                    let mine = p["host"].as_str() == Some(self.me.as_str());
                    let answered = p["rsvp"][self.me.as_str()].clone();
                    let going = p["rsvp"].as_object().map_or(0, |o| o.values().filter(|r| r["yes"].as_bool() == Some(true)).count());
                    let t = &p["title"];
                    let art = t["background"].as_str().or(t["poster"].as_str()).map(|u| sized(u, "w780"));
                    let rsvp = |yes: bool, pid: String| {
                        move |_: &gpui::ClickEvent, _: &mut gpui::Window, cx: &mut gpui::App| {
                            let pid = pid.clone();
                            cx.spawn(async move |cx| {
                                let r = api::call::<Value>("crew_social", json!({ "method": "POST", "path": format!("/v1/party/{pid}/rsvp"), "body": { "yes": yes } })).await;
                                let _ = cx.update(|cx| Nav::toast(if r.is_ok() { if yes { "you're going".into() } else { "maybe next time".into() } } else { r.err().unwrap_or_default() }, cx));
                            })
                            .detach();
                        }
                    };
                    div()
                        .w(px(300.))
                        .rounded(px(16.))
                        .overflow_hidden()
                        .bg(theme::white(0.05))
                        .border_1()
                        .border_color(if mins <= 0 { theme::white(0.0).blend(theme::neon().into()) } else { theme::white(0.08) })
                        .child(
                            div()
                                .relative()
                                .w_full()
                                .h(px(130.))
                                .bg(theme::surface2())
                                .children(art.map(|u| img(u).size_full().object_fit(ObjectFit::Cover)))
                                .child(div().absolute().left(px(10.)).top(px(10.)).px(px(10.)).py(px(3.)).rounded_full().bg(if mins <= 0 { theme::neon().into() } else { theme::black(0.6) }).text_size(px(12.)).font_weight(FontWeight::BLACK).child(when)),
                        )
                        .child(
                            div()
                                .p(px(14.))
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(div().text_size(px(16.)).font_weight(FontWeight::BLACK).truncate().child(t["name"].as_str().unwrap_or("").to_string()))
                                .child(div().text_size(px(12.)).text_color(theme::label2()).child(format!("{} · {going} going", if mine { "your party".to_string() } else { format!("{}'s", p["hostName"].as_str().unwrap_or("")) })))
                                .child(div().flex().gap(px(8.)).mt(px(4.)).map(|d| {
                                    if mine {
                                        d.child(div().text_size(px(13.)).text_color(theme::label2()).child("you're hosting"))
                                    } else if answered.is_null() {
                                        d.child(btn(SharedString::from(format!("pin-{pid}")), "I'm in", true).on_click(rsvp(true, pid.clone())))
                                            .child(btn(SharedString::from(format!("pout-{pid}")), "can't", false).on_click(rsvp(false, pid.clone())))
                                    } else {
                                        d.child(div().text_size(px(13.)).text_color(theme::label2()).child(if answered["yes"].as_bool() == Some(true) { "you're going" } else { "you passed" }))
                                    }
                                })),
                        )
                }),
            ));
        }

        // ------------------------------------------------------------ crew
        if !self.people.is_empty() {
            page = page.child(div().flex_none().mt(px(10.)).text_size(px(24.)).font_weight(FontWeight::BLACK).child("the crew")).child(div().flex_none().flex().flex_wrap().gap(px(18.)).children(
                self.people.iter().map(|p| {
                    let name = p["name"].as_str().unwrap_or("").to_string();
                    let on = self.members.iter().any(|m| m["id"] == p["id"]);
                    div()
                        .w(px(92.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.))
                        .child(div().rounded_full().border_2().border_color(if on { theme::neon().into() } else { theme::white(0.0) }).p(px(2.)).child(avatar(p["avatar"].as_str(), &name, 72.)))
                        .child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).truncate().child(name))
                })
            ));
        }
        page.child(div().h(px(80.)))
    }
}
