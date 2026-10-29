//! The one HTTP client everything shares, with optional DNS-over-HTTPS.
//!
//! DoH matters here more than it would in most apps: the image hosts and
//! some addon APIs are filtered at the DNS level on common ISPs. Resolving
//! over HTTPS brings them back without routing a single byte through a
//! third-party proxy. If DoH itself fails we fall back to the system
//! resolver, so turning it on can never make things worse.

use crate::db::Db;
use crate::error::{AppError, AppResult};
use hickory_resolver::config::ResolverConfig;
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::TokioResolver;
use parking_lot::RwLock;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use serde::de::DeserializeOwned;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

pub const UA: &str = concat!("Lumen/", env!("CARGO_PKG_VERSION"), " (+stremio-compatible)");

struct DohResolver {
    inner: TokioResolver,
}

impl Resolve for DohResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let r = self.inner.clone();
        Box::pin(async move {
            let host = name.as_str().to_string();
            match r.lookup_ip(host.as_str()).await {
                Ok(ips) => {
                    let v: Vec<SocketAddr> = ips.iter().map(|ip| SocketAddr::new(ip, 0)).collect();
                    if !v.is_empty() {
                        return Ok(Box::new(v.into_iter()) as Addrs);
                    }
                    system_lookup(&host).await
                }
                Err(_) => system_lookup(&host).await,
            }
        })
    }
}

async fn system_lookup(host: &str) -> Result<Addrs, Box<dyn std::error::Error + Send + Sync>> {
    let v: Vec<SocketAddr> = tokio::net::lookup_host((host, 0)).await?.collect();
    Ok(Box::new(v.into_iter()))
}

fn build_client(doh: Option<&str>) -> reqwest::Client {
    let mut b = reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(8))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(8)
        .gzip(true)
        .brotli(true);

    if let Some(provider) = doh {
        let cfg = match provider {
            "google" => ResolverConfig::google_https(),
            "quad9" => ResolverConfig::quad9_https(),
            _ => ResolverConfig::cloudflare_https(),
        };
        let inner = TokioResolver::builder_with_config(cfg, TokioConnectionProvider::default()).build();
        b = b.dns_resolver(Arc::new(DohResolver { inner }));
    }
    b.build().expect("http client")
}

pub struct Http {
    client: RwLock<reqwest::Client>,
    db: Arc<Db>,
}

impl Http {
    pub fn new(db: Arc<Db>, doh: Option<&str>) -> Self {
        Self { client: RwLock::new(build_client(doh)), db }
    }

    /// Rebuild after the DoH setting changes. In-flight requests keep the
    /// client they started with.
    pub fn reconfigure(&self, doh: Option<&str>) {
        *self.client.write() = build_client(doh);
    }

    pub fn client(&self) -> reqwest::Client {
        self.client.read().clone()
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str, timeout: Duration) -> AppResult<T> {
        let res = self.client().get(url).timeout(timeout).send().await?;
        if !res.status().is_success() {
            return Err(AppError::msg(format!("HTTP {} from {}", res.status(), host_of(url))));
        }
        Ok(res.json::<T>().await?)
    }

    /// GET with a persistent cache. A hit never touches the network. Failed
    /// responses are not cached, so a flaky upstream heals on the next call.
    pub async fn get_json_cached<T: DeserializeOwned>(
        &self,
        url: &str,
        ttl: Duration,
        timeout: Duration,
    ) -> AppResult<T> {
        if let Some(body) = self.db.cache_get(url) {
            if let Ok(v) = serde_json::from_slice::<T>(&body) {
                return Ok(v);
            }
        }
        let res = self.client().get(url).timeout(timeout).send().await?;
        if !res.status().is_success() {
            return Err(AppError::msg(format!("HTTP {} from {}", res.status(), host_of(url))));
        }
        let bytes = res.bytes().await?;
        let v = serde_json::from_slice::<T>(&bytes)?;
        self.db.cache_put(url, &bytes, ttl.as_millis() as i64);
        Ok(v)
    }
}

pub fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
        .unwrap_or_else(|| url.chars().take(40).collect())
}
