//! The smart bar: ask for anything ("a superhero film that isn't cringe or
//! for kids") and get picks that answer THAT first, ordered by your taste.
//!
//! The model works with tools: it reads a short summary of your taste (built
//! once from your Simkl ratings and cached), can pull the full rating
//! history when it needs specifics, searches TMDB, finds titles similar to
//! one it has in mind, and checks what you've already seen. Without Simkl it
//! simply answers the request.

use super::{simkl, tmdb};
use crate::core::Core;
use crate::error::{AppError, AppResult};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::{Duration, Instant};


struct Profile {
    text: String,
    /// the model's own short read of this taste, made once per change
    summary: Option<String>,
    exclude: HashSet<String>,
    rated: usize,
    sig: String,
    at: Instant,
}

static PROFILE: Mutex<Option<Profile>> = Mutex::new(None);

const BANDS: &[(u8, u8, &str)] = &[
    (10, 10, "10/10 — THE CANON. These define the taste; weight them above everything else"),
    (9, 9, "9/10 — near-canon"),
    (8, 8, "8/10 — strong likes"),
    (7, 7, "7/10 — enjoyed, but NOT the signal a 9 or 10 is"),
    (6, 6, "6/10 — mild; barely evidence of taste"),
    (5, 5, "5/10 — indifference"),
    (1, 4, "1-4/10 — ACTIVELY DISLIKED. Strong negative signal; steer away from what these share"),
];

const SCALE_NOTE: &str = "HOW TO READ THESE RATINGS: weigh the top of the scale much more than the
middle. A 10 or 9 says far more about this person than a 7, and a 7 is closer
to a shrug than a recommendation. Ratings of 1-4 are active dislike: what
those titles share is as informative as what the favourites share.";

const SUMMARY_INSTRUCTIONS: &str = r#"Below is one person's full film and TV rating history. Write a taste
summary another recommender will rely on. Find what actually predicts whether
they love something: register, tone, pacing, how dark or earnest, what they
forgive and what they punish. Subject matter (vampires, heists, space) is
almost never the reason, so don't lead with it, and don't build a pattern on
fewer than four highly rated titles.

Return JSON: {"summary": "120-200 words, second person", "loves": [6-10 short
axes], "avoids": [3-6 short axes], "canon": [their 10-15 most defining titles
with years]}"#;

const BAR_INSTRUCTIONS: &str = r#"You are couch's smart bar. Someone typed a request for something to watch.

1. THE REQUEST COMES FIRST. Work out exactly what they're asking for,
   including what they mean but didn't spell out: "a superhero thing that
   isn't cringe or for kids" means grown-up, subversive, adult superhero
   stories (think The Boys, Watchmen), not family Marvel. Read negations and
   slang the way a friend would. Anything that fails the request is out, no
   matter how well it fits their taste.
2. THEN THEIR TASTE decides the order (when there is a taste profile). Use
   get_taste_details when you need specifics. Never recommend anything they've
   already rated, dropped or put on their watchlist: use check_seen.
3. USE THE TOOLS to find real, existing titles: search_titles to check a
   title and its year, find_similar to expand from a title that fits, discover
   to browse a genre and era. Prefer titles you have confirmed exist.
4. Answer with JSON only:
   {"read": "one line on what they actually want", "picks": [{"title": "",
   "year": 2019, "type": "movie|series", "why": "one or two sentences"}]}
   Exactly 5 picks, best first (only the top 3 are shown; 4 and 5 are spares).
   Each "why" connects the pick to the request and, if there's a taste
   profile, to something specific they rated. Talk to them as "you", in their
   register."#;

const PROMPT_INSTRUCTIONS: &str = r#"From this person's taste summary, write 3 chips they'd tap on a smart bar
to find something to watch right now. Each is a mood or a vibe in their own
words, never a bare title or genre name: "a light comedy?", "cry all night
tonight", "a cinematography masterpiece". Make the three different: one close
to what they love, one at an edge of their taste, one a swing. Lowercase.

Return JSON: {"prompts": [{"label": "2-7 words", "q": "the fuller request"}]}"#;

