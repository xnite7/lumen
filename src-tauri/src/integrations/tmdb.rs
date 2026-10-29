//! TMDB: artwork, cast, collections, recommendations, trailers, people.
//!
//! Detail-page enrichment is ONE request (`append_to_response`) after the
//! imdb->tmdb lookup, which is cached for a month because it never changes.
//! Recommendation and filmography tiles carry their TMDB id and are turned
//! into an IMDb id only when clicked -- resolving 40 tiles up front would be
//! 40 requests to open one page.

use crate::core::Core;
use crate::error::{AppError, AppResult};
use serde_json::{json, Value};
use std::time::Duration;

pub const API: &str = "https://api.themoviedb.org/3";
pub const IMG: &str = "https://image.tmdb.org/t/p";

const DAY: Duration = Duration::from_secs(86_400);
const MONTH: Duration = Duration::from_secs(86_400 * 30);
const T: Duration = Duration::from_secs(10);

pub fn img(path: Option<&str>, size: &str) -> Option<String> {
    match path {
        Some(p) if !p.is_empty() && p != "null" => Some(format!("{IMG}/{size}{p}")),
        _ => None,
    }
}

pub fn kind_of(stremio_type: &str) -> &'static str {
    if stremio_type == "series" || stremio_type == "tv" {
        "tv"
    } else {
        "movie"
    }
}

fn url(core: &Core, path: &str, params: &str) -> AppResult<String> {
    let key = core.tmdb_key().ok_or_else(|| AppError::msg("TMDB key missing"))?;
    Ok(format!("{API}{path}?api_key={key}{params}"))
}

pub async fn get(core: &Core, path: &str, params: &str, ttl: Duration) -> AppResult<Value> {
    let u = url(core, path, params)?;
    core.http.get_json_cached(&u, ttl, T).await
}

pub async fn tmdb_id_for_imdb(core: &Core, imdb: &str, kind: &str) -> AppResult<Option<u64>> {
    let v = get(core, &format!("/find/{imdb}"), "&external_source=imdb_id", MONTH).await?;
    let key = if kind == "tv" { "tv_results" } else { "movie_results" };
    let hit = v.get(key).and_then(|a| a.as_array()).and_then(|a| a.first());
    // A series id clicked from a "movie" row (it happens) still resolves.
    let hit = hit.or_else(|| {
        let other = if kind == "tv" { "movie_results" } else { "tv_results" };
        v.get(other).and_then(|a| a.as_array()).and_then(|a| a.first())
    });
    Ok(hit.and_then(|h| h.get("id")).and_then(|i| i.as_u64()))
}

pub async fn imdb_for_tmdb(core: &Core, kind: &str, id: u64) -> AppResult<Option<String>> {
    let v = get(core, &format!("/{kind}/{id}/external_ids"), "", MONTH).await?;
    Ok(v.get("imdb_id").and_then(|i| i.as_str()).filter(|s| s.starts_with("tt")).map(|s| s.to_string()))
}

/// A list/search row turned into a Stremio meta preview. `id` is the tmdb
/// form until resolved; the UI resolves on click via `tmdb_resolve`.
pub fn tile(r: &Value, kind: &str) -> Value {
    let date = r.get("release_date").or_else(|| r.get("first_air_date")).and_then(|v| v.as_str()).unwrap_or("");
    let tid = r.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
    json!({
        "id": format!("tmdb:{kind}:{tid}"),
        "tmdbId": tid,
        "tmdbKind": kind,
        "type": if kind == "tv" { "series" } else { "movie" },
        "name": r.get("title").or_else(|| r.get("name")).and_then(|v| v.as_str()).unwrap_or(""),
        "poster": img(r.get("poster_path").and_then(|v| v.as_str()), "w342"),
        "background": img(r.get("backdrop_path").and_then(|v| v.as_str()), "w1280"),
        "releaseInfo": date.get(0..4).unwrap_or(""),
        "description": r.get("overview"),
        "imdbRating": r.get("vote_average").and_then(|v| v.as_f64()).filter(|v| *v > 0.0).map(|v| format!("{:.1}", v)),
        "character": r.get("character"),
    })
}

