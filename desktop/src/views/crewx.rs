//! The crew, read a little deeper: who's on a roll (nights in a row with
//! something watched, and nights in a row you watched together), and the
//! crew's picks: what they rated highest lately that you haven't seen.
use crate::api::{self, Meta};
use chrono::{Duration as Days, Local, NaiveDate, TimeZone};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// a run of nights: whose (None: the two of you together), how long
#[derive(Clone, Debug)]
pub struct Streak {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub nights: u32,
    pub together: bool,
}

const NIGHTS_KEY: &str = "gpui_together_nights";

fn today() -> NaiveDate {
    Local::now().date_naive()
}

fn day_of(ms: f64) -> Option<NaiveDate> {
    Local.timestamp_millis_opt(ms as i64).single().map(|d| d.date_naive())
}

/// nights in a row ending today (or yesterday: tonight isn't over)
fn run(days: &HashSet<NaiveDate>) -> u32 {
    let t = today();
    let mut d = if days.contains(&t) { t } else { t - Days::days(1) };
    let mut n = 0;
    while days.contains(&d) {
        n += 1;
        d -= Days::days(1);
    }
    n
}

/// Watched together with someone tonight: remembered for the streaks.
pub fn note_together(id: &str) {
    if id.is_empty() {
        return;
    }
    let db = &couch::state::get().core.db;
    let mut all = db.get::<HashMap<String, Vec<String>>>(NIGHTS_KEY).unwrap_or_default();
    let t = today().to_string();
    let e = all.entry(id.to_string()).or_default();
    if !e.contains(&t) {
        e.push(t);
        let cut = (today() - Days::days(120)).to_string();
        e.retain(|d| *d >= cut);
        let _ = db.set(NIGHTS_KEY, &all);
    }
}

pub struct Extras {
    pub streaks: Vec<Streak>,
    pub picks: Vec<Meta>,
    pub picks_title: String,
}

fn me() -> String {
    couch::state::get().core.db.get::<Value>("discord_user").and_then(|u| u["id"].as_str().map(String::from)).unwrap_or_default()
}

pub async fn load(feed: &[Value]) -> Extras {
    let people = api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/people", "body": Value::Null })).await.unwrap_or_default();
    let people: Vec<Value> = people["people"].as_array().cloned().unwrap_or_default();
    let me = me();
    let jobs = people.iter().take(30).map(|p| {
        let id = p["id"].as_str().unwrap_or("").to_string();
        async move {
            let v = api::call::<Value>("crew_social", json!({ "method": "GET", "path": format!("/v1/profile/{id}"), "body": Value::Null })).await.unwrap_or_default();
            (id, v)
        }
    });
    let profiles: Vec<(String, Value)> = futures::future::join_all(jobs).await;
    let now = chrono::Utc::now().timestamp_millis() as f64;

    // ------------------------------------------------------------ streaks
    let mut streaks = vec![];
    let together = couch::state::get().core.db.get::<HashMap<String, Vec<String>>>(NIGHTS_KEY).unwrap_or_default();
    for (id, v) in &profiles {
        let name = v["name"].as_str().unwrap_or("someone").to_string();
        let avatar = v["avatar"].as_str().map(String::from);
        // nights with something watched: their recent and in-progress titles, and their feed
        let mut days: HashSet<NaiveDate> = HashSet::new();
        for key in ["watching", "recent"] {
            for i in v["profile"][key].as_array().into_iter().flatten() {
                if let Some(d) = i["at"].as_f64().and_then(day_of) {
                    days.insert(d);
                }
            }
        }
        for a in v["activity"].as_array().into_iter().flatten() {
            if let Some(d) = a["at"].as_f64().and_then(day_of) {
                days.insert(d);
            }
        }
        let n = run(&days);
        if n >= 3 {
            streaks.push(Streak { id: id.clone(), name: if *id == me { "you".into() } else { name.clone() }, avatar: avatar.clone(), nights: n, together: false });
        }
        if *id != me {
            let nights: HashSet<NaiveDate> = together.get(id).into_iter().flatten().filter_map(|d| d.parse().ok()).collect();
            let n = run(&nights);
            if n >= 2 {
                streaks.push(Streak { id: id.clone(), name, avatar, nights: n, together: true });
            }
        }
    }
    streaks.sort_by(|a, b| b.together.cmp(&a.together).then(b.nights.cmp(&a.nights)));

    // --------------------------------------------------------- crew picks
    // what I've seen (or started), so it's never suggested
    let mine = profiles.iter().find(|(id, _)| *id == me).map(|(_, v)| v.clone()).unwrap_or_default();
    let mut seen: HashSet<String> = mine["profile"]["taste"]["ids"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
    for key in ["watching", "recent"] {
        for i in mine["profile"][key].as_array().into_iter().flatten() {
            if let Some(id) = i["id"].as_str() {
                seen.insert(id.to_string());
            }
        }
    }
    let mut best = |days: f64| -> Vec<Meta> {
        let since = now - days * 86_400_000.;
        // id -> (score, the title, how many of the crew)
        let mut score: HashMap<String, (f64, Value, u32)> = HashMap::new();
        let mut add = |t: &Value, r: f64| {
            let Some(id) = t["id"].as_str() else { return };
            let id = id.split(':').next().unwrap_or(id).to_string();
            if seen.contains(&id) || r < 7. {
                return;
            }
            let e = score.entry(id).or_insert((0., t.clone(), 0));
            e.0 += r - 6.;
            e.2 += 1;
        };
        for (id, v) in &profiles {
            if *id == me {
                continue;
            }
            let ratings = &v["profile"]["taste"]["ratings"];
            for i in v["profile"]["recent"].as_array().into_iter().flatten() {
                if i["at"].as_f64().unwrap_or(0.) < since {
                    continue;
                }
                let r = i["rating"].as_f64().or_else(|| i["id"].as_str().and_then(|x| ratings[x].as_f64()));
                if let Some(r) = r {
                    add(i, r);
                }
            }
        }
        for a in feed {
            if a["who"].as_str() == Some(me.as_str()) || a["at"].as_f64().unwrap_or(0.) < since {
                continue;
            }
            if let Some(r) = a["rating"].as_f64() {
                add(&a["title"], r);
            }
        }
        let mut list: Vec<(f64, Value, u32)> = score.into_values().collect();
        list.sort_by(|a, b| (b.0 + b.2 as f64).partial_cmp(&(a.0 + a.2 as f64)).unwrap_or(std::cmp::Ordering::Equal));
        list.into_iter().take(20).filter_map(|(_, t, _)| serde_json::from_value::<Meta>(t).ok()).filter(|m| !m.id.is_empty()).collect()
    };
    let mut picks = best(7.);
    let mut picks_title = "the crew's picks this week".to_string();
    if picks.len() < 4 {
        picks = best(45.);
        picks_title = "the crew's picks lately".into();
    }
    Extras { streaks, picks, picks_title }
}