fn esc(s: &str) -> String {
    s.replace(['\r', '\n', '|'], " ").trim().to_string()
}

fn line(e: &Value) -> String {
    let it = simkl::inner(e);
    let kind = if simkl::is_show(e) { "series" } else { "movie" };
    let genres = it
        .get("genres")
        .and_then(|g| g.as_array())
        .filter(|g| !g.is_empty())
        .map(|g| format!(" · {}", g.iter().take(4).filter_map(|x| x.as_str()).map(esc).collect::<Vec<_>>().join(", ")))
        .unwrap_or_default();
    let year = it.get("year").map(|y| format!(" ({y})")).unwrap_or_default();
    format!("- {}{year} [{kind}]{genres}", esc(it.get("title").and_then(|t| t.as_str()).unwrap_or("Unknown")))
}

/// Sorted by title so the prompt prefix is byte-identical between builds;
/// an unsorted list silently kills every prompt-cache read.
fn sorted(mut rows: Vec<Value>) -> Vec<Value> {
    let key = |e: &Value| {
        let it = simkl::inner(e);
        (it.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string(), it.get("year").and_then(|y| y.as_i64()).unwrap_or(0))
    };
    rows.sort_by(|a, b| key(a).cmp(&key(b)));
    rows
}

fn all_rows(d: &Value) -> Vec<Value> {
    let mut v = Vec::new();
    for k in ["movies", "shows", "anime"] {
        v.extend(d.get(k).and_then(|a| a.as_array()).cloned().unwrap_or_default());
    }
    v
}

fn base_rates(rated: &[Value]) -> Vec<String> {
    use std::collections::HashMap;
    let mut g: HashMap<String, (u32, u32, f64)> = HashMap::new();
    for e in rated {
        let r = e.get("user_rating").and_then(|v| v.as_f64()).unwrap_or(0.0);
        for name in simkl::inner(e).get("genres").and_then(|g| g.as_array()).into_iter().flatten().take(6) {
            let k = esc(name.as_str().unwrap_or(""));
            if k.is_empty() {
                continue;
            }
            let s = g.entry(k).or_insert((0, 0, 0.0));
            s.0 += 1;
            s.2 += r;
            if r >= 8.0 {
                s.1 += 1;
            }
        }
    }
    if g.is_empty() {
        return vec![];
    }
    let mut rows: Vec<(String, (u32, u32, f64))> = g.into_iter().collect();
    rows.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(b.1 .0.cmp(&a.1 .0)).then(a.0.cmp(&b.0)));
    let mut out = vec![
        "## BASE RATES — what each label actually accounts for".to_string(),
        "Counted exactly, because this is not countable by eye and miscounting it is".into(),
        "the whole failure mode. \"8+\" is the column that means anything: a big \"rated\"".into(),
        "with a small \"8+\" means they watch the label and are unmoved by it. Fewer".into(),
        "than 4 at 8+ is a coincidence, not a taste. These are coarse labels — treat".into(),
        "them as evidence of PROPORTION only, never as the axes themselves.".into(),
    ];
    for (name, (n, hi, sum)) in rows {
        out.push(format!(
            "- {name}: {n} rated, {hi} at 8+, mean {:.1}{}",
            sum / n as f64,
            if hi < 4 { "   <-- THIN. Not a preference." } else { "" }
        ));
    }
    out.push(String::new());
    out
}

