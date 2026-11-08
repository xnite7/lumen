//! Hand-picked Home rows: hits you might have missed, something for the
//! season, award winners, and "More like" the things you finished.

use crate::core::Core;
use crate::error::AppResult;
use crate::integrations::{simkl, simkl_sync, tmdb};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::Duration;

const DAY: Duration = Duration::from_secs(86_400);

/// Outstanding Drama / Comedy / Limited Series winners (title, first year).
const EMMY: &[(&str, u32)] = &[
    ("Shōgun", 2024), ("Succession", 2018), ("The Crown", 2016), ("Game of Thrones", 2011), ("Breaking Bad", 2008),
    ("The Bear", 2022), ("Hacks", 2021), ("Ted Lasso", 2020), ("Schitt's Creek", 2015), ("Fleabag", 2016),
    ("The Marvelous Mrs. Maisel", 2017), ("Veep", 2012), ("Adolescence", 2025), ("Baby Reindeer", 2024), ("Beef", 2023),
    ("The White Lotus", 2021), ("The Queen's Gambit", 2020), ("Chernobyl", 2019), ("Big Little Lies", 2017),
    ("The Handmaid's Tale", 2017), ("Homeland", 2011), ("Mad Men", 2007), ("Modern Family", 2009), ("30 Rock", 2006),
    ("The Sopranos", 1999), ("The West Wing", 1999),
];

/// Best Picture winners (title, release year).
const OSCAR: &[(&str, u32)] = &[
    ("Anora", 2024), ("Oppenheimer", 2023), ("Everything Everywhere All at Once", 2022), ("CODA", 2021),
    ("Nomadland", 2020), ("Parasite", 2019), ("Green Book", 2018), ("The Shape of Water", 2017), ("Moonlight", 2016),
    ("Spotlight", 2015), ("Birdman", 2014), ("12 Years a Slave", 2013), ("Argo", 2012), ("The Departed", 2006),
    ("No Country for Old Men", 2007), ("Slumdog Millionaire", 2008),
];

/// (row name, movie genres, tv genres) for this month.
fn season() -> (&'static str, &'static str, &'static str) {
    match month() {
        12 => ("Holiday Movie Nights", "10751", "10751"),
        1 => ("New Year, New Obsession", "18", "18"),
        2 => ("Date Night", "10749", "35"),
        3..=5 => ("Spring Mysteries", "9648", "9648"),
        6..=8 => ("Summer Blockbusters", "28|12", "10759"),
        9 => ("Back to the Couch", "35", "35"),
        10 => ("Spooky Season", "27", "9648"),
        _ => ("Cozy Nights In", "35|10749", "35|10751"),
    }
}

fn month() -> u32 {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    (if mp < 10 { mp + 3 } else { mp - 9 }) as u32
}

fn year() -> u32 {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    1970 + (secs / 31_556_952) as u32
}

/// What you've already seen or are watching (IMDb ids), so "missed" means missed.
fn seen(core: &Core) -> HashSet<String> {
    if !simkl::connected(core) {
        return HashSet::new();
    }
    ["completed", "watching", "dropped", "hold"]
        .iter()
        .flat_map(|st| ["movie", "series"].iter().flat_map(move |k| simkl_sync::list(core, st, k)))
        .filter_map(|r| simkl::inner(&r).pointer("/ids/imdb").and_then(|i| i.as_str()).map(|s| s.to_string()))
        .collect()
}

fn not_seen(v: Vec<Value>, seen: &HashSet<String>) -> Vec<Value> {
    v.into_iter().filter(|m| m.get("id").and_then(|i| i.as_str()).map_or(true, |id| !seen.contains(id))).collect()
}

async fn discover(core: &Core, kind: &str, params: &str, skip: u32) -> Vec<Value> {
    let page = skip / 20 + 1;
    let Ok(v) = tmdb::get(core, &format!("/discover/{kind}"), &format!("{params}&page={page}"), DAY).await else { return vec![] };
    let rows = v.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    tmdb::resolve_rows(core, rows, kind).await
}

async fn by_titles(core: &Core, kind: &str, list: &[(&str, u32)]) -> Vec<Value> {
    let year_key = if kind == "tv" { "first_air_date_year" } else { "year" };
    let mut rows = Vec::new();
    for (title, y) in list {
        let p = format!("&query={}&{year_key}={y}", urlencoding::encode(title));
        if let Ok(v) = tmdb::get(core, &format!("/search/{kind}"), &p, DAY * 30).await {
            if let Some(r) = v.get("results").and_then(|r| r.as_array()).and_then(|a| a.first()) {
                rows.push(r.clone());
            }
        }
    }
    tmdb::resolve_rows(core, rows, kind).await
}

