//! The Simkl worker, brought home.
//!
//! The worker existed because a plugin couldn't answer stremio-core's HTTP
//! requests -- core ran in a Web Worker with its own fetch. Lumen IS the core
//! now, so the same catalogs are answered in-process: no deploy, no public
//! URL, no subrequest budget, and the keys never leave your machine.

use super::types::*;
use super::BUILTIN_TRANSPORT;
use crate::core::Core;
use crate::error::{AppError, AppResult};
use crate::integrations::{motn, simkl, tmdb};
use serde_json::{json, Value};

const SIMKL_ROWS: &[(&str, &str, &[&str])] = &[
    ("simkl-watching", "Watching on Simkl", &["series"]),
    ("simkl-plantowatch", "Plan to Watch", &["movie", "series"]),
    ("simkl-recs", "Because You Watched", &["movie", "series"]),
    ("simkl-surprise", "Surprise Me", &["movie", "series"]),
    ("motn-top", "Top 10 Today", &["movie", "series"]),
];

pub fn descriptor() -> AddonDescriptor {
    let mut catalogs = vec![ManifestCatalog {
        kind: "all".into(),
        id: "search".into(),
        name: Some("Movies & Shows".into()),
        extra: vec![ExtraProp { name: "search".into(), is_required: true, ..Default::default() }],
        extra_supported: vec![],
        extra_required: vec![],
        rest: Default::default(),
    }];
    for (id, name, types) in SIMKL_ROWS {
        for t in *types {
            catalogs.push(ManifestCatalog {
                kind: t.to_string(),
                id: id.to_string(),
                name: Some(name.to_string()),
                extra: vec![ExtraProp { name: "skip".into(), ..Default::default() }],
                extra_supported: vec![],
                extra_required: vec![],
                rest: Default::default(),
            });
        }
    }
    for def in tmdb::CATALOGS {
        for t in ["movie", "series"] {
            let genres = if t == "series" { tmdb::TV_GENRES } else { tmdb::MOVIE_GENRES };
            catalogs.push(ManifestCatalog {
                kind: t.into(),
                id: def.id.into(),
                name: Some(def.name.into()),
                extra: vec![
                    ExtraProp {
                        name: "genre".into(),
                        options: Some(genres.iter().map(|(n, _)| n.to_string()).collect()),
                        ..Default::default()
                    },
                    ExtraProp { name: "skip".into(), ..Default::default() },
                ],
                extra_supported: vec![],
                extra_required: vec![],
                rest: Default::default(),
            });
        }
    }
    AddonDescriptor {
        transport_url: BUILTIN_TRANSPORT.into(),
        transport_name: "builtin".into(),
        manifest: Manifest {
            id: "app.lumen.builtin".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            name: "Lumen".into(),
            description: Some("Simkl lists, TMDB browse rows, Top 10 and richer artwork — built in.".into()),
            logo: None,
            background: None,
            types: vec!["movie".into(), "series".into(), "all".into()],
            catalogs,
            resources: vec![
                ManifestResource::Short("catalog".into()),
                ManifestResource::Full {
                    name: "meta".into(),
                    types: Some(vec!["movie".into(), "series".into()]),
                    id_prefixes: Some(vec!["tt".into()]),
                },
            ],
            id_prefixes: Some(vec!["tt".into()]),
            behavior_hints: json!({ "configurable": false }),
            rest: Default::default(),
        },
        flags: AddonFlags { official: true, protected: true },
    }
}

fn dedupe(v: Vec<Value>) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    v.into_iter().filter(|m| seen.insert(m.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string())).collect()
}