async fn build_profile(core: &Core) -> AppResult<Profile> {
    let scores = "1,2,3,4,5,6,7,8,9,10";
    let (pm, ps, pa) = (
        format!("/sync/ratings/movies/{scores}?extended=full"),
        format!("/sync/ratings/shows/{scores}?extended=full"),
        format!("/sync/ratings/anime/{scores}?extended=full"),
    );
    let (rm, rs, ra, dropped, ptw, act) = tokio::join!(
        simkl::get(core, &pm),
        simkl::get(core, &ps),
        simkl::get(core, &pa),
        simkl::get(core, "/sync/all-items/all/dropped?extended=full"),
        simkl::get(core, "/sync/all-items/all/plantowatch?extended=full"),
        simkl::get(core, "/sync/activities"),
    );
    let sig = act.map(|a| format!("{}|{}", a.get("movies").cloned().unwrap_or_default(), a.get("tv_shows").cloned().unwrap_or_default())).unwrap_or_default();
    let mut rated = Vec::new();
    for r in [rm, rs, ra].into_iter().flatten() {
        rated.extend(all_rows(&r).into_iter().filter(|e| e.get("user_rating").and_then(|v| v.as_f64()).is_some()));
    }
    let mut exclude = HashSet::new();
    let add_id = |e: &Value, ex: &mut HashSet<String>| {
        if let Some(i) = simkl::inner(e).pointer("/ids/imdb").and_then(|i| i.as_str()) {
            ex.insert(i.to_string());
        }
    };
    for e in &rated {
        add_id(e, &mut exclude);
    }
    let dropped = dropped.map(|d| all_rows(&d)).unwrap_or_default();
    for e in &dropped {
        add_id(e, &mut exclude);
    }

    let mut out = vec![SCALE_NOTE.to_string(), String::new()];
    out.extend(base_rates(&rated));
    for (lo, hi, title) in BANDS {
        let rows: Vec<Value> = rated
            .iter()
            .filter(|e| {
                let r = e.get("user_rating").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                r >= *lo && r <= *hi
            })
            .cloned()
            .collect();
        if rows.is_empty() {
            continue;
        }
        out.push(format!("## {title} ({})", rows.len()));
        out.extend(sorted(rows).iter().map(line));
        out.push(String::new());
    }
    let drop_rows = sorted(dropped.into_iter().filter(|e| e.get("user_rating").map_or(true, |r| r.is_null())).collect());
    if !drop_rows.is_empty() {
        out.push(format!("## Started and abandoned, unrated ({}) — bounced off these; softer negative signal", drop_rows.len()));
        out.extend(drop_rows.iter().take(120).map(line));
        out.push(String::new());
    }
    let ptw_rows = sorted(ptw.map(|d| all_rows(&d)).unwrap_or_default());
    if !ptw_rows.is_empty() {
        out.push(format!("## Watchlist, never started ({}) — ASPIRATION, not proven taste.", ptw_rows.len()));
        out.push("These say what they think they should like. Weight far below the rated bands,".into());
        out.push("and do not recommend these back — they already know about them.".into());
        for e in ptw_rows.iter().take(120) {
            out.push(line(e));
            add_id(e, &mut exclude);
        }
        out.push(String::new());
    }
    Ok(Profile { text: out.join("\n"), summary: None, exclude, rated: rated.len(), sig, at: Instant::now() })
}

async fn profile(core: &Core) -> AppResult<(String, HashSet<String>, usize)> {
    // Invalidate on the activities signature (a new rating changes it) with
    // a 6h backstop.
    let sig = simkl::get(core, "/sync/activities")
        .await
        .map(|a| format!("{}|{}", a.get("movies").cloned().unwrap_or_default(), a.get("tv_shows").cloned().unwrap_or_default()))
        .unwrap_or_default();
    {
        let g = PROFILE.lock();
        if let Some(p) = g.as_ref() {
            if p.sig == sig && p.at.elapsed() < Duration::from_secs(6 * 3600) {
                return Ok((p.text.clone(), p.exclude.clone(), p.rated));
            }
        }
    }
    let p = build_profile(core).await?;
    let r = (p.text.clone(), p.exclude.clone(), p.rated);
    *PROFILE.lock() = Some(p);
    Ok(r)
}

/// The smart bar runs on a free model (Google Gemini's free tier through its
/// OpenAI-compatible endpoint, by default). The key is couch's own, built in;
/// the provider can be swapped at build time with COUCH_AI_URL / COUCH_AI_MODEL.
const AI_URL: &str = match option_env!("COUCH_AI_URL") {
    Some(u) => u,
    None => "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
};
/// Tried in order: a pinned model if the build names one, then Google's
/// "latest flash" alias, then a known model, so a retired name never breaks it.
fn models() -> Vec<&'static str> {
    let mut v = vec![];
    if let Some(m) = option_env!("COUCH_AI_MODEL").filter(|m| !m.is_empty()) {
        v.push(m);
    }
    // when one is swamped ("high demand"), the next one usually isn't
    v.extend(["gemini-flash-latest", "gemini-2.5-flash", "gemini-2.5-flash-lite"]);
    v
}