/// The rows Home (or the Shows / Movies tabs) should show, with their names.
pub fn rows(core: &Core, scope: &str) -> Vec<Value> {
    let (sname, _, _) = season();
    let mut out = vec![];
    let want = |t: &str| scope == "all" || scope == t;
    if want("series") {
        out.push(json!({ "id": "couch-hits", "type": "series", "name": "Hit Shows You Might Have Missed" }));
    }
    if want("movie") {
        out.push(json!({ "id": "couch-hits", "type": "movie", "name": "Hit Films You Might Have Missed" }));
    }
    for t in ["movie", "series"] {
        if want(t) && (scope != "all" || t == "movie") {
            out.push(json!({ "id": "couch-season", "type": t, "name": sname }));
        }
    }
    if want("series") {
        out.push(json!({ "id": "couch-emmy", "type": "series", "name": "Emmy-Winning Series" }));
    }
    if want("movie") {
        out.push(json!({ "id": "couch-oscar", "type": "movie", "name": "Best Picture Winners" }));
    }
    // "More like" the last things you finished (and didn't hate)
    if simkl::connected(core) {
        let mut seeds = 0;
        for t in ["series", "movie"] {
            if !want(t) {
                continue;
            }
            for r in simkl_sync::list(core, "completed", t)
                .iter()
                .filter(|r| r.get("user_rating").and_then(|v| v.as_u64()).map_or(true, |r| r >= 7))
                .filter(|r| (if simkl::is_show(r) { "series" } else { "movie" }) == t)
                .take(if scope == "all" { 1 } else { 2 })
            {
                let inner = simkl::inner(r);
                let (Some(imdb), Some(title)) = (inner.pointer("/ids/imdb").and_then(|i| i.as_str()), inner.get("title").and_then(|i| i.as_str())) else { continue };
                out.push(json!({ "id": "couch-morelike", "type": t, "name": format!("More Like {title}"), "extra": [["seed", imdb]] }));
                seeds += 1;
            }
        }
        let _ = seeds;
    }
    out
}

pub async fn catalog(core: &Core, id: &str, kind: &str, seed: Option<&str>, skip: u32) -> AppResult<Vec<Value>> {
    let tk = tmdb::kind_of(kind);
    let seen = seen(core);
    Ok(match id {
        "couch-hits" => {
            // loved by lots of people, a few years old: the ones that got away
            let cut = year().saturating_sub(2);
            let date = if tk == "tv" { "first_air_date.lte" } else { "primary_release_date.lte" };
            let p = format!("&sort_by=vote_count.desc&vote_average.gte=7.8&vote_count.gte=1500&{date}={cut}-12-31&without_genres=16,10764,10767");
            not_seen(discover(core, tk, &p, skip).await, &seen)
        }
        "couch-season" => {
            let (_, mg, tg) = season();
            let g = if tk == "tv" { tg } else { mg };
            let p = format!("&sort_by=popularity.desc&with_genres={}&vote_count.gte=300&vote_average.gte=6.5", urlencoding::encode(g));
            discover(core, tk, &p, skip).await
        }
        // a streaming service's shelf: couch-prov-<TMDB provider id>
        c if c.starts_with("couch-prov-") => {
            let pid = c.trim_start_matches("couch-prov-");
            let p = format!("&with_watch_providers={}&watch_region=US&sort_by=popularity.desc&vote_count.gte=40", urlencoding::encode(pid));
            discover(core, tk, &p, skip).await
        }
        "couch-emmy" if skip == 0 => by_titles(core, "tv", EMMY).await,
        "couch-oscar" if skip == 0 => by_titles(core, "movie", OSCAR).await,
        "couch-morelike" if skip == 0 => {
            let Some(imdb) = seed else { return Ok(vec![]) };
            // Simkl's "watched this, also watched" first; TMDB when it's thin
            let s = not_seen(simkl::similar(core, imdb, kind).await, &seen);
            if s.len() >= 8 {
                return Ok(s);
            }
            let Ok(Some(tid)) = tmdb::tmdb_id_for_imdb(core, imdb, tk).await else { return Ok(vec![]) };
            let rows = tmdb::get(core, &format!("/{tk}/{tid}/recommendations"), "", DAY)
                .await
                .ok()
                .and_then(|v| v.get("results").and_then(|r| r.as_array()).cloned())
                .unwrap_or_default();
            not_seen(tmdb::resolve_rows(core, rows, tk).await, &seen)
        }
        _ => vec![],
    })
}
