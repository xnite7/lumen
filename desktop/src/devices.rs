//! Your own devices, Spotify-style: each couch (this PC, a Mac, an iPhone)
//! says what it's playing every few seconds; the others show "playing on
//! Sultan's iPhone" with play/pause, and "play here" moves the film over at
//! the same spot (the same stream when this device can play it).
use crate::api::{self, Meta};
use crate::playback;
use crate::theme;
use crate::ui::{icon, icon_fill};
use gpui::{div, img, prelude::*, px, App, FontWeight, ObjectFit};
use serde_json::{json, Value};

/// another of your devices that's playing something, as last heard
#[derive(Clone, Debug)]
pub struct Other {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub playing: Value,
}

static OTHERS: parking_lot::Mutex<Vec<Other>> = parking_lot::Mutex::new(Vec::new());
/// a press on the bar that's on its way (shown at once, before the device answers)
static PENDING: parking_lot::Mutex<Option<(String, bool, std::time::Instant)>> = parking_lot::Mutex::new(None);

fn my_id() -> String {
    let db = &couch::state::get().core.db;
    if let Some(id) = db.get::<String>("device_id") {
        return id;
    }
    let id: String = (0..16).map(|_| format!("{:x}", rand_nibble())).collect();
    let _ = db.set("device_id", &id);
    id
}

fn rand_nibble() -> u8 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
    (h.finish() & 0xf) as u8
}

/// what this device is called ("Sultan's MacBook Pro", the PC's name)
fn my_name() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(o) = std::process::Command::new("/usr/sbin/scutil").args(["--get", "ComputerName"]).output() {
        let n = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !n.is_empty() {
            return n;
        }
    }
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).map(|n| {
        // Windows shouts its names: DESKTOP-M8CE2FE → Desktop-m8ce2fe
        let low = n.to_lowercase();
        let mut c = low.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    }).unwrap_or_else(|_| "couch".into())
}

const KIND: &str = if cfg!(target_os = "macos") { "mac" } else { "pc" };

