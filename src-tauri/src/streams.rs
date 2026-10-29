//! Reads what an addon's stream title is trying to say -- resolution, HDR,
//! codec, size, seeders, debrid cache -- and attaches it as structured data.
//! The UI groups and badges from `_lumen`; ranking here means "best pick" is
//! the same decision everywhere (picker, autoplay, binge).

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Parsed {
    /// "4K" | "1080p" | "720p" | "SD" | "Unknown"
    pub res: &'static str,
    pub res_rank: u8,
    pub hdr: Vec<&'static str>,
    pub codec: Option<&'static str>,
    pub audio: Vec<&'static str>,
    pub source: Option<&'static str>,
    pub size_bytes: Option<u64>,
    pub seeders: Option<u32>,
    /// Debrid-cached (instant) vs needs downloading.
    pub cached: Option<bool>,
    pub debrid: Option<String>,
    /// "debrid" | "torrent" | "http" | "youtube" | "external"
    pub kind: &'static str,
    pub cam: bool,
    pub three_d: bool,
    pub languages: Vec<String>,
    pub group: Option<String>,
    pub label: String,
    pub score: i64,
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: Lazy<Regex> = Lazy::new(|| Regex::new($pat).unwrap());
    };
}

re!(R4K, r"(?i)\b(4k|2160p|uhd)\b");
re!(R1080, r"(?i)\b1080[pi]\b");
re!(R720, r"(?i)\b720p\b");
re!(RSD, r"(?i)\b(480p|576p|360p|sd|dvdrip|dvdscr)\b");
re!(RDV, r"(?i)\b(dv|dovi|dolby[ .]?vision)\b");
re!(RHDR10P, r"(?i)\bhdr10(\+|plus)");
re!(RHDR10, r"(?i)\bhdr10\b");
re!(RHDR, r"(?i)\bhdr\b");
re!(RHEVC, r"(?i)\b(hevc|x265|h\.?265)\b");
re!(RAV1, r"(?i)\bav1\b");
re!(RAVC, r"(?i)\b(avc|x264|h\.?264)\b");
re!(RATMOS, r"(?i)\batmos\b");
re!(RTRUEHD, r"(?i)\btrue[ -]?hd\b");
re!(RDTSHD, r"(?i)\bdts[ -.]?(hd|x|ma)\b");
re!(RDTS, r"(?i)\bdts\b");
re!(RDDP, r"(?i)\b(ddp|dd\+|e-?ac-?3)\s?([257]\.[01])?");
re!(R51, r"(?i)\b([57]\.1)\b");
re!(RREMUX, r"(?i)\bremux\b");
re!(RBLURAY, r"(?i)\b(blu-?ray|bdrip|brrip|bdremux)\b");
re!(RWEBDL, r"(?i)\bweb[- .]?dl\b");
re!(RWEBRIP, r"(?i)\bweb-?rip\b");
re!(RHDTV, r"(?i)\bhdtv\b");
re!(RCAM, r"(?i)\b(cam|camrip|hdcam|ts|telesync|hdts|tc|telecine)\b");
re!(R3D, r"(?i)\b(3d|sbs|h-?sbs|half-?ou)\b");
re!(RSIZE, r"(?i)(?:💾|size[:\s]*)?\s*(\d+(?:[.,]\d+)?)\s*(tb|gb|mb|gib|mib)\b");
re!(RSEED, r"(?i)(?:👤|seeders?[:\s]*|seeds?[:\s]*|🌱)\s*(\d[\d,]*)");
re!(RCACHED, r"(?i)(⚡|\[(rd|ad|pm|tb|dl|oc|ed|pk)\+\]|\binstant\b|\bcached\b|✅)");
re!(RUNCACHED, r"(?i)(\[(rd|ad|pm|tb|dl|oc|ed|pk) download\]|⏳|\buncached\b|\[(rd|ad|pm|tb)\]\s)");
re!(RDEBRID, r"(?i)\b(rd|real-?debrid|ad|alldebrid|pm|premiumize|tb|torbox|dl|debrid-?link|ed|easydebrid|oc|offcloud)\b");
re!(RGROUP, r"-([A-Za-z0-9]{2,12})(?:\.[a-z0-9]{2,4})?\s*$");
re!(RFLAG, r"[\u{1F1E6}-\u{1F1FF}]{2}");

fn text_of(s: &Value) -> String {
    let mut parts = Vec::new();
    for k in ["name", "title", "description"] {
        if let Some(v) = s.get(k).and_then(|v| v.as_str()) {
            parts.push(v.to_string());
        }
    }
    if let Some(f) = s.pointer("/behaviorHints/filename").and_then(|v| v.as_str()) {
        parts.push(f.to_string());
    }
    parts.join("\n")
}

