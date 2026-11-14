//! Settings: a list of sections down the left (with a search box that finds
//! any setting by name), the chosen section on the right.
use crate::api;
use crate::theme;
use crate::ui::icon;
use crate::ui::input::{Changed, Submitted, TextInput};
use crate::Nav;
use gpui::{div, prelude::*, px, AnyElement, Context, Entity, FontWeight, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Playing,
    Video,
    Subtitles,
    Couch,
    Simkl,
    Lights,
    Network,
    Storage,
    About,
}

const TABS: [(Tab, &str, &str, &str); 9] = [
    (Tab::Playing, "playing", "play", "which stream couch picks, and what plays by itself"),
    (Tab::Video, "picture", "tv", "how the video is decoded and drawn"),
    (Tab::Subtitles, "subtitles", "subtitles", "how they look"),
    (Tab::Couch, "couch", "couch", "home, previews, alerts and what your crew sees"),
    (Tab::Simkl, "Simkl", "checkCircle", "keep what you've watched, everywhere"),
    (Tab::Lights, "lights", "bulb", "Philips Hue: the room dims or follows the picture"),
    (Tab::Network, "network", "globe", "getting past blocks"),
    (Tab::Storage, "storage", "storage", "what couch keeps on this PC"),
    (Tab::About, "about", "info", "this couch, your account"),
];

