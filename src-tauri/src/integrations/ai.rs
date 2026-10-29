//! Vibe search: answer a MOOD from your own Simkl ratings.
//!
//! Ported from the worker's /ai routes with the same prompts and the same
//! taste-profile construction, so answers read the way you tuned them. The
//! difference: the Anthropic key stays on this machine and is sent straight
//! to Anthropic, so there's no shared secret and no public URL to leak.

use super::{simkl, tmdb};
use crate::core::Core;
use crate::error::{AppError, AppResult};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::{Duration, Instant};

const ANTHROPIC: &str = "https://api.anthropic.com/v1/messages";

struct Profile {
    text: String,
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

const SCALE_NOTE: &str = "HOW TO READ THESE RATINGS — this is not a linear 1-10 scale.
This user rates exponentially. Approximate weight = 2^(rating-5):
  10 -> 32x   9 -> 16x   8 -> 8x   7 -> 4x   6 -> 2x   5 -> 1x (indifference)
A single 10 outweighs eight 7s. A 7 is NOT a recommendation, it is a shrug.
Ratings of 1-4 are active dislike and carry strong NEGATIVE weight — what those
titles have in common is as informative as what the 10s have in common.";

const VIBE_INSTRUCTIONS: &str = r#"You recommend films and TV to ONE person whose complete rating history follows.
Your job is not to match genres. It is to understand them.

Work in this order. Do not skip the first step.

1. THE REQUEST IS A HARD FILTER, NOT A HINT. Decide what they asked for rules
   IN and what it rules OUT, and write that into "constraint" before anything
   else. A title that fails the literal request is disqualified however well it
   fits their taste. Taste decides the ORDER of what qualifies. It never
   overrides the ask, and it never quietly widens it.

2. DERIVE THE AXES — the things that actually predict whether they will love
   something. The right shape of axis: "rewards formal audacity over plot",
   "punishes sentimentality", "loves a film that fully commits to a bit",
   "wants texture and atmosphere more than story", "forgives a mess if it is a
   specific mess". Genre labels are the shallow read; derive what sits
   underneath. Titles that share a genre but sit in different rating bands are
   where the real signal is.

SUBJECT MATTER IS NOT TASTE. This is the single most common way to get this
wrong, so treat it as a rule and not advice. Vampires, boxing, heists, WWII,
space, chess — that is what a film is ABOUT, and what a film is about is almost
never why this person liked it. If two gothic horrors sit in the top band, the
finding is NOT "likes vampires"; it is whatever those two share with the REST
of the canon — a register, a rhythm, a kind of excess, a specific joke. Check
the base rates before generalising from any cluster: a motif in three titles
out of two hundred is noise no matter how highly those three are rated. If you
cannot find four high-band titles supporting a pattern, it is not a pattern.

Then answer the mood they gave you. Rules:
- Exactly 5 picks, STRICTLY ordered best-fit first. Only the top 3 are ever
  shown — the other two exist solely as replacements for any that cannot be
  linked. So put everything into the first three and treat 4 and 5 as spares;
  do not pad the top of the list with safe choices to hold something back.
- NEVER recommend anything that appears anywhere in their history below.
- No obvious canon they have plainly already seen. If it is the first thing
  anyone would name for this mood, it is the wrong pick.
- Do not let one loud corner of their history colour every answer. If the picks
  cluster on a single subject, era or sensibility, that is your imagination
  failing, not a read of them — throw the list out and rebuild it.
- Every "why" must cite SPECIFIC ratings of theirs by name ("you gave X a 10
  and bounced off Y, so..."). A "why" that would work for anyone is a failure.
  A "why" whose whole argument is a shared subject — "another vampire one" —
  is also a failure, and is the exact mistake this prompt exists to prevent.
- Deep cuts are welcome; obscurity for its own sake is not. It has to fit.
- Address them as "you". Match their register — if the request is blunt or
  crude, do not answer it like a press release.

"read" is one sentence naming what they are actually in the mood for,
including the part they did not say out loud."#;

const PROMPT_INSTRUCTIONS: &str = r#"The rating history below is one person's. Predict what they are about to
want to watch, and write chips they would actually tap.

Return exactly 3. Each is a MOOD or a vibe, never a title and never a genre
name. They should sound like a person, not a menu:
  "a light comedy?"  ·  "cry all night tonight"  ·  "a cinematography masterpiece"
  "saltburn + ethel cain had a baby"

Make the three genuinely different from each other — one safe and close to
what they already love, one that pushes at an edge of their taste, one that is
a swing. Draw on what they rate highly, and lean into specificity: a chip that
names a texture, a feeling or an unexpected pairing beats a category every time.
Lowercase, no trailing punctuation except a question mark where it fits."#;

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
    Ok(Profile { text: out.join("\n"), exclude, rated: rated.len(), sig, at: Instant::now() })
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

async fn ask(core: &Core, profile: &str, instructions: &str, user: &str, schema: Value, max_tokens: u32, effort: &str) -> AppResult<Value> {
    let key = core.db.secret("anthropic").ok_or_else(|| AppError::msg("Add your Anthropic API key in Settings → Integrations"))?;
    let s = core.settings.read().integrations.clone();
    let mut body = json!({
        "model": s.ai_model,
        "max_tokens": max_tokens,
        "thinking": { "type": "adaptive" },
        "output_config": { "effort": effort, "format": { "type": "json_schema", "schema": schema } },
        "system": [
            { "type": "text", "text": instructions },
            { "type": "text", "text": profile, "cache_control": { "type": "ephemeral", "ttl": "1h" } }
        ],
        "messages": [{ "role": "user", "content": user }]
    });
    let send = |b: &Value, fast: bool| {
        let mut betas = vec!["server-side-fallback-2026-07-01"];
        if fast {
            betas.push("fast-mode-2026-02-01");
        }
        core.http
            .client()
            .post(ANTHROPIC)
            .header("x-api-key", &key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", betas.join(","))
            .timeout(Duration::from_secs(120))
            .json(b)
            .send()
    };
    if s.ai_fast {
        body["speed"] = json!("fast");
    }
    let mut res = send(&body, s.ai_fast).await?;
    // Fast mode is opportunistic: 400/403/429 mean "not available", never
    // "fail the search".
    if s.ai_fast && matches!(res.status().as_u16(), 400 | 403 | 429) {
        body.as_object_mut().map(|o| o.remove("speed"));
        res = send(&body, false).await?;
    }
    // A few spaced retries on transient errors; retrying hard made it worse.
    for attempt in 1..4 {
        let st = res.status().as_u16();
        if st != 403 && st < 500 {
            break;
        }
        tokio::time::sleep(Duration::from_millis((400.0 * 2.4f64.powi(attempt - 1)) as u64)).await;
        res = send(&body, false).await?;
    }
    if !res.status().is_success() {
        let st = res.status();
        let t = res.text().await.unwrap_or_default();
        return Err(AppError::msg(format!("Anthropic HTTP {st}: {}", t.chars().take(240).collect::<String>())));
    }
    let data: Value = res.json().await?;
    if data.get("stop_reason").and_then(|s| s.as_str()) == Some("refusal") {
        return Err(AppError::msg("The model declined that one"));
    }
    let text = data
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|c| c.iter().find(|b| b.get("type").and_then(|t| t.as_str()) == Some("text")))
        .and_then(|b| b.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AppError::msg("Empty answer"))?;
    Ok(serde_json::from_str(text)?)
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
    if !simkl::connected(core) {
        return Err(AppError::msg("Vibe search reads your Simkl ratings — connect Simkl first"));
    }
    let (text, exclude, rated) = profile(core).await?;
    if rated < 5 {
        return Ok(json!({ "read": "Not enough ratings on Simkl yet for this to know you.", "picks": [] }));
    }
    let schema = json!({
        "type": "object",
        "properties": {
            "constraint": { "type": "string", "description": "What the request rules IN and OUT, decided before any title is chosen. One line." },
            "read": { "type": "string", "description": "One sentence on what they are ACTUALLY in the mood for, in their own register." },
            "picks": { "type": "array", "maxItems": 5, "items": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "year": { "type": "integer" },
                    "type": { "type": "string", "enum": ["movie", "series"] },
                    "why": { "type": "string", "description": "One or two sentences citing SPECIFIC ratings of theirs." }
                },
                "required": ["title", "year", "type", "why"],
                "additionalProperties": false
            }}
        },
        "required": ["constraint", "read", "picks"],
        "additionalProperties": false
    });
    let parsed = ask(core, &text, VIBE_INSTRUCTIONS, &format!("Tonight's mood: {q}"), schema, 12000, "high").await?;
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
    log::info!("vibe constraint: {}", parsed.get("constraint").and_then(|c| c.as_str()).unwrap_or(""));
    Ok(json!({ "read": parsed.get("read"), "picks": picks }))
}

pub async fn prompts(core: &Core) -> AppResult<Value> {
    if !simkl::connected(core) || core.db.secret("anthropic").is_none() {
        return Ok(json!({ "prompts": [] }));
    }
    let (text, _, rated) = profile(core).await?;
    if rated < 5 {
        return Ok(json!({ "prompts": [] }));
    }
    let schema = json!({
        "type": "object",
        "properties": { "prompts": { "type": "array", "items": {
            "type": "object",
            "properties": {
                "label": { "type": "string", "description": "Short, lowercase, 2-7 words. What goes on the chip." },
                "q": { "type": "string", "description": "The fuller query this chip should submit." }
            },
            "required": ["label", "q"],
            "additionalProperties": false
        }}},
        "required": ["prompts"],
        "additionalProperties": false
    });
    let parsed = ask(core, &text, PROMPT_INSTRUCTIONS, "Give me three chips for right now.", schema, 6000, "medium").await?;
    Ok(json!({ "prompts": parsed.get("prompts").and_then(|p| p.as_array()).map(|p| p.iter().take(3).cloned().collect::<Vec<_>>()) }))
}
