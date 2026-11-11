//! Settings: the switches that matter, your addons, and your accounts.
use crate::api;
use crate::theme;
use crate::ui::input::{Submitted, TextInput};
use crate::Nav;
use couch::settings::Settings as S;
use gpui::{div, prelude::*, px, Context, Entity, FontWeight, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

type Pick = fn(&mut S) -> &mut bool;

const SECTIONS: &[(&str, &[(&str, &str, Pick)])] = &[
    (
        "playing",
        &[
            ("couch picks the stream", "the best one your connection can keep up with", |s| &mut s.streams.autoplay_best),
            ("hide cam recordings", "never pick a cinema camera rip", |s| &mut s.streams.hide_cam),
            ("hide 3D releases", "", |s| &mut s.streams.hide_3d),
            ("prefer HDR", "", |s| &mut s.streams.prefer_hdr),
            ("next episode by itself", "", |s| &mut s.playback.autoplay_next),
            ("skip intros", "", |s| &mut s.playback.auto_skip_intro),
            ("skip recaps", "", |s| &mut s.playback.auto_skip_recap),
            ("volume up to 130%", "", |s| &mut s.playback.volume_boost),
            ("smooth motion", "frame interpolation (needs a good GPU)", |s| &mut s.playback.interpolation),
        ],
    ),
    (
        "subtitles",
        &[
            ("on by default", "", |s| &mut s.subtitles.enabled_by_default),
            ("hide [sound descriptions]", "", |s| &mut s.subtitles.hide_sdh),
        ],
    ),
    (
        "couch",
        &[
            ("new episode alerts", "a notification when something you watch is out", |s| &mut s.general.episode_alerts),
            ("big releases on the calendar", "", |s| &mut s.general.calendar_hits),
            ("hide adult titles", "", |s| &mut s.general.hide_adult),
            ("show what I'm watching on the couch", "", |s| &mut s.integrations.crew_share),
            ("gatekeeper", "people ask before joining your couch", |s| &mut s.integrations.crew_gatekeeper),
            ("Discord status", "\"watching …\" on your Discord profile", |s| &mut s.integrations.discord_presence),
        ],
    ),
    (
        "Simkl",
        &[
            ("scrobble what I watch", "", |s| &mut s.integrations.simkl_scrobble),
            ("sync my watchlist", "", |s| &mut s.integrations.simkl_sync_watchlist),
        ],
    ),
];

pub struct Settings {
    addons: Vec<Value>,
    secrets: Value,
    url: Entity<TextInput>,
    scroll: ScrollHandle,
}

impl Settings {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let url = cx.new(|cx| TextInput::new("paste an addon link (…/manifest.json) and press enter", cx));
        cx.subscribe(&url, |s, _, _: &Submitted, cx| s.install(cx)).detach();
        let me = Self { addons: vec![], secrets: Value::Null, url, scroll: ScrollHandle::new() };
        Self::load(cx);
        me
    }
    fn load(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let addons = api::call::<Vec<Value>>("addons_list", json!({})).await.unwrap_or_default();
            let secrets = api::call::<Value>("secrets_status", json!({})).await.unwrap_or_default();
            let _ = this.update(cx, |s, cx| {
                s.addons = addons;
                s.secrets = secrets;
                cx.notify();
            });
        })
        .detach();
    }
    fn install(&mut self, cx: &mut Context<Self>) {
        let url = self.url.read(cx).content.trim().replace("stremio://", "https://");
        if url.is_empty() {
            return;
        }
        self.url.update(cx, |i, cx| i.set_text("", cx));
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("addons_install", json!({ "url": url })).await;
            let _ = this.update(cx, |_, cx| {
                Nav::toast(match &r {
                    Ok(d) => format!("added {}", d["manifest"]["name"].as_str().unwrap_or("the addon")),
                    Err(e) => e.clone(),
                }, cx);
                Self::load(cx);
            });
        })
        .detach();
    }
}

fn toggle(on: bool) -> impl IntoElement {
    div()
        .w(px(48.))
        .h(px(28.))
        .flex_none()
        .rounded_full()
        .p(px(3.))
        .flex()
        .when(on, |d| d.bg(theme::neon()).justify_end())
        .when(!on, |d| d.bg(theme::white(0.18)))
        .child(div().size(px(22.)).rounded_full().bg(theme::text()))
}

fn save(f: Pick) {
    let core = couch::state::get().core.clone();
    let mut s = core.settings();
    let v = f(&mut s);
    *v = !*v;
    let val = serde_json::to_value(&s).unwrap_or_default();
    *core.settings.write() = s;
    couch::rt::spawn(async move {
        let _ = couch::dispatch::invoke("settings_set", json!({ "settings": val })).await;
    });
}

