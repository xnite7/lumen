//! Settings: switches and choices, Philips Hue, eye tracking, storage,
//! your addons and your accounts.
use crate::api;
use crate::theme;
use crate::ui::input::{Submitted, TextInput};
use crate::Nav;
use gpui::{div, prelude::*, px, AnyElement, Context, Entity, FontWeight, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

/// One row: a switch, or a choice of a few values. Paths are into the
/// settings as JSON ("streams.sort").
enum R {
    Switch(&'static str, &'static str, &'static str),
}
use R::*;

const fn sw(label: &'static str, sub: &'static str, path: &'static str) -> R {
    Switch(label, sub, path)
}

fn sections() -> Vec<(&'static str, Vec<R>, Vec<(&'static str, &'static str, Vec<(Value, &'static str)>)>)> {
    vec![
        (
            "playing",
            vec![
                sw("couch picks the stream", "the best one your connection can keep up with", "streams.autoplayBest"),
                sw("hide cam recordings", "never pick a cinema camera rip", "streams.hideCam"),
                sw("hide 3D releases", "", "streams.hide3d"),
                sw("prefer HDR", "", "streams.preferHdr"),
                sw("prefer cached (debrid)", "", "streams.preferCached"),
                sw("next episode by itself", "", "playback.autoplayNext"),
                sw("skip intros", "", "playback.autoSkipIntro"),
                sw("skip recaps", "", "playback.autoSkipRecap"),
                sw("skip credits", "", "playback.autoSkipCredits"),
                sw("volume up to 130%", "", "playback.volumeBoost"),
            ],
            vec![
                ("sort streams by", "streams.sort", vec![(json!("smart"), "best"), (json!("quality"), "quality"), (json!("size"), "size"), (json!("seeders"), "seeders"), (json!("addon"), "addon")]),
                ("largest file", "streams.maxSizeGb", vec![(json!(0.0), "any"), (json!(5.0), "5 GB"), (json!(10.0), "10 GB"), (json!(20.0), "20 GB"), (json!(40.0), "40 GB")]),
                ("seek step", "playback.seekSmall", vec![(json!(5), "5 s"), (json!(10), "10 s"), (json!(15), "15 s"), (json!(30), "30 s")]),
                ("picture", "playback.aspect", vec![(json!("fit"), "fit"), (json!("fill"), "fill"), (json!("stretch"), "stretch")]),
            ],
        ),
        (
            "video engine",
            vec![
                sw("HDR passthrough", "send HDR straight to an HDR screen", "playback.hdrPassthrough"),
                sw("debanding", "smooths colour bands in dark scenes", "playback.deband"),
                sw("smooth motion", "frame interpolation (needs a good GPU)", "playback.interpolation"),
            ],
            vec![
                ("hardware decoding", "playback.hwdec", vec![(json!("auto-safe"), "auto"), (json!("nvdec"), "NVDEC"), (json!("d3d11va"), "D3D11"), (json!("no"), "off")]),
                ("renderer", "playback.vo", vec![(json!("gpu-next"), "gpu-next"), (json!("gpu"), "gpu")]),
            ],
        ),
        (
            "subtitles",
            vec![sw("on by default", "", "subtitles.enabledByDefault"), sw("hide [sound descriptions]", "", "subtitles.hideSdh")],
            vec![
                ("size", "subtitles.size", vec![(json!(0.8), "small"), (json!(1.0), "medium"), (json!(1.25), "large"), (json!(1.5), "huge")]),
                ("weight", "subtitles.weight", vec![(json!(400), "regular"), (json!(600), "semibold"), (json!(750), "bold")]),
                ("colour", "subtitles.color", vec![(json!("#ffffff"), "white"), (json!("#ffe14a"), "yellow"), (json!("#bfe9ff"), "ice")]),
            ],
        ),
        (
            "couch",
            vec![
                sw("previews on hover", "posters play a clip when you point at them", "general.hoverPreviews"),
                sw("billboard clips", "the big title on Home plays a clip", "general.heroTrailers"),
                sw("top 10 numbers", "", "general.topTenRanks"),
                sw("new episode alerts", "a notification when something you watch is out", "general.episodeAlerts"),
                sw("big releases on the calendar", "", "general.calendarHits"),
                sw("hide adult titles", "", "general.hideAdult"),
                sw("show what I'm watching on the couch", "", "integrations.crewShare"),
                sw("gatekeeper", "people ask before joining your couch", "integrations.crewGatekeeper"),
                sw("Discord status", "\"watching …\" on your Discord profile", "integrations.discordPresence"),
                sw("experiments", "eye tracking and other things still being tried", "general.experimental"),
            ],
            vec![],
        ),
        (
            "Simkl",
            vec![
                sw("scrobble what I watch", "", "integrations.simklScrobble"),
                sw("sync my watchlist", "", "integrations.simklSyncWatchlist"),
                sw("ticks on Simkl count as watched here", "", "integrations.simklMirrorWatched"),
            ],
            vec![],
        ),
        (
            "network",
            vec![sw("private DNS", "look up addresses over HTTPS (gets past blocks)", "network.doh")],
            vec![("DNS provider", "network.dohProvider", vec![(json!("cloudflare"), "Cloudflare"), (json!("google"), "Google"), (json!("quad9"), "Quad9")])],
        ),
    ]
}