pub fn available(core: &Core) -> bool {
    core.key("ai").is_some()
}

/// One chat turn. Returns the assistant message (content and/or tool calls).
async fn chat(core: &Core, messages: &[Value], tools: Option<&Value>, json_out: bool) -> AppResult<Value> {
    let key = core.key("ai").ok_or_else(|| AppError::msg("The smart bar isn't set up in this build"))?;
    let mut last_err = String::new();
    for model in models() {
        let mut body = json!({ "model": model, "messages": messages, "max_tokens": 4096 });
        if let Some(t) = tools {
            body["tools"] = t.clone();
        }
        if json_out {
            body["response_format"] = json!({ "type": "json_object" });
        }
        let send = || core.http.client().post(AI_URL).bearer_auth(&key).timeout(Duration::from_secs(90)).json(&body).send();
        let mut res = send().await?;
        // free tiers rate-limit: a few spaced retries
        for attempt in 1..3 {
            let st = res.status().as_u16();
            if st != 429 && st < 500 {
                break;
            }
            tokio::time::sleep(Duration::from_millis((900.0 * 2.0f64.powi(attempt - 1)) as u64)).await;
            res = send().await?;
        }
        let st = res.status();
        if st.is_success() {
            let data: Value = res.json().await?;
            return data.pointer("/choices/0/message").cloned().ok_or_else(|| AppError::msg("Empty answer"));
        }
        let t = res.text().await.unwrap_or_default();
        // overloaded or rate-limited: try the next model before giving up
        if matches!(st.as_u16(), 429 | 500 | 502 | 503 | 504) {
            last_err = format!("{st}");
            continue;
        }
        // unknown model: try the next one
        if st.as_u16() == 404 || (st.as_u16() == 400 && t.to_lowercase().contains("model")) {
            last_err = format!("{st}");
            continue;
        }
        return Err(AppError::msg(format!("Smart bar error {st}: {}", t.chars().take(200).collect::<String>())));
    }
    Err(AppError::msg(if last_err.starts_with("429") || last_err.starts_with("503") {
        "the smart bar's a bit swamped right now. give it a minute and ask again.".to_string()
    } else {
        format!("the smart bar couldn't think straight ({last_err}). try again?")
    }))
}

fn parse_json(text: &str) -> AppResult<Value> {
    let t = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    // tolerate prose around the object
    let t = match (t.find('{'), t.rfind('}')) {
        (Some(a), Some(b)) if b > a => &t[a..=b],
        _ => t,
    };
    Ok(serde_json::from_str(t)?)
}

/// Your taste in a paragraph, made once from your ratings and cached until
/// they change.
async fn taste(core: &Core) -> Option<(String, String, HashSet<String>)> {
    if !simkl::connected(core) {
        return None;
    }
    let (text, exclude, rated) = profile(core).await.ok()?;
    if rated < 5 {
        return None;
    }
    if let Some(s) = PROFILE.lock().as_ref().and_then(|p| p.summary.clone()) {
        return Some((s, text, exclude));
    }
    let msgs = vec![json!({ "role": "system", "content": SUMMARY_INSTRUCTIONS }), json!({ "role": "user", "content": text })];
    let m = chat(core, &msgs, None, true).await.ok()?;
    let v = parse_json(m.get("content")?.as_str()?).ok()?;
    let list = |k: &str| v.get(k).and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("; ")).unwrap_or_default();
    let summary = format!(
        "{}\nLoves: {}\nAvoids: {}\nTheir canon: {}",
        v.get("summary").and_then(|s| s.as_str()).unwrap_or(""),
        list("loves"),
        list("avoids"),
        list("canon")
    );
    if let Some(p) = PROFILE.lock().as_mut() {
        p.summary = Some(summary.clone());
    }
    Some((summary, text, exclude))
}