/// One plain row: a switch, or a choice of a few values. Paths are into the
/// settings as JSON ("streams.sort").
enum R {
    Switch(&'static str, &'static str, &'static str),
    Choice(&'static str, &'static str, Vec<(Value, &'static str)>),
}
use R::*;

fn rows(tab: Tab) -> Vec<R> {
    match tab {
        Tab::Playing => vec![
            Switch("couch picks the stream", "the best one your connection can keep up with", "streams.autoplayBest"),
            Switch("next episode by itself", "", "playback.autoplayNext"),
            Switch("skip intros", "", "playback.autoSkipIntro"),
            Switch("skip recaps", "", "playback.autoSkipRecap"),
            Switch("skip credits", "", "playback.autoSkipCredits"),
            Switch("hide cam recordings", "never pick a cinema camera rip", "streams.hideCam"),
            Switch("hide 3D releases", "", "streams.hide3d"),
            Switch("prefer HDR", "", "streams.preferHdr"),
            Switch("prefer instant streams", "ones that start right away", "streams.preferCached"),
            Switch("volume up to 130%", "", "playback.volumeBoost"),
            Choice("sort streams by", "streams.sort", vec![(json!("smart"), "best"), (json!("quality"), "quality"), (json!("size"), "size"), (json!("seeders"), "seeders")]),
            Choice("largest file", "streams.maxSizeGb", vec![(json!(0.0), "any"), (json!(5.0), "5 GB"), (json!(10.0), "10 GB"), (json!(20.0), "20 GB"), (json!(40.0), "40 GB")]),
        ],
        Tab::Video => vec![
            Choice("picture", "playback.aspect", vec![(json!("fit"), "fit"), (json!("fill"), "fill"), (json!("stretch"), "stretch")]),
            Switch("HDR passthrough", "send HDR straight to an HDR screen", "playback.hdrPassthrough"),
            Switch("debanding", "smooths colour bands in dark scenes", "playback.deband"),
            Switch("smooth motion", "frame interpolation (needs a good GPU)", "playback.interpolation"),
            Choice("hardware decoding", "playback.hwdec", vec![(json!("auto-safe"), "auto"), (json!("nvdec"), "NVDEC"), (json!("d3d11va"), "D3D11"), (json!("no"), "off")]),
            Choice("renderer", "playback.vo", vec![(json!("gpu-next"), "gpu-next"), (json!("gpu"), "gpu")]),
        ],
        Tab::Subtitles => vec![
            Switch("on by default", "", "subtitles.enabledByDefault"),
            Switch("hide [sound descriptions]", "", "subtitles.hideSdh"),
            Choice("size", "subtitles.size", vec![(json!(0.8), "small"), (json!(1.0), "medium"), (json!(1.25), "large"), (json!(1.5), "huge")]),
            Choice("weight", "subtitles.weight", vec![(json!(400), "regular"), (json!(600), "semibold"), (json!(750), "bold")]),
            Choice("colour", "subtitles.color", vec![(json!("#ffffff"), "white"), (json!("#ffe14a"), "yellow"), (json!("#bfe9ff"), "ice")]),
        ],
        Tab::Couch => vec![
            Switch("previews on hover", "posters play a clip when you point at them", "general.hoverPreviews"),
            Switch("billboard clips", "the big title on Home plays a clip", "general.heroTrailers"),
            Switch("top 10 numbers", "", "general.topTenRanks"),
            Switch("new episode alerts", "a notification when something you watch is out", "general.episodeAlerts"),
            Switch("big releases on the calendar", "", "general.calendarHits"),
            Switch("hide adult titles", "", "general.hideAdult"),
            Switch("show what I'm watching on the couch", "", "integrations.crewShare"),
            Switch("gatekeeper", "people ask before joining your couch", "integrations.crewGatekeeper"),
            Switch("Discord status", "\"watching …\" on your Discord profile", "integrations.discordPresence"),
        ],
        Tab::Simkl => vec![
            Switch("mark what I watch on Simkl", "as you finish things", "integrations.simklScrobble"),
            Switch("keep my watchlist in sync", "", "integrations.simklSyncWatchlist"),
            Switch("ticks on Simkl count as watched here", "", "integrations.simklMirrorWatched"),
        ],
        Tab::Network => vec![
            Switch("private DNS", "look up addresses over HTTPS (gets past blocks)", "network.doh"),
            Choice("DNS provider", "network.dohProvider", vec![(json!("cloudflare"), "Cloudflare"), (json!("google"), "Google"), (json!("quad9"), "Quad9")]),
        ],
        _ => vec![],
    }
}

/// words that find the sections that aren't plain rows
fn keywords(tab: Tab) -> &'static str {
    match tab {
        Tab::Simkl => "simkl connect account watch history watchlist sync ratings",
        Tab::Lights => "philips hue lights bridge room ambilight dim couch mode",
        Tab::Storage => "storage cache clear space",
        Tab::About => "about version update text smooth sharp sign out discord account remove uninstall",
        _ => "",
    }
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
    tab: Tab,
    find: Entity<TextInput>,
    secrets: Value,
    hue_ip: Entity<TextInput>,
    bridges: Vec<Value>,
    hue: Value,
    hue_busy: bool,
    testing: bool,
    cache: Value,
    scroll: ScrollHandle,
    /// "remove couch" / "disconnect" ask once more before they go
    confirm_remove: bool,
    confirm_simkl: bool,
    confirm_out: bool,
    /// the browser is open on Simkl's page
    simkl_waiting: bool,
}

impl Settings {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let find = cx.new(|cx| TextInput::new("find a setting", cx));
        cx.subscribe(&find, |s, _, _: &Changed, cx| {
            s.scroll.set_offset(gpui::point(px(0.), px(0.)));
            cx.notify();
        })
        .detach();
        let hue_ip = cx.new(|cx| TextInput::new("or the bridge's address, e.g. 192.168.1.20, and press enter", cx));
        cx.subscribe(&hue_ip, |s, i, _: &Submitted, cx| {
            let ip = i.read(cx).content.trim().to_string();
            if !ip.is_empty() {
                s.pair(ip, cx);
            }
        })
        .detach();
        // Simkl's page sends the browser back to couch: that's this
        let mut auth = crate::host::listen("simkl-auth");
        cx.spawn(async move |this, cx| {
            use futures::StreamExt;
            while let Some(v) = auth.next().await {
                let ok = v["ok"].as_bool() == Some(true);
                let Ok(()) = this.update(cx, |s, cx| {
                    s.simkl_waiting = false;
                    Nav::toast(if ok { "Simkl connected".to_string() } else { v["error"].as_str().unwrap_or("Simkl sign-in didn't finish").to_string() }, cx);
                    Self::load(cx);
                    cx.notify();
                }) else {
                    break;
                };
                if ok {
                    let _ = api::call::<Value>("simkl_sync", json!({})).await;
                }
            }
        })
        .detach();
        let me = Self {
            tab: Tab::Playing,
            find,
            secrets: Value::Null,
            hue_ip,
            bridges: vec![],
            hue: Value::Null,
            hue_busy: false,
            testing: false,
            cache: Value::Null,
            scroll: ScrollHandle::new(),
            confirm_remove: false,
            confirm_simkl: false,
            confirm_out: false,
            simkl_waiting: false,
        };
        Self::load(cx);
        me
    }
    fn load(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let secrets = api::call::<Value>("secrets_status", json!({})).await.unwrap_or_default();
            let cache = api::call::<Value>("cache_info", json!({})).await.unwrap_or_default();
            let paired = secrets["hue_username"].as_bool() == Some(true);
            let _ = this.update(cx, |s, cx| {
                s.secrets = secrets;
                s.cache = cache;
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
    fn go(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        self.confirm_remove = false;
        self.confirm_simkl = false;
        self.confirm_out = false;
        self.find.update(cx, |i, cx| i.set_text("", cx));
        self.scroll.set_offset(gpui::point(px(0.), px(0.)));
        cx.notify();
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
        .w(px(46.))
        .h(px(28.))
        .flex_none()
        .rounded_full()
        .p(px(3.))
        .flex()
        .when(on, |d| d.bg(theme::neon()).justify_end())
        .when(!on, |d| d.bg(theme::white(0.16)))
        .child(div().size(px(22.)).rounded_full().bg(theme::text()))
}

/// a card of rows, hairlines between them
fn group(rows: Vec<AnyElement>) -> gpui::Div {
    let n = rows.len();
    div().flex_none().rounded(px(18.)).bg(theme::white(0.045)).border_1().border_color(theme::white(0.06)).overflow_hidden().children(rows.into_iter().enumerate().map(move |(i, r)| {
        div().flex().flex_col().child(r).when(i + 1 < n, |d| d.child(div().mx(px(18.)).h(px(1.)).bg(theme::white(0.06))))
    }))
}
fn line(_i: usize) -> gpui::Div {
    div().flex().items_center().gap(px(16.)).px(px(18.)).min_h(px(58.)).py(px(10.))
}
fn label(t: &str, sub: &str) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
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
        .px(px(16.))
        .h(px(36.))
        .flex()
        .items_center()
        .rounded_full()
        .when(strong, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
        .when(!strong, |d| d.bg(theme::white(0.1)))
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|s| s.opacity(0.85))
        .child(t.to_string())
}
fn caption(t: &str) -> gpui::Div {
    div().flex_none().mt(px(10.)).px(px(4.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child(t.to_string())
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
                    .font_weight(FontWeight::SEMIBOLD)
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

    fn plain(&self, tab: Tab, filter: &str, cur: &Value, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let name = TABS.iter().find(|t| t.0 == tab).map(|t| t.1).unwrap_or("");
        rows(tab)
            .into_iter()
            .enumerate()
            .filter_map(|(i, r)| match r {
                Switch(t, sub, path) => (filter.is_empty() || t.to_lowercase().contains(filter) || sub.to_lowercase().contains(filter)).then(|| self.switch_row(format!("set-{name}-{i}"), i, t, sub, path, cur, cx)),
                Choice(t, path, opts) => (filter.is_empty() || t.to_lowercase().contains(filter) || opts.iter().any(|o| o.1.to_lowercase().contains(filter))).then(|| {
                    let opts = opts.into_iter().map(|(v, l)| (v, l.to_string())).collect();
                    self.choice_row(format!("ch-{name}-{i}"), i, t, path, opts, cur, cx)
                }),
            })
            .collect()
    }

    fn simkl(&self, cur: &Value, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let on = self.secrets["simkl_token"].as_bool() == Some(true);
        let who = self.secrets["simklUser"]["user"]["name"].as_str().unwrap_or("").to_string();
        let waiting = self.simkl_waiting;
        let sure = self.confirm_simkl;
        let status = if on {
            if who.is_empty() { "connected".to_string() } else { format!("connected as {who}") }
        } else if waiting {
            "waiting for you to say yes in your browser…".to_string()
        } else {
            "not connected".to_string()
        };
        let head = div()
            .flex_none()
            .p(px(22.))
            .rounded(px(18.))
            .bg(theme::white(0.045))
            .border_1()
            .border_color(if on { theme::white(0.14) } else { theme::white(0.06) })
            .flex()
            .items_center()
            .gap(px(16.))
            .child(div().size(px(52.)).flex_none().rounded(px(14.)).bg(theme::white(0.08)).flex().items_center().justify_center().child(icon(if on { "checkCircle" } else { "link" }, 26.)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(20.)).font_weight(FontWeight::BLACK).child("Simkl"))
                    .child(div().text_size(px(14.)).text_color(theme::label2()).child(status)),
            )
            .child(if on {
                small_btn("simkl-off".into(), if sure { "sure? disconnect" } else { "disconnect" }, false)
                    .on_click(cx.listener(|s, _, _, cx| {
                        if !s.confirm_simkl {
                            s.confirm_simkl = true;
                            cx.notify();
                            return;
                        }
                        s.confirm_simkl = false;
                        cx.spawn(async move |this, cx| {
                            let _ = api::call::<Value>("simkl_logout", json!({})).await;
                            let _ = this.update(cx, |_, cx| {
                                Nav::toast("Simkl disconnected".into(), cx);
                                Self::load(cx);
                            });
                        })
                        .detach();
                    }))
                    .into_any_element()
            } else {
                small_btn("simkl-on".into(), if waiting { "open it again" } else { "connect" }, true)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.simkl_waiting = true;
                        cx.notify();
                        cx.spawn(async move |this, cx| {
                            if let Err(e) = api::call::<Value>("simkl_oauth_start", json!({})).await {
                                let _ = this.update(cx, |s, cx| {
                                    s.simkl_waiting = false;
                                    Nav::toast(e, cx);
                                    cx.notify();
                                });
                            }
                        })
                        .detach();
                    }))
                    .into_any_element()
            })
            .into_any_element();
        let why = [
            ("refresh", "your history is never lost", "everything you finish is saved to your Simkl account. new PC, reinstall, a wiped drive: connect again and your watched ticks and progress come back."),
            ("bookmark", "one watchlist everywhere", "add something on couch and it's on Simkl's phone app and website too, and the other way round."),
            ("star", "your ratings stay yours", "what you rate on couch is kept on your Simkl profile, not just on this PC."),
            ("tv", "it counts what you watch elsewhere", "Simkl can track Netflix, Crunchyroll and others. those ticks show up here, so couch knows what you've already seen."),
            ("sparkles", "it's free", "a free Simkl account is all you need. couch only reads and updates your lists and history."),
        ];
        let mut out = vec![head, caption(if on { "what you're getting" } else { "why connect" }).into_any_element()];
        out.push(
            group(
                why.into_iter()
                    .map(|(ic, t, sub)| {
                        div()
                            .flex()
                            .items_start()
                            .gap(px(14.))
                            .px(px(18.))
                            .py(px(14.))
                            .child(div().mt(px(2.)).child(icon(ic, 20.)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(t))
                                    .child(div().text_size(px(14.)).line_height(px(20.)).text_color(theme::label2()).child(sub)),
                            )
                            .into_any_element()
                    })
                    .collect(),
            )
            .into_any_element(),
        );
        if on {
            out.push(caption("what couch does with it").into_any_element());
            out.push(group(self.plain(Tab::Simkl, "", cur, cx)).into_any_element());
        }
        out
    }

    fn lights(&self, cur: &Value, cx: &mut Context<Self>) -> Vec<AnyElement> {
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
        }
        vec![group(hue).into_any_element()]
    }

    fn storage(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mb = |k: &str| self.cache[k].as_f64().map(|b| format!("{:.0} MB", b / 1_048_576.)).unwrap_or_default();
        let total = ["images", "torrents", "thumbs", "bytes", "total"].iter().map(|k| mb(k)).find(|s| !s.is_empty()).unwrap_or_default();
        vec![group(vec![line(0)
            .child(label("cache", &if total.is_empty() { "pictures and bits of video couch keeps to be quick".to_string() } else { format!("{total} · pictures and bits of video couch keeps to be quick") }))
            .child(small_btn("cache-clear".into(), "clear", false).on_click(cx.listener(|_, _, _, cx| {
                cx.spawn(async move |this, cx| {
                    let _ = api::call::<Value>("cache_clear", json!({})).await;
                    let _ = this.update(cx, |_, cx| {
                        Nav::toast("cache cleared".into(), cx);
                        Self::load(cx);
                    });
                })
                .detach();
            })))
            .into_any_element()])
        .into_any_element()]
    }

    fn about(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let me = self.secrets["discordUser"].clone();
        let name = me["global_name"].as_str().or(me["username"].as_str()).or(me["name"].as_str()).unwrap_or("").to_string();
        let (sure_out, sure_rm) = (self.confirm_out, self.confirm_remove);
        let mode = crate::theme::text_mode();
        let text = line(0)
            .child(label("how text is drawn", "smooth is softest. its edges settle after a restart"))
            .child(div().flex().gap(px(4.)).p(px(3.)).rounded_full().bg(theme::white(0.08)).children([("auto", "auto"), ("sharp", "sharp"), ("smooth", "smooth")].into_iter().map(|(k, l)| {
                let on = mode == k;
                div()
                    .id(SharedString::from(format!("tm-{k}")))
                    .px(px(12.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
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
            })))
            .into_any_element();
        vec![
            group(vec![
                line(0).child(label("midnite™ couch", &format!("version {}", crate::update::VERSION))).child(div().text_size(px(13.)).text_color(theme::label3()).child("updates install themselves")).into_any_element(),
                text,
            ])
            .into_any_element(),
            caption("account").into_any_element(),
            group(vec![line(0)
                .child(label(if name.is_empty() { "Discord" } else { &name }, "signed in with Discord"))
                .child(small_btn("sign-out".into(), if sure_out { "sure? sign out" } else { "sign out" }, false).on_click(cx.listener(|s, _, _, cx| {
                    if !s.confirm_out {
                        s.confirm_out = true;
                        cx.notify();
                        return;
                    }
                    s.confirm_out = false;
                    cx.spawn(async move |_, cx| {
                        let _ = api::call::<Value>("discord_logout", json!({})).await;
                        let _ = cx.update(|cx| Nav::go(crate::Route::SignIn, cx));
                    })
                    .detach();
                })))
                .into_any_element()])
            .into_any_element(),
            caption("this PC").into_any_element(),
            group(vec![line(0)
                .child(label("remove couch", if sure_rm { "sure? click again. your lists, ratings and sign-ins are kept" } else { "takes the app off this PC" }))
                .child(small_btn("remove-couch".into(), if sure_rm { "remove it" } else { "remove" }, false).on_click(cx.listener(|s, _, _, cx| {
                    if !s.confirm_remove {
                        s.confirm_remove = true;
                        cx.notify();
                        return;
                    }
                    crate::uninstall::run();
                    cx.quit();
                })))
                .into_any_element()])
            .into_any_element(),
        ]
    }
}

impl Render for Settings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let cur = serde_json::to_value(couch::state::get().core.settings()).unwrap_or_default();
        let q = self.find.read(cx).content.trim().to_lowercase();
        let tab = self.tab;

        // ---- the section list
        let side = div()
            .flex_none()
            .w(px(250.))
            .h_full()
            .pt(px(96.))
            .pb(px(28.))
            .pl(px(g))
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(div().flex_none().mb(px(14.)).text_size(px(34.)).font_weight(FontWeight::BLACK).child("settings"))
            .child(
                div()
                    .flex_none()
                    .mb(px(12.))
                    .h(px(42.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded_full()
                    .bg(theme::white(0.07))
                    .border_1()
                    .border_color(theme::white(0.06))
                    .text_size(px(15.))
                    .child(icon("search", 17.).text_color(theme::label3()))
                    .child(div().flex_1().min_w_0().child(self.find.clone())),
            )
            .children(TABS.iter().enumerate().map(|(i, (t, name, ic, _))| {
                let on = q.is_empty() && *t == tab;
                let t = *t;
                div()
                    .id(("set-tab", i))
                    .flex_none()
                    .h(px(42.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .rounded(px(12.))
                    .cursor_pointer()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if on { theme::text() } else { theme::label2() })
                    .when(on, |d| d.bg(theme::white(0.09)))
                    .hover(|s| s.bg(theme::white(0.06)))
                    .on_click(cx.listener(move |s, _, _, cx| s.go(t, cx)))
                    .child(icon(ic, 19.).text_color(if on { theme::text() } else { theme::label3() }))
                    .child(name.to_string())
            }))
            .child(div().flex_1())
            .child(div().flex_none().px(px(12.)).text_size(px(13.)).text_color(theme::label3()).child(format!("couch {}", crate::update::VERSION)));

        // ---- what's chosen (or what was found)
        let mut body: Vec<AnyElement> = vec![];
        if q.is_empty() {
            let (_, name, _, about) = TABS.iter().find(|t| t.0 == tab).copied().unwrap_or(TABS[0]);
            body.push(
                div()
                    .flex_none()
                    .mb(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(26.)).font_weight(FontWeight::BLACK).child(name))
                    .child(div().text_size(px(15.)).text_color(theme::label2()).child(about))
                    .into_any_element(),
            );
            match tab {
                Tab::Simkl => body.extend(self.simkl(&cur, cx)),
                Tab::Lights => body.extend(self.lights(&cur, cx)),
                Tab::Storage => body.extend(self.storage(cx)),
                Tab::About => body.extend(self.about(cx)),
                t => body.push(group(self.plain(t, "", &cur, cx)).into_any_element()),
            }
        } else {
            let mut any = false;
            for (i, (t, name, ic, about)) in TABS.iter().enumerate() {
                let found = if *t == Tab::Simkl && self.secrets["simkl_token"].as_bool() != Some(true) { vec![] } else { self.plain(*t, &q, &cur, cx) };
                let section = name.to_lowercase().contains(&q) || keywords(*t).contains(q.as_str());
                if found.is_empty() && !section {
                    continue;
                }
                any = true;
                body.push(caption(name).into_any_element());
                let none = found.is_empty();
                if !none {
                    body.push(group(found).into_any_element());
                }
                if section && (none || !keywords(*t).is_empty()) {
                    let t = *t;
                    body.push(
                        group(vec![line(0)
                            .id(("set-goto", i))
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::white(0.03)))
                            .on_click(cx.listener(move |s, _, _, cx| s.go(t, cx)))
                            .child(icon(ic, 20.))
                            .child(label(&format!("open {name}"), about))
                            .child(icon("chevron", 16.).text_color(theme::label3()))
                            .into_any_element()])
                        .into_any_element(),
                    );
                }
            }
            if !any {
                body.push(div().flex_none().pt(px(40.)).text_size(px(17.)).text_color(theme::label2()).child(format!("nothing called \"{q}\"")).into_any_element());
            }
        }
        let page = div()
            .id("settings")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "settings"))
            .pt(px(104.))
            .pl(px(44.))
            .pr(px(g))
            .child(div().max_w(px(760.)).flex().flex_col().gap(px(12.)).children(body).child(div().flex_none().h(px(80.))));
        div().size_full().flex().bg(theme::bg()).child(side).child(page)
    }
}
