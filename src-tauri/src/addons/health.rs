//! Probe every addon with one real request and name the slow and dead ones,
//! instead of leaving empty rows to be guessed at. Three at a time: they're
//! third-party servers, and latency stops meaning anything once requests
//! queue behind each other.

use super::{Addons, AddonDescriptor, BUILTIN_TRANSPORT};
use futures::stream::{self, StreamExt};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub transport_url: String,
    /// "ok" | "slow" | "dead" | "skipped"
    pub status: &'static str,
    pub ms: u64,
    pub detail: Option<String>,
}

const SLOW_MS: u64 = 2500;

async fn probe(addons: &Addons, a: AddonDescriptor) -> Health {
    let t0 = Instant::now();
    // Prefer a browsable catalog; otherwise ask meta/stream about a title
    // every sane catalogue has.
    let res = if let Some(c) = a.manifest.catalogs.iter().find(|c| c.is_browsable()) {
        addons.fetch(&a, "catalog", &c.kind, &c.id, &[]).await.map(|_| ())
    } else if a.supports("meta", "movie", Some("tt0111161")) {
        addons.fetch(&a, "meta", "movie", "tt0111161", &[]).await.map(|_| ())
    } else if a.supports("stream", "movie", Some("tt0111161")) {
        addons.fetch(&a, "stream", "movie", "tt0111161", &[]).await.map(|_| ())
    } else {
        return Health { transport_url: a.transport_url, status: "skipped", ms: 0, detail: Some("Nothing safe to probe".into()) };
    };
    let ms = t0.elapsed().as_millis() as u64;
    match res {
        Ok(()) => Health { transport_url: a.transport_url, status: if ms > SLOW_MS { "slow" } else { "ok" }, ms, detail: None },
        Err(e) => Health { transport_url: a.transport_url, status: "dead", ms, detail: Some(e.to_string()) },
    }
}

pub async fn check_all(addons: Arc<Addons>) -> Vec<Health> {
    let list: Vec<AddonDescriptor> =
        addons.effective().into_iter().filter(|a| a.transport_url != BUILTIN_TRANSPORT).collect();
    stream::iter(list).map(|a| { let ad = addons.clone(); async move { probe(&ad, a).await } }).buffer_unordered(3).collect().await
}