pub async fn handle(core: &Core, resource: &str, kind: &str, id: &str, extra: &[(String, String)]) -> AppResult<Value> {
    let get = |k: &str| extra.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    match resource {
        "meta" => {
            let meta = tmdb::layered_meta(core, kind, id).await?;
            Ok(json!({ "meta": meta }))
        }
        "catalog" => {
            let skip: u32 = get("skip").and_then(|s| s.parse().ok()).unwrap_or(0);
            let metas = match id {
                "search" => {
                    let q = get("search").unwrap_or("");
                    let (multi, anime) = tokio::join!(tmdb::search_multi(core, q), search_anime(core, q));
                    let mut v = multi.unwrap_or_default();
                    v.extend(anime);
                    dedupe(v)
                }
                "simkl-watching" => simkl_list(core, "watching", kind, skip).await?,
                "simkl-plantowatch" => simkl_list(core, "plantowatch", kind, skip).await?,
                "simkl-recs" => recommendations(core, kind, skip).await?,
                "simkl-surprise" => surprise(core, kind).await?,
                "motn-top" => motn::top(core, kind).await?,
                other => {
                    if let Some(def) = tmdb::CATALOGS.iter().find(|d| d.id == other) {
                        tmdb::catalog(core, def, kind, get("genre"), skip).await?
                    } else {
                        return Err(AppError::msg("unknown catalog"));
                    }
                }
            };
            Ok(json!({ "metas": metas }))
        }
        _ => Err(AppError::msg("unsupported")),
    }
}

async fn search_anime(core: &Core, q: &str) -> Vec<Value> {
    if simkl::client_id(core).is_none() {
        return vec![];
    }
    match simkl::get(core, &format!("/search/anime?q={}&extended=full&limit=10", urlencoding::encode(q))).await {
        Ok(Value::Array(rows)) => rows.iter().filter_map(|r| simkl::to_meta(r, None)).collect(),
        _ => vec![],
    }
}

async fn simkl_list(core: &Core, status: &str, kind: &str, skip: u32) -> AppResult<Vec<Value>> {
    if !simkl::connected(core) || skip > 0 {
        return Ok(vec![]);
    }
    let d = simkl::get(core, &format!("/sync/all-items/all/{status}?extended=full")).await?;
    let mut rows = Vec::new();
    for k in ["shows", "movies", "anime"] {
        rows.extend(d.get(k).and_then(|a| a.as_array()).cloned().unwrap_or_default());
    }
    Ok(dedupe(rows.iter().filter_map(|r| simkl::to_meta(r, Some(kind))).collect()))
}

/// TMDB recommendations seeded from what you finished and rated 7+.
async fn recommendations(core: &Core, kind: &str, skip: u32) -> AppResult<Vec<Value>> {
    if !simkl::connected(core) || skip > 0 {
        return Ok(vec![]);
    }
    let d = simkl::get(core, "/sync/all-items/all/completed?extended=full").await?;
    let mut rows = Vec::new();
    for k in ["shows", "movies", "anime"] {
        rows.extend(d.get(k).and_then(|a| a.as_array()).cloned().unwrap_or_default());
    }
    // Most recently finished first, so the row follows what you're into now.
    rows.sort_by(|a, b| {
        b.get("last_watched_at").and_then(|v| v.as_str()).unwrap_or("").cmp(a.get("last_watched_at").and_then(|v| v.as_str()).unwrap_or(""))
    });
    let tkind = tmdb::kind_of(kind);
    let seeds: Vec<String> = rows
        .iter()
        .filter(|r| r.get("user_rating").and_then(|v| v.as_u64()).map_or(true, |r| r >= 7))
        .filter(|r| (if simkl::is_show(r) { "series" } else { "movie" }) == kind)
        .filter_map(|r| simkl::inner(r).pointer("/ids/imdb").and_then(|i| i.as_str()).map(|s| s.to_string()))
        .take(3)
        .collect();
    let mut out = Vec::new();
    for imdb in seeds {
        let Ok(Some(tid)) = tmdb::tmdb_id_for_imdb(core, &imdb, tkind).await else { continue };
        if let Ok(v) = tmdb::get(core, &format!("/{tkind}/{tid}/recommendations"), "", std::time::Duration::from_secs(86_400)).await {
            let rows: Vec<Value> = v.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default().into_iter().take(12).collect();
            out.extend(tmdb::resolve_rows(core, rows, tkind).await);
        }
    }
    Ok(dedupe(out))
}

async fn surprise(core: &Core, kind: &str) -> AppResult<Vec<Value>> {
    if simkl::client_id(core).is_none() {
        return Ok(vec![]);
    }
    let t = if kind == "series" { "tv" } else { "movie" };
    // Not cached: the point is that it rerolls.
    let v = simkl::get(core, &format!("/search/random/?type={t}&rank_limit=500&limit=20")).await?;
    let rows = match v {
        Value::Array(a) => a,
        other => vec![other],
    };
    Ok(dedupe(rows.iter().filter_map(|r| simkl::to_meta(r, Some(kind))).collect()))
}