fn tool_defs(with_taste: bool) -> Value {
    let mut t = vec![
        json!({ "type": "function", "function": {
            "name": "search_titles",
            "description": "Search TMDB for films or shows by title. Returns real titles with year, genres, rating and a short overview.",
            "parameters": { "type": "object", "properties": {
                "query": { "type": "string" },
                "type": { "type": "string", "enum": ["movie", "series"] },
                "year": { "type": "integer" }
            }, "required": ["query", "type"] }
        }}),
        json!({ "type": "function", "function": {
            "name": "find_similar",
            "description": "Titles TMDB recommends for people who liked the given title.",
            "parameters": { "type": "object", "properties": {
                "title": { "type": "string" },
                "type": { "type": "string", "enum": ["movie", "series"] },
                "year": { "type": "integer" }
            }, "required": ["title", "type"] }
        }}),
        json!({ "type": "function", "function": {
            "name": "discover",
            "description": "Browse well-rated titles by genre and era.",
            "parameters": { "type": "object", "properties": {
                "type": { "type": "string", "enum": ["movie", "series"] },
                "genres": { "type": "array", "items": { "type": "string" }, "description": "Movie genres: Action, Adventure, Animation, Comedy, Crime, Documentary, Drama, Family, Fantasy, History, Horror, Music, Mystery, Romance, Science Fiction, Thriller, War, Western. Show genres: Action & Adventure, Animation, Comedy, Crime, Documentary, Drama, Family, Kids, Mystery, Reality, Sci-Fi & Fantasy, War & Politics, Western." },
                "exclude_genres": { "type": "array", "items": { "type": "string" } },
                "year_from": { "type": "integer" },
                "year_to": { "type": "integer" },
                "sort": { "type": "string", "enum": ["popular", "acclaimed", "newest"] }
            }, "required": ["type"] }
        }}),
    ];
    if with_taste {
        t.push(json!({ "type": "function", "function": {
            "name": "get_taste_details",
            "description": "Their full rating history by band (10s, 9s ... dislikes, dropped, watchlist). Use when the summary isn't specific enough.",
            "parameters": { "type": "object", "properties": {} }
        }}));
        t.push(json!({ "type": "function", "function": {
            "name": "check_seen",
            "description": "Which of these titles they have already rated, dropped or put on their watchlist.",
            "parameters": { "type": "object", "properties": {
                "titles": { "type": "array", "items": { "type": "string" } }
            }, "required": ["titles"] }
        }}));
    }
    Value::Array(t)
}

fn brief(rows: &[Value], tk: &str, n: usize) -> Value {
    let genres = if tk == "tv" { tmdb::TV_GENRES } else { tmdb::MOVIE_GENRES };
    Value::Array(
        rows.iter()
            .take(n)
            .map(|r| {
                let g: Vec<&str> = r
                    .get("genre_ids")
                    .and_then(|g| g.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|id| id.as_u64())
                    .filter_map(|id| genres.iter().find(|(_, gid)| *gid as u64 == id).map(|(n, _)| *n))
                    .collect();
                json!({
                    "title": r.get("title").or_else(|| r.get("name")),
                    "year": r.get("release_date").or_else(|| r.get("first_air_date")).and_then(|d| d.as_str()).and_then(|d| d.get(0..4)),
                    "genres": g,
                    "rating": r.get("vote_average"),
                    "overview": r.get("overview").and_then(|o| o.as_str()).map(|o| o.chars().take(220).collect::<String>()),
                })
            })
            .collect(),
    )
}

async fn tmdb_rows(core: &Core, path: &str, params: &str) -> Vec<Value> {
    tmdb::get(core, path, params, Duration::from_secs(86_400))
        .await
        .ok()
        .and_then(|v| v.get("results").and_then(|r| r.as_array()).cloned())
        .unwrap_or_default()
}

