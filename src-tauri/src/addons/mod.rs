//! Addon management and the protocol client.
//!
//! Every request is made directly by Lumen, concurrently, with its own
//! timeout -- there is no windowing and no "visible range" bookkeeping, so
//! the bug Search Load All existed for (late addons never being asked)
//! cannot happen here. Results stream to the UI as each addon answers.

pub mod builtin;
pub mod health;
pub mod types;

use crate::core::CoreRef;
use crate::error::{AppError, AppResult};
use crate::streams;
use futures::stream::{FuturesUnordered, StreamExt};
use parking_lot::RwLock;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
pub use types::*;

pub const BUILTIN_TRANSPORT: &str = "lumen://builtin/manifest.json";

const DEFAULT_ADDONS: &[&str] = &[
    "https://v3-cinemeta.strem.io/manifest.json",
    "https://opensubtitles-v3.strem.io/manifest.json",
];

pub struct Addons {
    core: CoreRef,
    list: RwLock<Vec<AddonDescriptor>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunk {
    pub req_id: String,
    pub addon_id: String,
    pub addon_name: String,
    pub streams: Vec<Value>,
    pub error: Option<String>,
    pub ms: u64,
    pub finished: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchChunk {
    pub req_id: String,
    pub catalog: CatalogRef,
    pub metas: Vec<Value>,
    pub error: Option<String>,
    pub finished: bool,
}

impl Addons {
    pub fn new(core: CoreRef) -> Arc<Self> {
        let saved: Vec<AddonDescriptor> = core.db.get("addons").unwrap_or_default();
        Arc::new(Self { core, list: RwLock::new(saved) })
    }

    /// First launch: install the defaults so Home isn't empty.
    pub async fn ensure_defaults(&self) {
        if !self.list.read().is_empty() {
            return;
        }
        for url in DEFAULT_ADDONS {
            if let Ok(d) = self.fetch_descriptor(url).await {
                self.list.write().push(d);
            }
        }
        self.persist();
    }

    fn persist(&self) {
        let _ = self.core.db.set("addons", &*self.list.read());
    }

    pub fn list(&self) -> Vec<AddonDescriptor> {
        self.list.read().clone()
    }

    /// The user's addons plus the built-in one (first, so its rich meta wins
    /// for tt ids -- the same reason the worker asked to be dragged above
    /// Cinemeta).
    pub fn effective(&self) -> Vec<AddonDescriptor> {
        let mut v = Vec::new();
        if self.core.settings.read().integrations.builtin_catalogs {
            v.push(builtin::descriptor());
        }
        v.extend(self.list.read().iter().cloned());
        v
    }

    pub async fn fetch_descriptor(&self, url: &str) -> AppResult<AddonDescriptor> {
        let transport_url = normalize_transport(url);
        let manifest: Manifest = self
            .core
            .http
            .get_json(&transport_url, Duration::from_secs(12))
            .await
            .map_err(|e| AppError::msg(format!("Couldn't read that addon: {e}")))?;
        let official = transport_url.contains(".strem.io/");
        Ok(AddonDescriptor {
            transport_url,
            transport_name: "http".into(),
            manifest,
            flags: AddonFlags { official, protected: false },
        })
    }

    pub async fn install(&self, url: &str) -> AppResult<AddonDescriptor> {
        let d = self.fetch_descriptor(url).await?;
        {
            let mut l = self.list.write();
            // Reinstalling (e.g. after reconfiguring) replaces in place, so the
            // addon keeps its position.
            if let Some(pos) = l.iter().position(|x| x.manifest.id == d.manifest.id) {
                l[pos] = d.clone();
            } else {
                l.push(d.clone());
            }
        }
        self.persist();
        Ok(d)
    }

    pub fn uninstall(&self, transport_url: &str) {
        self.list.write().retain(|a| a.transport_url != transport_url);
        self.persist();
    }

    pub fn reorder(&self, transport_urls: &[String]) {
        let mut l = self.list.write();
        let mut out = Vec::with_capacity(l.len());
        for t in transport_urls {
            if let Some(pos) = l.iter().position(|a| &a.transport_url == t) {
                out.push(l.remove(pos));
            }
        }
        out.extend(l.drain(..));
        *l = out;
        drop(l);
        self.persist();
    }

    pub fn replace_all(&self, list: Vec<AddonDescriptor>) {
        *self.list.write() = list;
        self.persist();
    }

    /// Re-fetch every manifest. Addons change their catalogs over time
    /// (configured addons especially), and Stremio never refreshes them.
    pub async fn refresh_manifests(&self) {
        let list = self.list();
        let mut futs = FuturesUnordered::new();
        for a in list {
            futs.push(async move {
                let fresh = self.fetch_descriptor(&a.transport_url).await.ok();
                (a.transport_url.clone(), fresh)
            });
        }
        let mut updated = Vec::new();
        while let Some((url, fresh)) = futs.next().await {
            if let Some(f) = fresh {
                updated.push((url, f.manifest));
            }
        }
        drop(futs);
        {
            let mut l = self.list.write();
            for (url, m) in updated {
                if let Some(a) = l.iter_mut().find(|a| a.transport_url == url) {
                    a.manifest = m;
                }
            }
        }
        self.persist();
    }

    // ------------------------------------------------------------ fetching

    fn timeout(&self) -> Duration {
        Duration::from_millis(self.core.settings.read().network.addon_timeout_ms.max(2000))
    }

    pub async fn fetch(
        &self,
        addon: &AddonDescriptor,
        resource: &str,
        kind: &str,
        id: &str,
        extra: &[(String, String)],
    ) -> AppResult<Value> {
        if addon.transport_url == BUILTIN_TRANSPORT {
            return builtin::handle(&self.core, resource, kind, id, extra).await;
        }
        let url = resource_url(&addon.base_url(), resource, kind, id, extra);
        // Catalogs and meta are cacheable for a while; streams and subtitles
        // depend on debrid state and must always be fresh.
        let ttl = match resource {
            "catalog" if extra.iter().any(|(k, _)| k == "search") => Some(Duration::from_secs(60 * 30)),
            "catalog" => Some(Duration::from_secs(60 * 20)),
            "meta" => Some(Duration::from_secs(60 * 60 * 6)),
            _ => None,
        };
        match ttl {
            Some(ttl) => self.core.http.get_json_cached(&url, ttl, self.timeout()).await,
            None => self.core.http.get_json(&url, self.timeout()).await,
        }
    }

    fn find(&self, transport_url: &str) -> Option<AddonDescriptor> {
        self.effective().into_iter().find(|a| a.transport_url == transport_url)
    }

    /// All catalogs, in addon order, as row references.
    pub fn catalogs(&self) -> Vec<CatalogRef> {
        let mut out = Vec::new();
        for a in self.effective() {
            let builtin = a.transport_url == BUILTIN_TRANSPORT;
            for c in &a.manifest.catalogs {
                out.push(CatalogRef {
                    key: format!("{}/{}/{}", a.manifest.id, c.kind, c.id),
                    addon_id: a.manifest.id.clone(),
                    addon_name: a.manifest.name.clone(),
                    transport_url: a.transport_url.clone(),
                    kind: c.kind.clone(),
                    id: c.id.clone(),
                    name: c.name.clone().unwrap_or_else(|| c.id.clone()),
                    extra: c.extra.clone(),
                    adult: a.is_adult() || c.kind == "xxx",
                    builtin,
                });
            }
        }
        out
    }

    pub async fn catalog(&self, cref: &CatalogRef, extra: &[(String, String)]) -> AppResult<Vec<Value>> {
        let addon = self.find(&cref.transport_url).ok_or_else(|| AppError::msg("Addon is no longer installed"))?;
        let v = self.fetch(&addon, "catalog", &cref.kind, &cref.id, extra).await?;
        let adult = cref.adult;
        let metas = v
            .get("metas")
            .and_then(|m| m.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.get("id").and_then(|i| i.as_str()).is_some())
            .map(|mut m| {
                if adult {
                    m["adult"] = json!(true);
                }
                m
            })
            .collect();
        Ok(metas)
    }

    /// Meta from the first addon (in order) that answers for this id. A hint
    /// addon (the one the item was clicked from) is asked first, since it is
    /// the only one guaranteed to know a custom id prefix.
    pub async fn meta(&self, kind: &str, id: &str, hint: Option<&str>) -> AppResult<Value> {
        let mut addons: Vec<AddonDescriptor> =
            self.effective().into_iter().filter(|a| a.supports("meta", kind, Some(id))).collect();
        if let Some(h) = hint {
            if let Some(pos) = addons.iter().position(|a| a.transport_url == h) {
                let a = addons.remove(pos);
                addons.insert(0, a);
            }
        }
        let mut last_err = None;
        for a in addons {
            match self.fetch(&a, "meta", kind, id, &[]).await {
                Ok(v) => {
                    if let Some(meta) = v.get("meta").filter(|m| !m.is_null()) {
                        let mut meta = meta.clone();
                        meta["_addon"] = json!(a.transport_url);
                        if a.is_adult() {
                            meta["adult"] = json!(true);
                        }
                        return Ok(meta);
                    }
                }
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| AppError::msg("No addon knows this title")))
    }

    /// Ask every stream addon at once and emit results as they arrive.
    pub async fn streams(self: Arc<Self>, kind: String, id: String, req_id: String) {
        let addons: Vec<AddonDescriptor> =
            self.effective().into_iter().filter(|a| a.supports("stream", &kind, Some(&id))).collect();
        let mut futs = FuturesUnordered::new();
        for a in addons {
            let this = self.clone();
            let (kind, id) = (kind.clone(), id.clone());
            futs.push(async move {
                let t0 = Instant::now();
                let res = this.fetch(&a, "stream", &kind, &id, &[]).await;
                (a, res, t0.elapsed().as_millis() as u64)
            });
        }
        while let Some((a, res, ms)) = futs.next().await {
            let (streams, error) = match res {
                Ok(v) => {
                    let list = v.get("streams").and_then(|s| s.as_array()).cloned().unwrap_or_default();
                    let annotated =
                        list.into_iter().map(|s| streams::annotate(s, &a.manifest.id, &a.manifest.name)).collect();
                    (annotated, None)
                }
                Err(e) => (vec![], Some(e.to_string())),
            };
            self.core.emit(
                "streams",
                StreamChunk {
                    req_id: req_id.clone(),
                    addon_id: a.manifest.id.clone(),
                    addon_name: a.manifest.name.clone(),
                    streams,
                    error,
                    ms,
                    finished: false,
                },
            );
        }
        self.core.emit(
            "streams",
            StreamChunk {
                req_id,
                addon_id: String::new(),
                addon_name: String::new(),
                streams: vec![],
                error: None,
                ms: 0,
                finished: true,
            },
        );
    }

    /// Streams for one id, awaited and ranked. Used for "play next episode
    /// with the same source" where there is no picker on screen.
    pub async fn streams_collect(&self, kind: &str, id: &str) -> Vec<Value> {
        let addons: Vec<AddonDescriptor> =
            self.effective().into_iter().filter(|a| a.supports("stream", kind, Some(id))).collect();
        let futs: FuturesUnordered<_> = addons
            .into_iter()
            .map(|a| async move {
                let r = self.fetch(&a, "stream", kind, id, &[]).await;
                (a, r)
            })
            .collect();
        let results: Vec<_> = futs.collect().await;
        let mut out = Vec::new();
        for (a, r) in results {
            if let Ok(v) = r {
                for s in v.get("streams").and_then(|s| s.as_array()).cloned().unwrap_or_default() {
                    out.push(streams::annotate(s, &a.manifest.id, &a.manifest.name));
                }
            }
        }
        out
    }

    pub async fn subtitles(&self, kind: &str, id: &str, extra: &[(String, String)]) -> Vec<Value> {
        let addons: Vec<AddonDescriptor> =
            self.effective().into_iter().filter(|a| a.supports("subtitles", kind, Some(id))).collect();
        let futs: FuturesUnordered<_> = addons
            .into_iter()
            .map(|a| async move {
                let r = tokio::time::timeout(Duration::from_secs(7), self.fetch(&a, "subtitles", kind, id, extra)).await;
                (a, r)
            })
            .collect();
        let results: Vec<_> = futs.collect().await;
        let mut out = Vec::new();
        for (a, r) in results {
            if let Ok(Ok(v)) = r {
                for mut s in v.get("subtitles").and_then(|s| s.as_array()).cloned().unwrap_or_default() {
                    s["addon"] = json!(a.manifest.name);
                    out.push(s);
                }
            }
        }
        out
    }

    /// Fan a query out to every catalog that declares `search`.
    pub async fn search(self: Arc<Self>, query: String, req_id: String) {
        let cats: Vec<CatalogRef> = self
            .effective()
            .iter()
            .flat_map(|a| {
                a.manifest.catalogs.iter().filter(|c| c.supports_extra("search")).map(move |c| CatalogRef {
                    key: format!("{}/{}/{}", a.manifest.id, c.kind, c.id),
                    addon_id: a.manifest.id.clone(),
                    addon_name: a.manifest.name.clone(),
                    transport_url: a.transport_url.clone(),
                    kind: c.kind.clone(),
                    id: c.id.clone(),
                    name: c.name.clone().unwrap_or_else(|| c.id.clone()),
                    extra: c.extra.clone(),
                    adult: a.is_adult() || c.kind == "xxx",
                    builtin: a.transport_url == BUILTIN_TRANSPORT,
                })
            })
            .collect();
        let mut futs = FuturesUnordered::new();
        for c in cats {
            let this = self.clone();
            let q = query.clone();
            futs.push(async move {
                let r = this.catalog(&c, &[("search".into(), q)]).await;
                (c, r)
            });
        }
        while let Some((catalog, r)) = futs.next().await {
            let (metas, error) = match r {
                Ok(m) => (m, None),
                Err(e) => (vec![], Some(e.to_string())),
            };
            self.core.emit("search", SearchChunk { req_id: req_id.clone(), catalog, metas, error, finished: false });
        }
        self.core.emit(
            "search",
            SearchChunk {
                req_id,
                catalog: CatalogRef {
                    key: String::new(),
                    addon_id: String::new(),
                    addon_name: String::new(),
                    transport_url: String::new(),
                    kind: String::new(),
                    id: String::new(),
                    name: String::new(),
                    extra: vec![],
                    adult: false,
                    builtin: false,
                },
                metas: vec![],
                error: None,
                finished: true,
            },
        );
    }
}
