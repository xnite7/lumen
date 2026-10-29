//! The Stremio addon protocol, typed only as far as Lumen needs to make
//! decisions. Everything else is kept verbatim in `rest` so nothing an addon
//! sends is ever dropped on the way to the UI.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub catalogs: Vec<ManifestCatalog>,
    #[serde(default)]
    pub resources: Vec<ManifestResource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_prefixes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub behavior_hints: Value,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ManifestResource {
    Short(String),
    #[serde(rename_all = "camelCase")]
    Full {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        types: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id_prefixes: Option<Vec<String>>,
    },
}

impl ManifestResource {
    pub fn name(&self) -> &str {
        match self {
            ManifestResource::Short(n) => n,
            ManifestResource::Full { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestCatalog {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "lenient_extra")]
    pub extra: Vec<ExtraProp>,
    /// Legacy (pre-`extra`) declaration style, still used by some addons.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_supported: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_required: Vec<String>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExtraProp {
    pub name: String,
    #[serde(default)]
    pub is_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_limit: Option<u32>,
}

/// Some addons write `extra: ["search"]`; most write objects. Accept both.
fn lenient_extra<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ExtraProp>, D::Error> {
    let v = Value::deserialize(d)?;
    let mut out = Vec::new();
    if let Value::Array(items) = v {
        for it in items {
            match it {
                Value::String(name) => out.push(ExtraProp { name, ..Default::default() }),
                other => {
                    if let Ok(p) = serde_json::from_value::<ExtraProp>(other) {
                        out.push(p)
                    }
                }
            }
        }
    }
    Ok(out)
}

impl ManifestCatalog {
    pub fn supports_extra(&self, name: &str) -> bool {
        self.extra.iter().any(|e| e.name == name) || self.extra_supported.iter().any(|e| e == name)
    }

    #[allow(dead_code)]
    pub fn requires(&self, name: &str) -> bool {
        self.extra.iter().any(|e| e.name == name && e.is_required) || self.extra_required.iter().any(|e| e == name)
    }

    /// A catalog that can be shown as a plain row: nothing required, or only
    /// requirements we can satisfy with a default (a genre with options).
    pub fn is_browsable(&self) -> bool {
        let legacy_required = !self.extra_required.is_empty();
        let required: Vec<&ExtraProp> = self.extra.iter().filter(|e| e.is_required).collect();
        !legacy_required && required.iter().all(|e| e.name != "search" && e.options.as_ref().map_or(false, |o| !o.is_empty()))
    }
}

/// An installed addon, in the exact shape Stremio's API stores it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonDescriptor {
    pub transport_url: String,
    #[serde(default = "http_transport")]
    pub transport_name: String,
    pub manifest: Manifest,
    #[serde(default)]
    pub flags: AddonFlags,
}

fn http_transport() -> String {
    "http".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddonFlags {
    #[serde(default)]
    pub official: bool,
    #[serde(default)]
    pub protected: bool,
}

impl AddonDescriptor {
    pub fn base_url(&self) -> String {
        base_of(&self.transport_url)
    }

    /// Does this addon answer `resource` for this type and id?
    pub fn supports(&self, resource: &str, kind: &str, id: Option<&str>) -> bool {
        let m = &self.manifest;
        for r in &m.resources {
            if r.name() != resource {
                continue;
            }
            let (types, prefixes) = match r {
                ManifestResource::Short(_) => (Some(&m.types), m.id_prefixes.as_ref()),
                ManifestResource::Full { types, id_prefixes, .. } => {
                    (types.as_ref().or(Some(&m.types)), id_prefixes.as_ref().or(m.id_prefixes.as_ref()))
                }
            };
            let type_ok = types.map_or(true, |t| t.is_empty() || t.iter().any(|x| x == kind));
            let id_ok = match (id, prefixes) {
                (Some(id), Some(p)) if !p.is_empty() => p.iter().any(|pre| id.starts_with(pre.as_str())),
                _ => true,
            };
            if type_ok && id_ok {
                return true;
            }
        }
        false
    }

    pub fn is_adult(&self) -> bool {
        self.manifest.behavior_hints.get("adult").and_then(|v| v.as_bool()).unwrap_or(false)
    }

    #[allow(dead_code)]
    pub fn configurable(&self) -> bool {
        self.manifest.behavior_hints.get("configurable").and_then(|v| v.as_bool()).unwrap_or(false)
    }
}

pub fn base_of(transport_url: &str) -> String {
    let t = transport_url.trim();
    let t = t.strip_suffix("/manifest.json").unwrap_or(t);
    t.trim_end_matches('/').to_string()
}

/// `stremio://host/x/manifest.json` -> `https://host/x/manifest.json`.
pub fn normalize_transport(url: &str) -> String {
    let u = url.trim();
    let u = if let Some(rest) = u.strip_prefix("stremio://") { format!("https://{rest}") } else { u.to_string() };
    if u.ends_with("/manifest.json") {
        u
    } else {
        format!("{}/manifest.json", u.trim_end_matches('/'))
    }
}

/// Encode `extra` the way the protocol wants: `genre=Action&skip=100`.
pub fn encode_extra(extra: &[(String, String)]) -> String {
    extra
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn resource_url(base: &str, resource: &str, kind: &str, id: &str, extra: &[(String, String)]) -> String {
    let id = urlencoding::encode(id);
    let extra = encode_extra(extra);
    if extra.is_empty() {
        format!("{base}/{resource}/{kind}/{id}.json")
    } else {
        format!("{base}/{resource}/{kind}/{id}/{extra}.json")
    }
}

/// A reference to one catalog of one addon, which is what Home rows and
/// Discover filters are made of.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRef {
    pub key: String,
    pub addon_id: String,
    pub addon_name: String,
    pub transport_url: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub name: String,
    pub extra: Vec<ExtraProp>,
    pub adult: bool,
    pub builtin: bool,
}