fn certification(full: &Value, kind: &str) -> Option<String> {
    // UAE first (you're there), then the US rating everybody recognises.
    let prefs = ["AE", "US", "GB"];
    if kind == "movie" {
        let results = full.pointer("/release_dates/results")?.as_array()?;
        for p in prefs {
            if let Some(r) = results.iter().find(|r| r.get("iso_3166_1").and_then(|v| v.as_str()) == Some(p)) {
                if let Some(c) = r
                    .get("release_dates")
                    .and_then(|d| d.as_array())
                    .and_then(|d| d.iter().filter_map(|x| x.get("certification").and_then(|c| c.as_str())).find(|c| !c.is_empty()))
                {
                    return Some(c.to_string());
                }
            }
        }
    } else {
        let results = full.pointer("/content_ratings/results")?.as_array()?;
        for p in prefs {
            if let Some(c) = results
                .iter()
                .find(|r| r.get("iso_3166_1").and_then(|v| v.as_str()) == Some(p))
                .and_then(|r| r.get("rating"))
                .and_then(|c| c.as_str())
                .filter(|c| !c.is_empty())
            {
                return Some(c.to_string());
            }
        }
    }
    None
}

/// Everything the detail page shows beyond the addon's own meta.
pub async fn enrich(core: &Core, stremio_type: &str, id: &str) -> AppResult<Value> {
    let kind = kind_of(stremio_type);
    let tid = if let Some(rest) = id.strip_prefix("tmdb:") {
        rest.rsplit(':').next().and_then(|s| s.parse().ok())
    } else if id.starts_with("tt") {
        tmdb_id_for_imdb(core, id, kind).await?
    } else {
        None
    };
    let Some(tid) = tid else { return Ok(Value::Null) };

    let extra = if kind == "movie" { "release_dates" } else { "content_ratings,aggregate_credits" };
    let full = get(
        core,
        &format!("/{kind}/{tid}"),
        &format!(
            "&append_to_response=images,videos,credits,recommendations,similar,keywords,external_ids,{extra}&include_image_language=en,null"
        ),
        DAY,
    )
    .await?;

    let logo = full
        .pointer("/images/logos")
        .and_then(|l| l.as_array())
        .and_then(|l| {
            l.iter().find(|x| x.get("iso_639_1").and_then(|v| v.as_str()) == Some("en")).or_else(|| l.first())
        })
        .and_then(|l| l.get("file_path"))
        .and_then(|p| p.as_str());
    let backdrops: Vec<String> = full
        .pointer("/images/backdrops")
        .and_then(|b| b.as_array())
        .map(|b| {
            b.iter()
                .filter(|x| x.get("iso_639_1").map_or(true, |v| v.is_null()))
                .take(12)
                .filter_map(|x| img(x.get("file_path").and_then(|p| p.as_str()), "original"))
                .collect()
        })
        .unwrap_or_default();

    // Aggregate credits for TV (the whole run), plain credits for film.
    let cast_src = if kind == "tv" {
        full.pointer("/aggregate_credits/cast").or_else(|| full.pointer("/credits/cast"))
    } else {
        full.pointer("/credits/cast")
    };
    let cast: Vec<Value> = cast_src
        .and_then(|c| c.as_array())
        .map(|c| {
            c.iter()
                .take(30)
                .map(|p| {
                    let character = p
                        .get("character")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| {
                            p.pointer("/roles/0/character").and_then(|v| v.as_str()).map(|s| s.to_string())
                        });
                    json!({
                        "id": p.get("id"),
                        "name": p.get("name"),
                        "character": character,
                        "profile": img(p.get("profile_path").and_then(|v| v.as_str()), "w185"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let crew = full.pointer("/credits/crew").and_then(|c| c.as_array()).cloned().unwrap_or_default();
    let directors: Vec<Value> =
        crew.iter().filter(|c| c.get("job").and_then(|j| j.as_str()) == Some("Director")).map(|c| c["name"].clone()).collect();
    let writers: Vec<Value> = crew
        .iter()
        .filter(|c| matches!(c.get("job").and_then(|j| j.as_str()), Some("Screenplay") | Some("Writer")))
        .map(|c| c["name"].clone())
        .take(4)
        .collect();
    let creators: Vec<Value> = full
        .get("created_by")
        .and_then(|c| c.as_array())
        .map(|c| c.iter().map(|x| x["name"].clone()).collect())
        .unwrap_or_default();

    let videos: Vec<Value> = full
        .pointer("/videos/results")
        .and_then(|v| v.as_array())
        .map(|v| {
            let mut v: Vec<&Value> =
                v.iter().filter(|x| x.get("site").and_then(|s| s.as_str()) == Some("YouTube")).collect();
            // Trailers first, then teasers, then the rest (featurettes, clips).
            let rank = |t: &str| match t {
                "Trailer" => 0,
                "Teaser" => 1,
                "Clip" => 2,
                "Featurette" => 3,
                _ => 4,
            };
            v.sort_by_key(|x| (rank(x.get("type").and_then(|t| t.as_str()).unwrap_or("")), !x.get("official").and_then(|o| o.as_bool()).unwrap_or(false)));
            v.into_iter()
                .take(16)
                .map(|x| json!({ "key": x["key"], "name": x["name"], "type": x["type"], "official": x["official"] }))
                .collect()
        })
        .unwrap_or_default();

    let list_tiles = |p: &str| -> Vec<Value> {
        full.pointer(p)
            .and_then(|r| r.as_array())
            .map(|r| r.iter().filter(|x| x.get("poster_path").map_or(false, |p| !p.is_null())).take(20).map(|x| tile(x, kind)).collect())
            .unwrap_or_default()
    };
    let mut recommendations = list_tiles("/recommendations/results");
    if recommendations.len() < 6 {
        recommendations.extend(list_tiles("/similar/results"));
        recommendations.truncate(20);
    }

    let keywords: Vec<Value> = full
        .pointer(if kind == "tv" { "/keywords/results" } else { "/keywords/keywords" })
        .and_then(|k| k.as_array())
        .map(|k| k.iter().take(12).map(|x| x["name"].clone()).collect())
        .unwrap_or_default();

    let mut collection = Value::Null;
    if let Some(cid) = full.pointer("/belongs_to_collection/id").and_then(|v| v.as_u64()) {
        if let Ok(c) = get(core, &format!("/collection/{cid}"), "", DAY).await {
            let mut parts: Vec<Value> = c.get("parts").and_then(|p| p.as_array()).cloned().unwrap_or_default();
            parts.sort_by(|a, b| {
                a.get("release_date").and_then(|v| v.as_str()).unwrap_or("9").cmp(b.get("release_date").and_then(|v| v.as_str()).unwrap_or("9"))
            });
            collection = json!({
                "id": cid,
                "name": c.get("name"),
                "overview": c.get("overview"),
                "backdrop": img(c.get("backdrop_path").and_then(|v| v.as_str()), "w1280"),
                "parts": parts.iter().map(|p| tile(p, "movie")).collect::<Vec<_>>(),
            });
        }
    }

    let seasons: Vec<Value> = full
        .get("seasons")
        .and_then(|s| s.as_array())
        .map(|s| {
            s.iter()
                .map(|x| {
                    json!({
                        "number": x["season_number"],
                        "name": x["name"],
                        "poster": img(x.get("poster_path").and_then(|v| v.as_str()), "w342"),
                        "episodes": x["episode_count"],
                        "airDate": x["air_date"],
                        "overview": x["overview"],
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(json!({
        "tmdbId": tid,
        "tmdbKind": kind,
        "imdbId": full.pointer("/external_ids/imdb_id").or_else(|| full.get("imdb_id")),
        "logo": img(logo, "w500"),
        "backdrop": img(full.get("backdrop_path").and_then(|v| v.as_str()), "original").or_else(|| backdrops.first().cloned()),
        "backdrops": backdrops,
        "poster": img(full.get("poster_path").and_then(|v| v.as_str()), "w500"),
        "tagline": full.get("tagline").filter(|t| t.as_str().map_or(false, |s| !s.is_empty())),
        "overview": full.get("overview"),
        "genres": full.get("genres").and_then(|g| g.as_array()).map(|g| g.iter().map(|x| x["name"].clone()).collect::<Vec<_>>()),
        "runtime": full.get("runtime").or_else(|| full.pointer("/episode_run_time/0")),
        "status": full.get("status"),
        "released": full.get("release_date").or_else(|| full.get("first_air_date")),
        "lastAired": full.get("last_air_date"),
        "networks": full.get("networks").and_then(|n| n.as_array()).map(|n| n.iter().map(|x| json!({"name": x["name"], "logo": img(x.get("logo_path").and_then(|v| v.as_str()), "w154")})).collect::<Vec<_>>()),
        "studios": full.get("production_companies").and_then(|n| n.as_array()).map(|n| n.iter().take(3).map(|x| x["name"].clone()).collect::<Vec<_>>()),
        "certification": certification(&full, kind),
        "rating": full.get("vote_average"),
        "votes": full.get("vote_count"),
        "budget": full.get("budget").filter(|b| b.as_u64().unwrap_or(0) > 0),
        "revenue": full.get("revenue").filter(|b| b.as_u64().unwrap_or(0) > 0),
        "cast": cast,
        "directors": directors,
        "writers": writers,
        "creators": creators,
        "videos": videos,
        "recommendations": recommendations,
        "keywords": keywords,
        "collection": collection,
        "seasons": seasons,
        "spokenLanguages": full.get("spoken_languages").and_then(|n| n.as_array()).map(|n| n.iter().map(|x| x["english_name"].clone()).collect::<Vec<_>>()),
    }))
}

/// Best trailer for a hover card or the hero: a YouTube key.
pub async fn trailer(core: &Core, stremio_type: &str, id: &str) -> AppResult<Option<String>> {
    let kind = kind_of(stremio_type);
    let tid = if id.starts_with("tt") { tmdb_id_for_imdb(core, id, kind).await? } else { None };
    if let Some(tid) = tid {
        let v = get(core, &format!("/{kind}/{tid}/videos"), "", DAY).await?;
        let list = v.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default();
        let yt: Vec<&Value> = list.iter().filter(|x| x.get("site").and_then(|s| s.as_str()) == Some("YouTube")).collect();
        let pick = yt
            .iter()
            .find(|x| x.get("type").and_then(|t| t.as_str()) == Some("Trailer") && x.get("official").and_then(|o| o.as_bool()) == Some(true))
            .or_else(|| yt.iter().find(|x| x.get("type").and_then(|t| t.as_str()) == Some("Trailer")))
            .or_else(|| yt.iter().find(|x| x.get("type").and_then(|t| t.as_str()) == Some("Teaser")))
            .or_else(|| yt.first());
        if let Some(k) = pick.and_then(|p| p.get("key")).and_then(|k| k.as_str()) {
            return Ok(Some(k.to_string()));
        }
    }
    // KinoCheck: free, keyed by IMDb id, official trailers only.
    if id.starts_with("tt") && core.settings.read().integrations.kinocheck_trailers {
        let path = if kind == "tv" { "shows" } else { "movies" };
        let u = format!("https://api.kinocheck.com/{path}?imdb_id={id}");
        if let Ok(v) = core.http.get_json_cached::<Value>(&u, DAY * 7, T).await {
            if let Some(k) = v.pointer("/trailer/youtube_video_id").and_then(|k| k.as_str()) {
                return Ok(Some(k.to_string()));
            }
        }
    }
    Ok(None)
}

pub async fn person(core: &Core, name: &str, tmdb_id: Option<u64>) -> AppResult<Value> {
    let pid = match tmdb_id {
        Some(id) => id,
        None => {
            let found = get(core, "/search/person", &format!("&query={}", urlencoding::encode(name)), DAY).await?;
            let wanted = name.trim().to_lowercase();
            let results = found.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default();
            let hit = results
                .iter()
                .find(|p| p.get("name").and_then(|n| n.as_str()).map(|n| n.trim().to_lowercase()) == Some(wanted.clone()))
                .or_else(|| results.first())
                .ok_or_else(|| AppError::msg("No one by that name on TMDB"))?;
            hit.get("id").and_then(|i| i.as_u64()).unwrap_or(0)
        }
    };
    let p = get(core, &format!("/person/{pid}"), "&append_to_response=images,combined_credits,external_ids", DAY).await?;
    let mut credits: Vec<Value> = p
        .pointer("/combined_credits/cast")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.get("poster_path").map_or(false, |x| !x.is_null()))
        .filter(|c| matches!(c.get("media_type").and_then(|m| m.as_str()), Some("movie") | Some("tv")))
        .filter(|c| {
            let ch = c.get("character").and_then(|v| v.as_str()).unwrap_or("");
            !(ch.starts_with("Self") || ch.starts_with("Himself") || ch.starts_with("Herself") || ch.starts_with("Themselves"))
        })
        .collect();
    // Popular work first is what people actually want from a cast page; the
    // full chronology is one tap away in the UI.
    credits.sort_by(|a, b| {
        b.get("vote_count").and_then(|v| v.as_f64()).unwrap_or(0.0).partial_cmp(&a.get("vote_count").and_then(|v| v.as_f64()).unwrap_or(0.0)).unwrap()
    });
    let mut seen = std::collections::HashSet::new();
    let credits: Vec<Value> = credits
        .into_iter()
        .filter(|c| seen.insert(c.get("id").and_then(|i| i.as_u64()).unwrap_or(0)))
        .take(40)
        .map(|c| {
            let kind = c.get("media_type").and_then(|m| m.as_str()).unwrap_or("movie").to_string();
            tile(&c, &kind)
        })
        .collect();
    Ok(json!({
        "source": "tmdb",
        "id": pid,
        "name": p.get("name"),
        "bio": p.get("biography"),
        "birthday": p.get("birthday"),
        "deathday": p.get("deathday"),
        "birthplace": p.get("place_of_birth"),
        "knownFor": p.get("known_for_department"),
        "aliases": p.get("also_known_as"),
        "profile": img(p.get("profile_path").and_then(|v| v.as_str()), "h632"),
        "gallery": p.pointer("/images/profiles").and_then(|i| i.as_array()).map(|i| i.iter().take(16).filter_map(|x| img(x.get("file_path").and_then(|v| v.as_str()), "w342")).collect::<Vec<_>>()),
        "imdbId": p.pointer("/external_ids/imdb_id"),
        "instagram": p.pointer("/external_ids/instagram_id"),
        "credits": credits,
    }))
}

/// TMDB browse rows, ported from the Simkl worker.
pub struct CatalogDef {
    pub id: &'static str,
    pub name: &'static str,
    pub movie: &'static str,
    pub series: &'static str,
}

pub const CATALOGS: &[CatalogDef] = &[
    CatalogDef { id: "tmdb-trending", name: "Trending This Week", movie: "/trending/movie/week", series: "/trending/tv/week" },
    CatalogDef { id: "tmdb-popular", name: "Popular", movie: "/movie/popular", series: "/tv/popular" },
    CatalogDef { id: "tmdb-top", name: "Top Rated", movie: "/movie/top_rated", series: "/tv/top_rated" },
    CatalogDef { id: "tmdb-new", name: "New Releases", movie: "/movie/now_playing", series: "/tv/on_the_air" },
];

pub const MOVIE_GENRES: &[(&str, u32)] = &[
    ("Action", 28), ("Adventure", 12), ("Animation", 16), ("Comedy", 35), ("Crime", 80),
    ("Documentary", 99), ("Drama", 18), ("Family", 10751), ("Fantasy", 14), ("History", 36),
    ("Horror", 27), ("Music", 10402), ("Mystery", 9648), ("Romance", 10749),
    ("Science Fiction", 878), ("Thriller", 53), ("War", 10752), ("Western", 37),
];
pub const TV_GENRES: &[(&str, u32)] = &[
    ("Action & Adventure", 10759), ("Animation", 16), ("Comedy", 35), ("Crime", 80),
    ("Documentary", 99), ("Drama", 18), ("Family", 10751), ("Kids", 10762), ("Mystery", 9648),
    ("Reality", 10764), ("Sci-Fi & Fantasy", 10765), ("Soap", 10766), ("Talk", 10767),
    ("War & Politics", 10768), ("Western", 37),
];

/// Resolve a page of TMDB results to IMDb-keyed metas, preserving TMDB's
/// order (the ranking IS the catalog). Rows with no IMDb id are dropped:
/// nothing downstream can find streams for them.
pub async fn resolve_rows(core: &Core, rows: Vec<Value>, kind: &str) -> Vec<Value> {
    use futures::stream::{self, StreamExt};
    let genres = if kind == "tv" { TV_GENRES } else { MOVIE_GENRES };
    let out: Vec<Option<Value>> = stream::iter(rows.into_iter())
        .map(|r| async move {
            let k = r.get("media_type").and_then(|m| m.as_str()).map(|m| if m == "tv" { "tv" } else { "movie" }).unwrap_or(kind);
            let tid = r.get("id").and_then(|i| i.as_u64())?;
            let imdb = imdb_for_tmdb(core, k, tid).await.ok().flatten()?;
            let date = r.get("release_date").or_else(|| r.get("first_air_date")).and_then(|v| v.as_str()).unwrap_or("");
            let g: Vec<&str> = r
                .get("genre_ids")
                .and_then(|g| g.as_array())
                .map(|ids| ids.iter().filter_map(|i| i.as_u64()).filter_map(|i| genres.iter().find(|(_, gid)| *gid as u64 == i).map(|(n, _)| *n)).collect())
                .unwrap_or_default();
            Some(json!({
                "id": imdb,
                "type": if k == "tv" { "series" } else { "movie" },
                "name": r.get("title").or_else(|| r.get("name")),
                "poster": img(r.get("poster_path").and_then(|v| v.as_str()), "w342").unwrap_or_else(|| format!("https://images.metahub.space/poster/medium/{imdb}/img")),
                "posterShape": "poster",
                "background": img(r.get("backdrop_path").and_then(|v| v.as_str()), "w1280"),
                "logo": format!("https://images.metahub.space/logo/medium/{imdb}/img"),
                "description": r.get("overview"),
                "releaseInfo": date.get(0..4).unwrap_or(""),
                "released": if date.is_empty() { Value::Null } else { json!(format!("{date}T00:00:00.000Z")) },
                "genres": g,
                "imdbRating": r.get("vote_average").and_then(|v| v.as_f64()).filter(|v| *v > 0.0).map(|v| format!("{:.1}", v)),
            }))
        })
        .buffered(8)
        .collect()
        .await;
    out.into_iter().flatten().collect()
}

pub async fn catalog(core: &Core, def: &CatalogDef, stremio_type: &str, genre: Option<&str>, skip: u32) -> AppResult<Vec<Value>> {
    let kind = kind_of(stremio_type);
    let page = skip / 20 + 1;
    let genres = if kind == "tv" { TV_GENRES } else { MOVIE_GENRES };
    let gid = genre.and_then(|g| genres.iter().find(|(n, _)| *n == g).map(|(_, id)| *id));
    let v = match gid {
        Some(gid) => get(core, &format!("/discover/{kind}"), &format!("&sort_by=popularity.desc&with_genres={gid}&page={page}"), Duration::from_secs(3600)).await?,
        None => {
            let path = if kind == "tv" { def.series } else { def.movie };
            get(core, path, &format!("&page={page}"), Duration::from_secs(3600)).await?
        }
    };
    let rows = v.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    Ok(resolve_rows(core, rows, kind).await)
}

pub async fn search_multi(core: &Core, q: &str) -> AppResult<Vec<Value>> {
    let v = get(core, "/search/multi", &format!("&include_adult=false&query={}", urlencoding::encode(q)), Duration::from_secs(1800)).await?;
    let rows: Vec<Value> = v
        .get("results")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|r| matches!(r.get("media_type").and_then(|m| m.as_str()), Some("movie") | Some("tv")))
        .take(20)
        .collect();
    Ok(resolve_rows(core, rows, "movie").await)
}

/// Cinemeta is the base (it owns `videos`, which is what makes an episode
/// list exist) and TMDB is layered on top for art and trailer.
pub async fn layered_meta(core: &Core, stremio_type: &str, imdb: &str) -> AppResult<Value> {
    let base: Option<Value> = core
        .http
        .get_json_cached::<Value>(&format!("https://v3-cinemeta.strem.io/meta/{stremio_type}/{imdb}.json"), DAY, T)
        .await
        .ok()
        .and_then(|v| v.get("meta").cloned())
        .filter(|m| !m.is_null());
    let kind = kind_of(stremio_type);
    let Ok(Some(tid)) = tmdb_id_for_imdb(core, imdb, kind).await else {
        return base.ok_or_else(|| AppError::msg("not found"));
    };
    let full = match get(core, &format!("/{kind}/{tid}"), "&append_to_response=images,videos&include_image_language=en,null", DAY).await {
        Ok(f) => f,
        Err(_) => return base.ok_or_else(|| AppError::msg("not found")),
    };
    let mut meta = base.unwrap_or_else(|| json!({ "id": imdb, "type": stremio_type, "name": full.get("title").or_else(|| full.get("name")) }));
    if let Some(l) = full.pointer("/images/logos/0/file_path").and_then(|p| p.as_str()) {
        meta["logo"] = json!(img(Some(l), "w500"));
    }
    let back = full.pointer("/images/backdrops/0/file_path").and_then(|p| p.as_str()).or_else(|| full.get("backdrop_path").and_then(|p| p.as_str()));
    if let Some(b) = back {
        meta["background"] = json!(img(Some(b), "original"));
    }
    if let Some(p) = full.get("poster_path").and_then(|p| p.as_str()) {
        meta["poster"] = json!(img(Some(p), "w500"));
    }
    if meta.get("description").map_or(true, |d| d.is_null()) {
        meta["description"] = full["overview"].clone();
    }
    if let Some(t) = full.get("tagline").and_then(|t| t.as_str()).filter(|t| !t.is_empty()) {
        meta["tagline"] = json!(t);
    }
    if meta.get("trailers").and_then(|t| t.as_array()).map_or(true, |t| t.is_empty()) {
        if let Some(k) = full
            .pointer("/videos/results")
            .and_then(|v| v.as_array())
            .and_then(|v| v.iter().find(|x| x.get("site").and_then(|s| s.as_str()) == Some("YouTube") && x.get("type").and_then(|t| t.as_str()) == Some("Trailer")))
            .and_then(|x| x.get("key"))
        {
            meta["trailers"] = json!([{ "source": k, "type": "Trailer" }]);
        }
    }
    Ok(meta)
}
