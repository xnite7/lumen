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
            crate::timer::after(Duration::from_secs(20)).await;
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
        .when(url.is_none(), |d| d.bg(theme::neon()))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size * 0.42))
        .font_weight(FontWeight::BLACK)
        .child(match url {
            Some(u) => img(crate::net::face(u, size)).size_full().into_any_element(),
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
        .font_weight(FontWeight::SEMIBOLD)
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
            .child(crate::ui::smooth::wheel(&self.scroll, "couch"))
            .bg(theme::bg())
            .pt(px(96.))
            .px(px(g))
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(40.)).font_weight(FontWeight::BLACK).child("couch"))
                    .children(crate::views::wrapped::season_year().map(|y| btn("wrapped-open".into(), &format!("your {y} wrapped ✨"), true).on_click(|_, _, cx| Nav::wrapped(cx)))),
            )
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
            let w = ((vw - g * 2. - 28.) / 3.).clamp(260., 380.);
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
                let title = crate::api::Meta { id: wt["metaId"].as_str().unwrap_or("").into(), kind: wt["type"].as_str().unwrap_or("").into(), name: wt["title"].as_str().unwrap_or("").into(), poster: wt["poster"].as_str().map(String::from), background: wt["background"].as_str().map(String::from), logo: wt["logo"].as_str().map(String::from), ..Default::default() };
                let (pre, open) = (title.clone(), title);
                let bw = w * 0.6;
                div()
                    .w(px(w))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        // the picture opens the title's page; "ask to join" asks to join
                        div()
                            .id(SharedString::from(format!("live-{id}")))
                            .cursor_pointer()
                            .on_hover(move |on, _, cx| {
                                if *on {
                                    crate::views::detail::prefetch(&pre, cx);
                                }
                            })
                            .on_click(move |_, _, cx| Nav::detail(&open, cx))
                            .relative()
                            .w_full()
                            .h(px(w * 9. / 16.))
                            .rounded(px(16.))
                            .overflow_hidden()
                            .bg(theme::surface2())
                            .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
                            .child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(180., linear_color_stop(theme::black(0.0), 0.4), linear_color_stop(theme::black(0.8), 1.))))
                            .child(div().absolute().left(px(12.)).top(px(12.)).flex().items_center().gap(px(8.)).child(avatar(m["avatar"].as_str(), &name, 28.)).child(div().font_weight(FontWeight::SEMIBOLD).child(name.clone())))
                            .child(div().absolute().right(px(12.)).top(px(12.)).text_size(px(12.)).text_color(theme::label2()).child(if m["paused"].as_bool() == Some(true) { "paused" } else { "" }))
                            .child(
                                div()
                                    .absolute()
                                    .right(px(12.))
                                    .bottom(px(18.))
                                    .child(btn(SharedString::from(format!("join-{id}")), "ask to join", true).on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        let (to, nm) = (to.clone(), nm.clone());
                                        crate::together::get(cx).update(cx, |t, cx| t.ask_to_join(to, nm, cx));
                                    })),
                            )
                    )
                    // under the picture, like continue watching: square, white, 60% wide
                    .child(div().w(px(w)).flex().justify_center().child(div().w(px(bw)).h(px(4.)).bg(theme::white(0.18)).child(div().h_full().w(px(bw * frac)).bg(gpui::white()))))
                    .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).truncate().child(line))
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
                    let pid = p["id"].as_str().unwrap_or("").to_string();
                    // nights in a row the two of you watched together (yours only, nobody else sees it)
                    let streak = crate::views::crewx::together_streak(&pid);
                    let group = SharedString::from(format!("crew-g-{pid}"));
                    div()
                        .id(SharedString::from(format!("crew-{pid}")))
                        .group(group.clone())
                        .cursor_pointer()
                        .on_click(move |_, _, cx| Nav::go(crate::Route::Member(pid.clone()), cx))
                        .w(px(92.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .relative()
                                .child(div().rounded_full().when(!on, |d| d.opacity(0.45)).child(avatar(p["avatar"].as_str(), &name, 72.)))
                                .when(streak >= 2, |d| {
                                    d.child(
                                        div()
                                            .absolute()
                                            .right(px(-6.))
                                            .bottom(px(-4.))
                                            .h(px(26.))
                                            .px(px(7.))
                                            .rounded_full()
                                            .bg(theme::black(0.85))
                                            .flex()
                                            .items_center()
                                            .gap(px(2.))
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::BLACK)
                                            .child(crate::ui::emoji::text("🔥", 13.))
                                            .child(div().text_color(theme::lamp()).child(streak.to_string())),
                                    )
                                }),
                        )
                        // the name; on hover with a streak, what the fire means
                        .child(
                            div()
                                .relative()
                                .w_full()
                                .flex()
                                .justify_center()
                                .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).truncate().when(streak >= 2, |d| d.group_hover(group.clone(), |s| s.opacity(0.))).child(name))
                                .when(streak >= 2, |d| {
                                    d.child(
                                        div()
                                            .absolute()
                                            .top(px(-2.))
                                            .w(px(150.))
                                            .flex()
                                            .justify_center()
                                            .opacity(0.)
                                            .group_hover(group, |s| s.opacity(1.))
                                            .child(div().px(px(8.)).py(px(3.)).rounded(px(8.)).bg(theme::black(0.85)).text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::lamp()).child(format!("{streak}-night streak together"))),
                                    )
                                }),
                        )
                })
            ));
        }
        page.child(div().h(px(80.)))
    }
}