impl Render for Settings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let mut cur = couch::state::get().core.settings();
        let mut page = div()
            .id("settings")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .bg(theme::bg())
            .pt(px(96.))
            .px(px(g))
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(div().flex_none().text_size(px(40.)).font_weight(FontWeight::BLACK).child("settings"));
        for (title, rows) in SECTIONS {
            page = page.child(div().flex_none().mt(px(18.)).text_size(px(22.)).font_weight(FontWeight::BLACK).child(*title)).child(
                div().flex_none().max_w(px(760.)).rounded(px(16.)).bg(theme::white(0.04)).border_1().border_color(theme::white(0.06)).children(rows.iter().enumerate().map(|(i, (label, sub, f))| {
                    let on = *f(&mut cur);
                    let f = *f;
                    div()
                        .id(SharedString::from(format!("set-{title}-{i}")))
                        .flex()
                        .items_center()
                        .gap(px(16.))
                        .px(px(18.))
                        .py(px(14.))
                        .when(i > 0, |d| d.border_t_1().border_color(theme::white(0.06)))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::white(0.03)))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            save(f);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().text_size(px(16.)).font_weight(FontWeight::MEDIUM).child(*label))
                                .children((!sub.is_empty()).then(|| div().text_size(px(13.)).text_color(theme::label3()).child(*sub))),
                        )
                        .child(toggle(on))
                })),
            );
        }

        // ------------------------------------------------------------ addons
        page = page
            .child(div().flex_none().mt(px(18.)).text_size(px(22.)).font_weight(FontWeight::BLACK).child("addons"))
            .child(
                div()
                    .flex_none()
                    .max_w(px(760.))
                    .h(px(50.))
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .rounded(px(14.))
                    .bg(theme::white(0.08))
                    .text_size(px(15.))
                    .child(self.url.clone()),
            )
            .child(div().flex_none().max_w(px(760.)).rounded(px(16.)).bg(theme::white(0.04)).border_1().border_color(theme::white(0.06)).children(self.addons.iter().enumerate().map(|(i, a)| {
                let name = a["manifest"]["name"].as_str().unwrap_or("").to_string();
                let desc = a["manifest"]["description"].as_str().unwrap_or("").to_string();
                let url = a["transportUrl"].as_str().unwrap_or("").to_string();
                let builtin = url.starts_with("couch://") || a["flags"]["protected"].as_bool() == Some(true);
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .px(px(18.))
                    .py(px(14.))
                    .when(i > 0, |d| d.border_t_1().border_color(theme::white(0.06)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().text_size(px(16.)).font_weight(FontWeight::BOLD).child(name.clone()))
                            .child(div().text_size(px(13.)).text_color(theme::label3()).line_clamp(1).text_ellipsis().child(desc)),
                    )
                    .when(!builtin, |d| {
                        d.child(
                            div()
                                .id(SharedString::from(format!("rm-{i}")))
                                .px(px(14.))
                                .h(px(34.))
                                .flex()
                                .items_center()
                                .rounded_full()
                                .bg(theme::white(0.1))
                                .text_size(px(14.))
                                .font_weight(FontWeight::BOLD)
                                .cursor_pointer()
                                .hover(|s| s.bg(gpui::rgb(0x8a1f2c)))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    let url = url.clone();
                                    cx.spawn(async move |this, cx| {
                                        let _ = api::call::<Value>("addons_uninstall", json!({ "transportUrl": url })).await;
                                        let _ = this.update(cx, |_, cx| Self::load(cx));
                                    })
                                    .detach();
                                }))
                                .child("remove"),
                        )
                    })
            })));

        // ---------------------------------------------------------- accounts
        let simkl = self.secrets["simkl_token"].as_bool() == Some(true);
        let stremio = self.secrets["stremio_auth"].as_bool() == Some(true);
        page.child(div().flex_none().mt(px(18.)).text_size(px(22.)).font_weight(FontWeight::BLACK).child("accounts"))
            .child(
                div()
                    .flex_none()
                    .max_w(px(760.))
                    .text_size(px(15.))
                    .text_color(theme::label2())
                    .child(format!(
                        "Simkl: {} · Stremio: {} — the new app uses the same sign-ins as the old one.",
                        if simkl { "connected" } else { "not connected" },
                        if stremio { "connected" } else { "not connected" }
                    )),
            )
            .child(div().h(px(80.)))
    }
}
