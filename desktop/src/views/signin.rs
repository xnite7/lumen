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
                        cx.defer(|cx| Nav::go(Route::Home, cx));
                    } else {
                        s.stage.clear();
                        s.error = v["error"].as_str().unwrap_or("that didn't work. try again.").to_string();
                    }
                    cx.notify();
                });
            }
        })
        .detach();
        Self { stage: String::new(), error: String::new() }
    }
}

impl Render for SignIn {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = !self.stage.is_empty();
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
                    .font_weight(FontWeight::BOLD)
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
    }
}
