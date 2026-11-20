//! People's names in their Discord display-name style: the font they picked
//! (all of Discord's: Tempo, Sakura, Jellybean, Modern, Medieval, 8Bit,
//! Vampyre, Monkey Bars, Mainframe, Headbang, Journal, and the older Bangers
//! and BioRhyme) and its colour. Looked up a few people at a time from the
//! crew server and kept for the session.
use gpui::{prelude::*, App, Hsla};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// id → (font, colour); None while unknown or with no style
static LOOKS: Mutex<Option<HashMap<String, Option<(Option<&'static str>, Option<Hsla>)>>>> = Mutex::new(None);
static WANT: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static ASKING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Discord's font ids → the families couch carries
pub fn font(id: i64) -> Option<&'static str> {
    Some(match id {
        1 => "Bangers",
        2 => "BioRhyme",
        3 => "Sakura",
        4 => "Jellybean",
        6 => "Modern",
        7 => "Néo-castel",
        8 => "8Bit",
        10 => "Sinistre",
        12 => "Tempo",
        13 => "Monkey Bars",
        14 => "Mainframe",
        15 => "Headbang",
        16 => "Journal",
        // gg sans: couch's own
        11 | 0 => return None,
        // newer ones (the GTA VI style is Pricedown, which isn't free to
        // ship): the nearest heavy display face couch has
        _ => "Bangers",
    })
}

fn parse(ns: &Value) -> Option<(Option<&'static str>, Option<Hsla>)> {
    let f = ns["font"].as_i64();
    let face = f.and_then(font);
    let col = ns["colors"][0].as_str().and_then(|s| {
        let s = s.trim_start_matches('#');
        u32::from_str_radix(s, 16).ok().filter(|_| s.len() == 6).map(|v| gpui::rgb(v).into())
    });
    (face.is_some() || col.is_some()).then_some((face, col))
}

/// Their style, if known (asks the server for it the first time)
pub fn of(id: &str, cx: &mut App) -> Option<(Option<&'static str>, Option<Hsla>)> {
    if id.is_empty() {
        return None;
    }
    if let Some(v) = LOOKS.lock().get_or_insert_with(HashMap::new).get(id) {
        return *v;
    }
    WANT.lock().get_or_insert_with(HashSet::new).insert(id.to_string());
    if !ASKING.swap(true, std::sync::atomic::Ordering::Relaxed) {
        cx.spawn(async move |cx| {
            // a moment, so a whole list goes in one ask
            crate::timer::after(std::time::Duration::from_millis(150)).await;
            let ids: Vec<String> = WANT.lock().get_or_insert_with(HashSet::new).drain().collect();
            ASKING.store(false, std::sync::atomic::Ordering::Relaxed);
            for chunk in ids.chunks(40) {
                let path = format!("/v1/looks?ids={}", chunk.join(","));
                let r = crate::api::call::<Value>("crew_social", json!({ "method": "GET", "path": path, "body": Value::Null })).await.unwrap_or_default();
                let mut map = LOOKS.lock();
                let map = map.get_or_insert_with(HashMap::new);
                for id in chunk {
                    map.insert(id.clone(), parse(&r["looks"][id.as_str()]));
                }
            }
            let _ = cx.update(|cx| cx.refresh_windows());
        })
        .detach();
    }
    None
}


/// Style one name element with a style looked up beforehand
pub fn apply(d: gpui::Div, st: Option<(Option<&'static str>, Option<Hsla>)>) -> gpui::Div {
    match st {
        Some((face, col)) => d.when_some(face, |d, f| d.font_family(f).font_weight(gpui::FontWeight::NORMAL)).when_some(col, |d, c| d.text_color(c)),
        None => d,
    }
}