fn size_of(t: &str, s: &Value) -> Option<u64> {
    if let Some(n) = s.pointer("/behaviorHints/videoSize").and_then(|v| v.as_u64()) {
        return Some(n);
    }
    let c = RSIZE.captures(t)?;
    let n: f64 = c[1].replace(',', ".").parse().ok()?;
    let mult = match c[2].to_ascii_lowercase().as_str() {
        "tb" => 1e12,
        "gb" | "gib" => 1e9,
        _ => 1e6,
    };
    Some((n * mult) as u64)
}

pub fn parse(s: &Value) -> Parsed {
    let t = text_of(s);
    let mut p = Parsed::default();

    (p.res, p.res_rank) = if R4K.is_match(&t) {
        ("4K", 4)
    } else if R1080.is_match(&t) {
        ("1080p", 3)
    } else if R720.is_match(&t) {
        ("720p", 2)
    } else if RSD.is_match(&t) {
        ("SD", 1)
    } else {
        ("Unknown", 0)
    };

    if RDV.is_match(&t) {
        p.hdr.push("DV");
    }
    if RHDR10P.is_match(&t) {
        p.hdr.push("HDR10+");
    } else if RHDR10.is_match(&t) {
        p.hdr.push("HDR10");
    } else if RHDR.is_match(&t) {
        p.hdr.push("HDR");
    }

    p.codec = if RAV1.is_match(&t) {
        Some("AV1")
    } else if RHEVC.is_match(&t) {
        Some("HEVC")
    } else if RAVC.is_match(&t) {
        Some("AVC")
    } else {
        None
    };

    if RATMOS.is_match(&t) {
        p.audio.push("Atmos");
    }
    if RTRUEHD.is_match(&t) {
        p.audio.push("TrueHD");
    } else if RDTSHD.is_match(&t) {
        p.audio.push("DTS-HD");
    } else if RDTS.is_match(&t) {
        p.audio.push("DTS");
    } else if RDDP.is_match(&t) {
        p.audio.push("DD+");
    }
    if let Some(c) = R51.captures(&t) {
        p.audio.push(if &c[1] == "7.1" { "7.1" } else { "5.1" });
    }

    p.source = if RREMUX.is_match(&t) {
        Some("REMUX")
    } else if RBLURAY.is_match(&t) {
        Some("BluRay")
    } else if RWEBDL.is_match(&t) {
        Some("WEB-DL")
    } else if RWEBRIP.is_match(&t) {
        Some("WEBRip")
    } else if RHDTV.is_match(&t) {
        Some("HDTV")
    } else {
        None
    };

    p.cam = RCAM.is_match(&t) && p.source.is_none();
    p.three_d = R3D.is_match(&t);
    p.size_bytes = size_of(&t, s);
    p.seeders = RSEED.captures(&t).and_then(|c| c[1].replace(',', "").parse().ok());

    let has_url = s.get("url").and_then(|v| v.as_str()).is_some();
    let has_hash = s.get("infoHash").and_then(|v| v.as_str()).is_some();
    p.kind = if s.get("ytId").is_some() {
        "youtube"
    } else if s.get("externalUrl").is_some() && !has_url {
        "external"
    } else if has_hash {
        "torrent"
    } else if has_url && (RCACHED.is_match(&t) || RDEBRID.is_match(s.get("name").and_then(|v| v.as_str()).unwrap_or(""))) {
        "debrid"
    } else {
        "http"
    };

    if RCACHED.is_match(&t) {
        p.cached = Some(true);
    } else if RUNCACHED.is_match(&t) {
        p.cached = Some(false);
    }
    if let Some(c) = RDEBRID.captures(s.get("name").and_then(|v| v.as_str()).unwrap_or("")) {
        p.debrid = Some(c[1].to_uppercase());
    }

    p.languages = RFLAG.find_iter(&t).map(|m| m.as_str().to_string()).take(6).collect();
    if let Some(f) = s.pointer("/behaviorHints/filename").and_then(|v| v.as_str()) {
        p.group = RGROUP.captures(f).map(|c| c[1].to_string());
    }

    // The first line of the title is usually the release name; fall back to
    // the addon's name column.
    p.label = s
        .get("title")
        .or_else(|| s.get("description"))
        .and_then(|v| v.as_str())
        .and_then(|v| v.lines().next())
        .unwrap_or_else(|| s.get("name").and_then(|v| v.as_str()).unwrap_or("Stream"))
        .trim()
        .to_string();

    p.score = score(&p);
    p
}

