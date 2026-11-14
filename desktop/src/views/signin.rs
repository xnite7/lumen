//! Signed out: sign in with Discord (the midnite server is the door).
use crate::host;
use crate::theme;
use crate::ui::icon;
use crate::{Nav, Route};
use futures::StreamExt;
use gpui::{div, prelude::*, px, Context, FontWeight, Window};
use serde_json::json;

pub struct SignIn {
    stage: String,
    error: String,
    /// first time in: one more screen (Simkl, optional), then the couch
    welcome: bool,
    simkl: bool,
    simkl_waiting: bool,
}

impl SignIn {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut stage = host::listen("discord-auth-stage");
        cx.spawn(async move |this, cx| {
            while let Some(v) = stage.next().await {
                let _ = this.update(cx, |s, cx| {
                    s.stage = v.as_str().unwrap_or("").to_string();
                    cx.notify();
                });
            }
        })
        .detach();
        let mut done = host::listen("discord-auth");
        cx.spawn(async move |this, cx| {
            while let Some(v) = done.next().await {
                let ok = v["ok"].as_bool() == Some(true);
                let _ = this.update(cx, |s, cx| {
                    if ok {
                        let db = &couch::state::get().core.db;
                        if db.get::<bool>("gpui_welcomed").unwrap_or(false) {
                            cx.defer(|cx| Nav::go(Route::Home, cx));
                        } else {
                            s.stage.clear();
                            s.welcome = true;
                            s.simkl = db.secret("simkl_v2_access").is_some() || db.secret("simkl_token").is_some();
                        }
                    } else {
                        s.stage.clear();
                        s.error = v["error"].as_str().unwrap_or("that didn't work. try again.").to_string();
                    }
                    cx.notify();
                });
            }
        })
        .detach();
        let mut simkl = host::listen("simkl-auth");
        cx.spawn(async move |this, cx| {
            while let Some(v) = simkl.next().await {
                let ok = v["ok"].as_bool() == Some(true);
                let _ = this.update(cx, |s, cx| {
                    s.simkl_waiting = false;
                    s.simkl = s.simkl || ok;
                    cx.notify();
                });
                if ok {
                    let _ = crate::api::call::<serde_json::Value>("simkl_sync", json!({})).await;
                }
            }
        })
        .detach();
        Self { stage: String::new(), error: String::new(), welcome: false, simkl: false, simkl_waiting: false }
    }
}

impl Render for SignIn {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = !self.stage.is_empty();
        if self.welcome {
            let me: serde_json::Value = couch::state::get().core.db.get("discord_user").unwrap_or_default();
            let name = me["global_name"].as_str().or(me["username"].as_str()).or(me["name"].as_str()).unwrap_or("").to_string();
            let (on, waiting) = (self.simkl, self.simkl_waiting);
            return div()
                .size_full()
                .bg(theme::bg())
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(14.))
                .child(div().text_size(px(48.)).font_weight(FontWeight::BLACK).child(if name.is_empty() { "you're in".to_string() } else { format!("you're in, {name}") }))
                .child(div().text_size(px(17.)).text_color(theme::label2()).child("one thing before the couch. you can skip it."))
                .child(
                    div()
                        .mt(px(14.))
                        .w(px(520.))
                        .p(px(22.))
                        .rounded(px(20.))
                        .bg(theme::white(0.05))
                        .border_1()
                        .border_color(theme::white(0.08))
                        .flex()
                        .items_center()
                        .gap(px(16.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(div().text_size(px(19.)).font_weight(FontWeight::BLACK).child(if on { "Simkl connected" } else { "connect Simkl" }))
                                .child(div().text_size(px(14.)).line_height(px(20.)).text_color(theme::label2()).child(if on {
                                    "what you watch is saved to your account from now on."
                                } else if waiting {
                                    "say yes in your browser, then come back."
                                } else {
                                    "a free account that remembers everything you've watched, your watchlist and your ratings, even on a new PC."
                                })),
                        )
                        .children((!on).then(|| {
                            div()
                                .id("wl-simkl")
                                .flex_none()
                                .px(px(18.))
                                .h(px(40.))
                                .flex()
                                .items_center()
                                .rounded_full()
                                .bg(theme::white(0.12))
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .hover(|s| s.bg(theme::white(0.2)))
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.simkl_waiting = true;
                                    cx.notify();
                                    couch::rt::spawn(async {
                                        let _ = couch::dispatch::invoke("simkl_oauth_start", json!({})).await;
                                    });
                                }))
                                .child(if waiting { "open again" } else { "connect" })
                        }))
                        .children(on.then(|| icon("checkCircle", 26.))),
                )
                .child(
                    div()
                        .id("wl-go")
                        .mt(px(14.))
                        .h(px(54.))
                        .px(px(30.))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .bg(theme::text())
                        .text_color(gpui::rgb(0x000000))
                        .text_size(px(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.9))
                        .on_click(cx.listener(|s, _, _, cx| {
                            let _ = couch::state::get().core.db.set("gpui_welcomed", &true);
                            s.welcome = false;
                            cx.defer(|cx| Nav::go(Route::Home, cx));
                        }))
                        .child(if on { "get on the couch" } else { "skip, get on the couch" }),
                )
                .into_any_element();
        }
        div()
            .size_full()
            .bg(theme::bg())
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(18.))
            .child(div().text_size(px(56.)).font_weight(FontWeight::BLACK).child("midnite™ couch"))
            .child(div().text_size(px(17.)).text_color(theme::label2()).child("for the midnite server. sign in with Discord to get on the couch."))
            .child(
                div()
                    .id("discord-go")
                    .mt(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(54.))
                    .px(px(26.))
                    .rounded(px(16.))
                    .bg(gpui::rgb(0x5865f2))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(18.))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.error.clear();
                        s.stage = "app".into();
                        cx.notify();
                        couch::rt::spawn(async {
                            let _ = couch::dispatch::invoke("discord_start", json!({})).await;
                        });
                    }))
                    .child(icon("discord", 22.))
                    .child(if busy { "check Discord or your browser…" } else { "Sign in with Discord" }),
            )
            .children((!self.error.is_empty()).then(|| div().text_size(px(15.)).text_color(gpui::rgb(0xff8a8a)).child(self.error.clone())))
            .into_any_element()
    }
}