pub fn start(cx: &mut App) {
    let (id, name) = (my_id(), my_name());
    cx.spawn(async move |cx| loop {
        let busy = OTHERS.lock().iter().any(|o| !o.playing.is_null());
        crate::timer::after(std::time::Duration::from_millis(if busy { 2000 } else { 4000 })).await;
        let playing = cx.update(|cx| {
            let p = playback::get(cx).read(cx);
            if !p.open || p.meta.is_none() {
                return Value::Null;
            }
            let m = p.meta.clone().unwrap_or_default();
            let parts: Vec<&str> = p.video_id.split(':').collect();
            let episode = if parts.len() == 3 {
                let t = if p.episode_title.is_empty() { String::new() } else { format!(" · {}", p.episode_title) };
                format!("S{} E{}{t}", parts[1], parts[2])
            } else {
                String::new()
            };
            json!({
                "metaId": m.id.split(':').next().unwrap_or(""), "type": m.kind, "title": m.name, "episode": episode,
                "videoId": p.video_id, "poster": m.poster, "background": m.background, "logo": m.logo,
                "pos": p.state.pos, "duration": p.state.duration, "paused": p.state.paused,
                "stream": p.stream.as_ref().map(playback::host_tag),
            })
        });
        let body = json!({ "id": id, "name": name, "kind": KIND, "playing": playing });
        let Ok(r) = api::call::<Value>("crew_social", json!({ "method": "PUT", "path": "/v1/me/device", "body": body })).await else { continue };
        let others: Vec<Other> = r["devices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| d["me"] != true)
            .map(|d| Other { id: d["id"].as_str().unwrap_or("").into(), name: d["name"].as_str().unwrap_or("couch").into(), kind: d["kind"].as_str().unwrap_or("pc").into(), playing: d["playing"].clone() })
            .collect();
        let changed = {
            let mut o = OTHERS.lock();
            let ch = o.iter().map(|x| (&x.id, &x.playing)).ne(others.iter().map(|x| (&x.id, &x.playing)));
            *o = others;
            ch
        };
        let cmds: Vec<Value> = r["cmds"].as_array().cloned().unwrap_or_default();
        let _ = cx.update(|cx| {
            for c in cmds {
                apply(&c, cx);
            }
            if changed {
                cx.refresh_windows();
            }
        });
    })
    .detach();
}

/// what another device asked this one to do
fn apply(c: &Value, cx: &mut App) {
    let pb = playback::get(cx);
    if !pb.read(cx).open {
        return;
    }
    match c["cmd"].as_str().unwrap_or("") {
        "pause" => pb.read(cx).cmd("pause", json!({ "value": true })),
        "play" => pb.read(cx).cmd("pause", json!({ "value": false })),
        "seek" => {
            if let Some(p) = c["pos"].as_f64() {
                pb.read(cx).cmd("seek", json!({ "to": p }));
            }
        }
        "next" => pb.update(cx, |p, cx| p.play_next(cx)),
        // moved to another device: this one lets go (where it got to is saved)
        "stop" => {
            pb.update(cx, |p, cx| p.close(cx));
            crate::Nav::toast("carrying on on your other device".into(), cx);
        }
        _ => {}
    }
}

fn send(dev: &str, cmd: &str, pos: Option<f64>) {
    let (dev, cmd) = (dev.to_string(), cmd.to_string());
    couch::rt::spawn(async move {
        let _ = couch::dispatch::invoke("crew_social", json!({ "method": "POST", "path": format!("/v1/me/device/{dev}/cmd"), "body": { "cmd": cmd, "pos": pos } })).await;
    });
}

/// Play it here instead, at the same spot (the same stream if this device can).
pub fn play_here(o: &Other, cx: &mut App) {
    let p = &o.playing;
    let meta = Meta {
        id: p["metaId"].as_str().unwrap_or("").into(),
        kind: p["type"].as_str().unwrap_or("movie").into(),
        name: p["title"].as_str().unwrap_or("").into(),
        poster: p["poster"].as_str().map(String::from),
        background: p["background"].as_str().map(String::from),
        logo: p["logo"].as_str().map(String::from),
        ..Default::default()
    };
    let vid = p["videoId"].as_str().filter(|v| v.split(':').count() == 3).map(String::from);
    let pos = p["pos"].as_f64().filter(|x| *x > 1.);
    let prefer = p.get("stream").filter(|s| !s.is_null()).cloned();
    send(&o.id, "stop", None);
    OTHERS.lock().retain(|x| x.id != o.id);
    playback::quick_play_with(meta, vid, pos, prefer, cx);
}

/// the bar at the bottom: another of your devices is playing (and this one isn't)
pub fn bar(cx: &mut App) -> Option<gpui::AnyElement> {
    if playback::get(cx).read(cx).open {
        return None;
    }
    let o = OTHERS.lock().iter().find(|o| !o.playing.is_null()).cloned()?;
    let p = o.playing.clone();
    let mut paused = p["paused"].as_bool().unwrap_or(false);
    // a press just made: show it straight away
    if let Some((d, want, at)) = PENDING.lock().clone() {
        if d == o.id && at.elapsed().as_secs() < 6 {
            paused = want;
        }
    }
    let frac = match (p["pos"].as_f64(), p["duration"].as_f64()) {
        (Some(a), Some(b)) if b > 0. => (a / b).clamp(0., 1.) as f32,
        _ => 0.,
    };
    let icon_kind = match o.kind.as_str() {
        "iphone" | "ipad" => "phone",
        "mac" => "tv",
        _ => "tv",
    };
    let title = p["title"].as_str().unwrap_or("").to_string();
    let sub = p["episode"].as_str().filter(|s| !s.is_empty()).map(String::from);
    let art = p["background"].as_str().or(p["poster"].as_str()).map(|u| crate::net::fit(&api::sized(u, "w300"), 120.));
    let (o1, o2) = (o.clone(), o.clone());
    Some(
        div()
            .absolute()
            .bottom(px(20.))
            .left_0()
            .w_full()
            .flex()
            .justify_center()
            .child(
                div()
                    .id("device-bar")
                    .occlude()
                    .w(px(560.))
                    .h(px(64.))
                    .rounded(px(20.))
                    .bg(gpui::rgba(0x161216e8))
                    .border_1()
                    .border_color(theme::white(0.10))
                    .shadow(vec![gpui::BoxShadow { color: theme::black(0.5), offset: gpui::point(px(0.), px(12.)), blur_radius: px(30.), spread_radius: px(0.), inset: false }])
                    .overflow_hidden()
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .pl(px(8.))
                    .pr(px(10.))
                    .children(art.map(|u| img(u).w(px(84.)).h(px(48.)).rounded(px(12.)).object_fit(ObjectFit::Cover)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().flex().items_center().gap(px(6.)).text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(gpui::rgb(0x7ee08a)).child(icon(icon_kind, 14.).text_color(gpui::rgb(0x7ee08a))).child(format!("playing on {}", o.name)))
                            .child(div().text_size(px(15.)).font_weight(FontWeight::BOLD).truncate().child(match &sub {
                                Some(s) => format!("{title} · {s}"),
                                None => title.clone(),
                            })),
                    )
                    .child(
                        div()
                            .id("device-toggle")
                            .size(px(40.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::white(0.12)))
                            .on_click(move |_, _, cx| {
                                send(&o1.id, if paused { "play" } else { "pause" }, None);
                                *PENDING.lock() = Some((o1.id.clone(), !paused, std::time::Instant::now()));
                                cx.refresh_windows();
                            })
                            .child(icon_fill(if paused { "play" } else { "pause" }, 22.)),
                    )
                    .child(
                        div()
                            .id("device-here")
                            .h(px(40.))
                            .px(px(16.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .cursor_pointer()
                            .bg(theme::text())
                            .text_color(gpui::rgb(0x000000))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(14.))
                            .hover(|s| s.opacity(0.88))
                            .on_click(move |_, _, cx| play_here(&o2, cx))
                            .child(icon_fill("play", 16.).text_color(gpui::rgb(0x000000)))
                            .child("play here"),
                    )
                    // how far it's got, along the bottom edge
                    .child(div().absolute().left_0().bottom_0().h(px(2.)).w(gpui::relative(frac)).bg(gpui::rgb(0x7ee08a))),
            )
            .into_any_element(),
    )
}