/// One number that orders streams the way a person would pick: instant
/// debrid first, then resolution, then the things that make a 4K worth it.
fn score(p: &Parsed) -> i64 {
    let mut v: i64 = 0;
    match p.cached {
        Some(true) => v += 60_000,
        Some(false) => v -= 20_000,
        None => {}
    }
    if p.kind == "debrid" || p.kind == "http" {
        v += 8_000;
    }
    v += p.res_rank as i64 * 10_000;
    if p.hdr.contains(&"DV") {
        v += 900;
    }
    if p.hdr.iter().any(|h| h.starts_with("HDR")) {
        v += 700;
    }
    if p.source == Some("REMUX") {
        v += 800;
    } else if p.source == Some("BluRay") {
        v += 500;
    } else if p.source == Some("WEB-DL") {
        v += 350;
    }
    if p.audio.contains(&"Atmos") {
        v += 300;
    }
    if matches!(p.codec, Some("HEVC") | Some("AV1")) {
        v += 120;
    }
    if let Some(s) = p.seeders {
        // Seeders only matter until there are enough of them.
        v += (s.min(400) as i64) * 12;
        if s == 0 {
            v -= 30_000;
        }
    }
    if p.cam {
        v -= 80_000;
    }
    if p.three_d {
        v -= 40_000;
    }
    v
}

pub fn annotate(mut s: Value, addon_id: &str, addon_name: &str) -> Value {
    let p = parse(&s);
    s["_lumen"] = json!(p);
    s["_addonId"] = json!(addon_id);
    s["_addonName"] = json!(addon_name);
    s
}

/// For binge-watching: find the stream for the next episode that came from
/// the same place as the current one. `bingeGroup` is the protocol's way of
/// saying "these belong together"; failing that, same addon + resolution +
/// release group is the next best thing.
pub fn pick_same_source<'a>(candidates: &'a [Value], previous: &Value) -> Option<&'a Value> {
    let prev_group = previous.pointer("/behaviorHints/bingeGroup").and_then(|v| v.as_str());
    if let Some(g) = prev_group {
        if let Some(hit) = candidates.iter().find(|c| c.pointer("/behaviorHints/bingeGroup").and_then(|v| v.as_str()) == Some(g)) {
            return Some(hit);
        }
    }
    let prev_addon = previous.get("_addonId").and_then(|v| v.as_str());
    let prev_res = previous.pointer("/_lumen/res").and_then(|v| v.as_str());
    let prev_rel = previous.pointer("/_lumen/group").and_then(|v| v.as_str());
    let mut same: Vec<&Value> = candidates
        .iter()
        .filter(|c| c.get("_addonId").and_then(|v| v.as_str()) == prev_addon)
        .filter(|c| c.pointer("/_lumen/res").and_then(|v| v.as_str()) == prev_res)
        .collect();
    if let Some(rel) = prev_rel {
        if let Some(hit) = same.iter().find(|c| c.pointer("/_lumen/group").and_then(|v| v.as_str()) == Some(rel)) {
            return Some(hit);
        }
    }
    same.sort_by_key(|c| -c.pointer("/_lumen/score").and_then(|v| v.as_i64()).unwrap_or(0));
    same.first().copied().or_else(|| best(candidates))
}

pub fn best(candidates: &[Value]) -> Option<&Value> {
    candidates
        .iter()
        .filter(|c| c.pointer("/_lumen/kind").and_then(|v| v.as_str()) != Some("external"))
        .max_by_key(|c| c.pointer("/_lumen/score").and_then(|v| v.as_i64()).unwrap_or(i64::MIN))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_torrentio_style() {
        let s = json!({
            "name": "[RD+] Torrentio\n4k DV",
            "title": "Dune.Part.Two.2024.2160p.UHD.BluRay.REMUX.DV.HDR10.HEVC.TrueHD.Atmos.7.1-FGT\n👤 512 💾 78.4 GB ⚙️ ThePirateBay",
            "url": "https://x/y"
        });
        let p = parse(&s);
        assert_eq!(p.res, "4K");
        assert!(p.hdr.contains(&"DV"));
        assert_eq!(p.source, Some("REMUX"));
        assert_eq!(p.cached, Some(true));
        assert_eq!(p.seeders, Some(512));
        assert_eq!(p.size_bytes, Some(78_400_000_000));
        assert_eq!(p.kind, "debrid");
        assert!(p.audio.contains(&"Atmos"));
    }
}