fn get<'a>(v: &'a Value, path: &str) -> &'a Value {
    path.split('.').fold(v, |v, k| &v[k])
}

/// change one setting (by path) and save
fn set(path: &str, val: Value) {
    let core = couch::state::get().core.clone();
    let mut v = serde_json::to_value(core.settings()).unwrap_or_default();
    let mut cur = &mut v;
    let keys: Vec<&str> = path.split('.').collect();
    for k in &keys[..keys.len() - 1] {
        cur = &mut cur[*k];
    }
    cur[keys[keys.len() - 1]] = val;
    if let Ok(s) = serde_json::from_value::<couch::settings::Settings>(v.clone()) {
        *core.settings.write() = s;
    }
    couch::rt::spawn(async move {
        let _ = couch::dispatch::invoke("settings_set", json!({ "settings": v })).await;
    });
}

pub struct Settings {
    addons: Vec<Value>,
    secrets: Value,
    url: Entity<TextInput>,
    hue_ip: Entity<TextInput>,
    bridges: Vec<Value>,
    hue: Value,
    hue_busy: bool,
    testing: bool,
    cache: Value,
    tobii: Value,
    scroll: ScrollHandle,
}

impl Settings {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let url = cx.new(|cx| TextInput::new("paste an addon link (…/manifest.json) and press enter", cx));
        cx.subscribe(&url, |s, _, _: &Submitted, cx| s.install(cx)).detach();
        let hue_ip = cx.new(|cx| TextInput::new("or the bridge's address, e.g. 192.168.1.20, and press enter", cx));
        cx.subscribe(&hue_ip, |s, i, _: &Submitted, cx| {
            let ip = i.read(cx).content.trim().to_string();
            if !ip.is_empty() {
                s.pair(ip, cx);
            }
        })
        .detach();
        let me = Self { addons: vec![], secrets: Value::Null, url, hue_ip, bridges: vec![], hue: Value::Null, hue_busy: false, testing: false, cache: Value::Null, tobii: Value::Null, scroll: ScrollHandle::new() };
        Self::load(cx);
        me
    }
    fn load(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let addons = api::call::<Vec<Value>>("addons_list", json!({})).await.unwrap_or_default();
            let secrets = api::call::<Value>("secrets_status", json!({})).await.unwrap_or_default();
            let cache = api::call::<Value>("cache_info", json!({})).await.unwrap_or_default();
            let tobii = api::call::<Value>("tobii_status", json!({})).await.unwrap_or_default();
            let paired = secrets["hue_username"].as_bool() == Some(true);
            let _ = this.update(cx, |s, cx| {
                s.addons = addons;
                s.secrets = secrets;
                s.cache = cache;
                s.tobii = tobii;
                cx.notify();
            });
            if paired {
                let hue = api::call::<Value>("hue_overview", json!({})).await.unwrap_or_default();
                let _ = this.update(cx, |s, cx| {
                    s.hue = hue;
                    cx.notify();
                });
            }
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
    fn discover(&mut self, cx: &mut Context<Self>) {
        self.hue_busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = api::call::<Vec<Value>>("hue_discover", json!({})).await;
            let _ = this.update(cx, |s, cx| {
                s.hue_busy = false;
                match r {
                    Ok(b) if !b.is_empty() => s.bridges = b,
                    Ok(_) => Nav::toast("no bridge found. type its address instead".into(), cx),
                    Err(e) => Nav::toast(e, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn pair(&mut self, ip: String, cx: &mut Context<Self>) {
        self.hue_busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("hue_pair", json!({ "ip": ip })).await;
            let _ = this.update(cx, |s, cx| {
                s.hue_busy = false;
                match r {
                    Ok(_) => {
                        Nav::toast("paired with your Hue bridge".into(), cx);
                        set("hue.enabled", json!(true));
                        Self::load(cx);
                    }
                    Err(e) => Nav::toast(e, cx),
                }
                cx.notify();
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

fn group() -> gpui::Div {
    div().flex_none().max_w(px(760.)).rounded(px(16.)).bg(theme::white(0.04)).border_1().border_color(theme::white(0.06))
}
fn heading(t: &str) -> gpui::Div {
    div().flex_none().mt(px(18.)).text_size(px(22.)).font_weight(FontWeight::BLACK).child(t.to_string())
}
fn line(i: usize) -> gpui::Div {
    div().flex().items_center().gap(px(16.)).px(px(18.)).py(px(14.)).when(i > 0, |d| d.border_t_1().border_color(theme::white(0.06)))
}
fn label(t: &str, sub: &str) -> gpui::Div {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(div().text_size(px(16.)).font_weight(FontWeight::MEDIUM).child(t.to_string()))
        .children((!sub.is_empty()).then(|| div().text_size(px(13.)).text_color(theme::label3()).child(sub.to_string())))
}
fn small_btn(id: SharedString, t: &str, strong: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .px(px(14.))
        .h(px(34.))
        .flex()
        .items_center()
        .rounded_full()
        .when(strong, |d| d.bg(theme::neon()))
        .when(!strong, |d| d.bg(theme::white(0.1)))
        .text_size(px(14.))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .hover(|s| s.opacity(0.85))
        .child(t.to_string())
}

impl Settings {
    fn switch_row(&self, key: String, i: usize, t: &'static str, sub: &str, path: &'static str, cur: &Value, cx: &mut Context<Self>) -> AnyElement {
        let on = get(cur, path).as_bool().unwrap_or(false);
        line(i)
            .id(SharedString::from(key))
            .cursor_pointer()
            .hover(|s| s.bg(theme::white(0.03)))
            .on_click(cx.listener(move |_, _, _, cx| {
                set(path, json!(!on));
                cx.notify();
            }))
            .child(label(t, sub))
            .child(toggle(on))
            .into_any_element()
    }
    fn choice_row(&self, key: String, i: usize, t: &'static str, path: &'static str, opts: Vec<(Value, String)>, cur: &Value, cx: &mut Context<Self>) -> AnyElement {
        let now = get(cur, path).clone();
        let same = |a: &Value, b: &Value| match (a.as_f64(), b.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() < 1e-6,
            _ => a == b,
        };
        line(i)
            .child(label(t, ""))
            .child(div().flex().flex_wrap().gap(px(4.)).p(px(3.)).rounded_full().bg(theme::white(0.08)).children(opts.into_iter().enumerate().map(|(k, (v, l))| {
                let on = same(&v, &now);
                div()
                    .id(SharedString::from(format!("{key}-{k}")))
                    .px(px(12.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .when(on, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
                    .when(!on, |d| d.hover(|s| s.bg(theme::white(0.1))))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        set(path, v.clone());
                        cx.notify();
                    }))
                    .child(l)
            })))
            .into_any_element()
    }
}

impl Render for Settings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let cur = serde_json::to_value(couch::state::get().core.settings()).unwrap_or_default();
        let mut page = div()
            .id("settings")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "settings"))
            .bg(theme::bg())
            .pt(px(96.))
            .px(px(g))
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(div().flex_none().text_size(px(40.)).font_weight(FontWeight::BLACK).child("settings"));
        for (title, rows, chs) in sections() {
            let mut el: Vec<AnyElement> = vec![];
            let mut i = 0;
            for r in rows {
                let Switch(t, sub, path) = r;
                el.push(self.switch_row(format!("set-{title}-{i}"), i, t, sub, path, &cur, cx));
                i += 1;
            }
            for (t, path, opts) in chs {
                let opts = opts.into_iter().map(|(v, l)| (v, l.to_string())).collect();
                el.push(self.choice_row(format!("ch-{title}-{i}"), i, t, path, opts, &cur, cx));
                i += 1;
            }
            page = page.child(heading(title)).child(group().children(el));
        }

        // ------------------------------------------------------------- text
        let mode = crate::theme::text_mode();
        page = page.child(heading("text")).child(group().child(
            line(0).child(label("how text is drawn", "try each — screens differ")).child(div().flex().gap(px(4.)).p(px(3.)).rounded_full().bg(theme::white(0.08)).children([("auto", "auto"), ("sharp", "sharp"), ("smooth", "smooth")].into_iter().map(|(k, l)| {
                let on = mode == k;
                div()
                    .id(SharedString::from(format!("tm-{k}")))
                    .px(px(12.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .when(on, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
                    .when(!on, |d| d.hover(|s| s.bg(theme::white(0.1))))
                    .on_click(cx.listener(move |_, _, window, cx| {
                        let _ = couch::state::get().core.db.set("gpui_text_mode", &k.to_string());
                        crate::theme::apply_text_mode(cx);
                        window.refresh();
                        cx.notify();
                    }))
                    .child(l)
            }))),
        ));

        // ------------------------------------------------------------ Hue
        let paired = self.secrets["hue_username"].as_bool() == Some(true);
        let mut hue: Vec<AnyElement> = vec![];
        if !paired {
            hue.push(
                line(0)
                    .child(label("find your bridge", "press the round button on the bridge, then pick it here within 30 seconds"))
                    .child(small_btn("hue-find".into(), if self.hue_busy { "searching…" } else { "search" }, false).on_click(cx.listener(|s, _, _, cx| s.discover(cx))))
                    .into_any_element(),
            );
            for (k, b) in self.bridges.iter().enumerate() {
                let ip = b["ip"].as_str().unwrap_or("").to_string();
                hue.push(
                    line(1)
                        .child(label(b["name"].as_str().unwrap_or("Hue Bridge"), &ip))
                        .child(small_btn(SharedString::from(format!("hue-pair-{k}")), "pair", true).on_click(cx.listener(move |s, _, _, cx| s.pair(ip.clone(), cx))))
                        .into_any_element(),
                );
            }
            hue.push(line(1).child(div().flex_1().h(px(40.)).px(px(12.)).flex().items_center().rounded(px(10.)).bg(theme::white(0.08)).child(self.hue_ip.clone())).into_any_element());
        } else {
            hue.push(self.switch_row("hue-on".into(), 0, "use Hue", "", "hue.enabled", &cur, cx));
            hue.push(self.choice_row("hue-mode".into(), 1, "during playback", "hue.playbackMode", vec![(json!("off"), "nothing".to_string()), (json!("dim"), "couch mode".to_string()), (json!("ambilight"), "match the picture".to_string())], &cur, cx));
            let groups: Vec<(Value, String)> = self.hue["groups"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|g| (g["id"].clone(), g["name"].as_str().unwrap_or("room").to_string()))
                .collect();
            if !groups.is_empty() {
                hue.push(self.choice_row("hue-room".into(), 2, "room", "hue.roomId", groups, &cur, cx));
            }
            let mode = get(&cur, "hue.playbackMode").as_str().unwrap_or("off").to_string();
            if mode == "ambilight" {
                let areas: Vec<(Value, String)> = self.hue["areas"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|a| (a["id"].clone(), format!("{} ({} lights)", a["name"].as_str().unwrap_or("area"), a["channels"].as_u64().unwrap_or(0))))
                    .collect();
                if !areas.is_empty() {
                    hue.push(self.choice_row("hue-area".into(), 3, "entertainment area", "hue.areaId", areas, &cur, cx));
                }
                hue.push(self.choice_row("hue-bri".into(), 4, "brightness", "hue.syncBrightness", vec![(json!(40.0), "40%".to_string()), (json!(60.0), "60%".to_string()), (json!(80.0), "80%".to_string()), (json!(100.0), "100%".to_string())], &cur, cx));
                let testing = self.testing;
                hue.push(
                    line(5)
                        .child(label("test", "the lights follow a few colours"))
                        .child(small_btn("hue-test".into(), if testing { "stop" } else { "start sync" }, testing).on_click(cx.listener(move |s, _, _, cx| {
                            s.testing = !s.testing;
                            let on = s.testing;
                            cx.spawn(async move |_, _| {
                                let _ = api::call::<Value>("hue_test", json!({ "on": on })).await;
                            })
                            .detach();
                            cx.notify();
                        })))
                        .into_any_element(),
                );
            } else if mode == "dim" {
                hue.push(self.choice_row("hue-dim".into(), 4, "dim to", "hue.dimBrightness", vec![(json!(0.0), "off".to_string()), (json!(10.0), "10%".to_string()), (json!(25.0), "25%".to_string()), (json!(40.0), "40%".to_string())], &cur, cx));
            }
            hue.push(self.switch_row("hue-restore".into(), 6, "restore lights afterwards", "", "hue.restoreAfter", &cur, cx));
            hue.push(self.switch_row("hue-mood".into(), 7, "room follows the artwork", "titles tint the room while you browse", "hue.artworkMood", &cur, cx));
        }
        page = page.child(heading("Philips Hue")).child(group().children(hue));

        // ---------------------------------------------------- eye tracking
        if get(&cur, "general.experimental").as_bool() == Some(true) {
            let status = self.tobii["status"].as_str().or(self.tobii["message"].as_str()).unwrap_or("").to_string();
            let tob: Vec<AnyElement> = vec![
                self.switch_row("tb-on".into(), 0, "Tobii eye tracker", &status, "tobii.enabled", &cur, cx),
                self.switch_row("tb-away".into(), 1, "pause when I look away", "", "tobii.presencePause", &cur, cx),
                self.switch_row("tb-gaze".into(), 2, "look to control", "rest your eyes on a button to press it", "tobii.gazeControls", &cur, cx),
                self.switch_row("tb-par".into(), 3, "head-tracked depth", "the picture shifts as you move your head", "tobii.headParallax", &cur, cx),
                self.switch_row("tb-3d".into(), 4, "3D releases in depth", "", "tobii.depth3d", &cur, cx),
                self.switch_row("tb-lights".into(), 5, "lights dim when I'm away", "", "tobii.awayDimsLights", &cur, cx),
            ];
            page = page.child(heading("eye tracking")).child(group().children(tob));
        }

        // ---------------------------------------------------------- storage
        let mb = |k: &str| self.cache[k].as_f64().map(|b| format!("{:.0} MB", b / 1_048_576.)).unwrap_or_default();
        let total = ["images", "torrents", "thumbs", "bytes", "total"].iter().map(|k| mb(k)).find(|s| !s.is_empty()).unwrap_or_default();
        page = page.child(heading("storage")).child(
            group().child(
                line(0)
                    .child(label("cache", &total))
                    .child(small_btn("cache-clear".into(), "clear", false).on_click(cx.listener(|_, _, _, cx| {
                        cx.spawn(async move |this, cx| {
                            let _ = api::call::<Value>("cache_clear", json!({})).await;
                            let _ = this.update(cx, |_, cx| {
                                Nav::toast("cache cleared".into(), cx);
                                Self::load(cx);
                            });
                        })
                        .detach();
                    }))),
            ),
        );

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