async fn run_tool(core: &Core, name: &str, args: &Value, full: &str, seen_titles: &HashSet<String>) -> Value {
    let kind = args.get("type").and_then(|t| t.as_str()).unwrap_or("movie");
    let tk = tmdb::kind_of(kind);
    let year_key = if tk == "tv" { "first_air_date_year" } else { "year" };
    match name {
        "search_titles" => {
            let mut p = format!("&query={}", urlencoding::encode(args.get("query").and_then(|q| q.as_str()).unwrap_or("")));
            if let Some(y) = args.get("year").and_then(|y| y.as_i64()) {
                p.push_str(&format!("&{year_key}={y}"));
            }
            brief(&tmdb_rows(core, &format!("/search/{tk}"), &p).await, tk, 6)
        }
        "find_similar" => {
            let mut p = format!("&query={}", urlencoding::encode(args.get("title").and_then(|q| q.as_str()).unwrap_or("")));
            if let Some(y) = args.get("year").and_then(|y| y.as_i64()) {
                p.push_str(&format!("&{year_key}={y}"));
            }
            let Some(id) = tmdb_rows(core, &format!("/search/{tk}"), &p).await.first().and_then(|r| r.get("id")).and_then(|i| i.as_u64()) else {
                return json!({ "error": "title not found" });
            };
            brief(&tmdb_rows(core, &format!("/{tk}/{id}/recommendations"), "").await, tk, 12)
        }
        "discover" => {
            let genres = if tk == "tv" { tmdb::TV_GENRES } else { tmdb::MOVIE_GENRES };
            let ids = |k: &str| {
                args.get(k)
                    .and_then(|g| g.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|g| g.as_str())
                    .filter_map(|g| genres.iter().find(|(n, _)| n.eq_ignore_ascii_case(g)).map(|(_, id)| id.to_string()))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            let date = if tk == "tv" { "first_air_date" } else { "primary_release_date" };
            let sort = match args.get("sort").and_then(|s| s.as_str()) {
                Some("acclaimed") => "&sort_by=vote_average.desc&vote_count.gte=500",
                Some("newest") => "&sort_by=primary_release_date.desc&vote_count.gte=50",
                _ => "&sort_by=popularity.desc&vote_count.gte=200",
            };
            let mut p = sort.to_string();
            let (g, x) = (ids("genres"), ids("exclude_genres"));
            if !g.is_empty() {
                p.push_str(&format!("&with_genres={g}"));
            }
            if !x.is_empty() {
                p.push_str(&format!("&without_genres={x}"));
            }
            if let Some(y) = args.get("year_from").and_then(|y| y.as_i64()) {
                p.push_str(&format!("&{date}.gte={y}-01-01"));
            }
            if let Some(y) = args.get("year_to").and_then(|y| y.as_i64()) {
                p.push_str(&format!("&{date}.lte={y}-12-31"));
            }
            brief(&tmdb_rows(core, &format!("/discover/{tk}"), &p).await, tk, 15)
        }
        "get_taste_details" => json!(full.chars().take(60_000).collect::<String>()),
        "check_seen" => {
            let hits: Vec<&str> = args
                .get("titles")
                .and_then(|t| t.as_array())
                .into_iter()
                .flatten()
                .filter_map(|t| t.as_str())
                .filter(|t| seen_titles.contains(&t.to_lowercase()))
                .collect();
            json!({ "already_seen": hits })
        }
        _ => json!({ "error": "no such tool" }),
    }
}

/// Lowercased titles from the rating history, for check_seen.
fn seen_titles(full: &str) -> HashSet<String> {
    full.lines()
        .filter_map(|l| l.strip_prefix("- "))
        .map(|l| l.split(" (").next().unwrap_or(l).split(" [").next().unwrap_or(l).trim().to_lowercase())
        .collect()
}

async fn resolve(core: &Core, title: &str, year: Option<i64>, kind: &str) -> Option<Value> {
    let tk = tmdb::kind_of(kind);
    let mut params = format!("&query={}", urlencoding::encode(title));
    if let Some(y) = year {
        params.push_str(&format!("&{}={y}", if tk == "tv" { "first_air_date_year" } else { "year" }));
    }
    let v = tmdb::get(core, &format!("/search/{tk}"), &params, Duration::from_secs(86_400)).await.ok()?;
    let hit = v.get("results")?.as_array()?.first()?.clone();
    let tid = hit.get("id")?.as_u64()?;
    let imdb = tmdb::imdb_for_tmdb(core, tk, tid).await.ok()??;
    Some(json!({
        "id": imdb,
        "type": if tk == "tv" { "series" } else { "movie" },
        "name": hit.get("name").or_else(|| hit.get("title")),
        "year": hit.get("first_air_date").or_else(|| hit.get("release_date")).and_then(|d| d.as_str()).and_then(|d| d.get(0..4)),
        "poster": tmdb::img(hit.get("poster_path").and_then(|p| p.as_str()), "w500"),
        "background": tmdb::img(hit.get("backdrop_path").and_then(|p| p.as_str()), "w1280"),
    }))
}

pub async fn vibe(core: &Core, q: &str) -> AppResult<Value> {
    let t = taste(core).await;
    let (summary, full, exclude) = match &t {
        Some((s, f, e)) => (Some(s.clone()), f.clone(), e.clone()),
        None => (None, String::new(), HashSet::new()),
    };
    let seen = seen_titles(&full);
    let tools = tool_defs(summary.is_some());
    let user = match &summary {
        Some(s) => format!("Request: {q}\n\nTheir taste (summary; get_taste_details has the full history):\n{s}"),
        None => format!("Request: {q}\n\nNo taste profile for this person: answer the request as well as you can."),
    };
    let mut msgs = vec![json!({ "role": "system", "content": BAR_INSTRUCTIONS }), json!({ "role": "user", "content": user })];
    let mut answer: Option<Value> = None;
    for round in 0..6 {
        let last = round == 5;
        let m = chat(core, &msgs, if last { None } else { Some(&tools) }, last).await?;
        let calls = m.get("tool_calls").and_then(|c| c.as_array()).cloned().unwrap_or_default();
        if calls.is_empty() {
            if let Some(c) = m.get("content").and_then(|c| c.as_str()) {
                answer = parse_json(c).ok();
            }
            if answer.is_some() || last {
                break;
            }
            msgs.push(m.clone());
            msgs.push(json!({ "role": "user", "content": "Now give the final answer as the JSON object only." }));
            continue;
        }
        msgs.push(m.clone());
        for c in calls {
            let name = c.pointer("/function/name").and_then(|n| n.as_str()).unwrap_or("");
            let args: Value = c.pointer("/function/arguments").and_then(|a| a.as_str()).and_then(|a| serde_json::from_str(a).ok()).unwrap_or(json!({}));
            let out = run_tool(core, name, &args, &full, &seen).await;
            msgs.push(json!({ "role": "tool", "tool_call_id": c.get("id"), "name": name, "content": out.to_string() }));
        }
    }
    let parsed = answer.ok_or_else(|| AppError::msg("The smart bar lost its train of thought, try again"))?;
    let mut picks = Vec::new();
    for p in parsed.get("picks").and_then(|p| p.as_array()).into_iter().flatten() {
        if picks.len() >= 3 {
            break;
        }
        let title = p.get("title").and_then(|t| t.as_str()).unwrap_or("");
        let Some(mut meta) = resolve(core, title, p.get("year").and_then(|y| y.as_i64()), p.get("type").and_then(|t| t.as_str()).unwrap_or("movie")).await else { continue };
        let id = meta["id"].as_str().unwrap_or("").to_string();
        if exclude.contains(&id) || picks.iter().any(|x: &Value| x["id"] == meta["id"]) {
            continue;
        }
        meta["why"] = p["why"].clone();
        picks.push(meta);
    }
    Ok(json!({ "read": parsed.get("read"), "picks": picks }))
}

pub async fn prompts(core: &Core) -> AppResult<Value> {
    if !available(core) {
        return Ok(json!({ "prompts": [] }));
    }
    let Some((summary, _, _)) = taste(core).await else { return Ok(json!({ "prompts": [] })) };
    let msgs = vec![json!({ "role": "system", "content": PROMPT_INSTRUCTIONS }), json!({ "role": "user", "content": summary })];
    let m = chat(core, &msgs, None, true).await?;
    let v = parse_json(m.get("content").and_then(|c| c.as_str()).unwrap_or("{}"))?;
    Ok(json!({ "prompts": v.get("prompts").and_then(|p| p.as_array()).map(|p| p.iter().take(3).cloned().collect::<Vec<_>>()) }))
}
