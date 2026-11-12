//! Format badges and language flags from the Kingsize Fusion Badges pack
//! (github.com/kingsizew/badges), top three tiers only.
use serde_json::Value;

const RAW: &str = "https://raw.githubusercontent.com/kingsizew/badges/main/badge-images/";

pub struct Badge {
    pub src: String,
    pub alt: &'static str,
}

fn b(p: &str, alt: &'static str) -> Badge {
    Badge { src: format!("{RAW}{p}.png"), alt }
}

/// badges for what's playing: `c` = couch's parse of the stream (`_couch`),
/// `height` = the real video height, `audio_codec` = the chosen audio track's codec
pub fn format(c: &Value, height: i64, audio_codec: &str) -> Vec<Badge> {
    let list = |k: &str| -> Vec<String> { c[k].as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default() };
    let (hdr, aud) = (list("hdr"), list("audio"));
    let has = |l: &Vec<String>, s: &str| l.iter().any(|x| x == s);
    let res = c["res"].as_str().unwrap_or("");
    let mut out = vec![];
    if height >= 2000 || res == "4K" {
        out.push(b("resolution/4k", "4K"));
    } else if height >= 1400 {
        out.push(b("resolution/1440p", "1440P"));
    } else if height >= 1000 || res == "1080p" {
        out.push(b("resolution/1080p", "1080P"));
    }
    let (dv, p, h10, any) = (has(&hdr, "DV"), has(&hdr, "HDR10+"), has(&hdr, "HDR10"), has(&hdr, "HDR"));
    let visual = if dv {
        Some(if p { ("visual/dolby-vision-hdr10-plus", "Dolby Vision · HDR10+") } else if h10 { ("visual/dolby-vision-hdr10", "Dolby Vision · HDR10") } else if any { ("visual/dolby-vision-hdr", "Dolby Vision · HDR") } else { ("visual/dolby-vision", "Dolby Vision") })
    } else if p {
        Some(("visual/hdr10-plus", "HDR10+"))
    } else if h10 {
        Some(("visual/hdr10", "HDR10"))
    } else if any {
        Some(("visual/hdr", "HDR"))
    } else {
        None
    };
    if let Some((s, a)) = visual {
        out.push(b(s, a));
    }
    let ac = audio_codec.to_lowercase();
    let atmos = has(&aud, "Atmos");
    let truehd = has(&aud, "TrueHD") || ac == "truehd";
    let ddp = has(&aud, "DD+") || ac == "eac3";
    let (dtsx, dtsma) = (has(&aud, "DTS:X"), has(&aud, "DTS-HD MA"));
    let dtshd = dtsma || has(&aud, "DTS-HD") || ac == "dts-hd";
    let dolby = if atmos {
        Some(if truehd { ("audio/dolby-atmos-truehd", "Dolby Atmos · TrueHD") } else if ddp { ("audio/dolby-atmos-digital-plus", "Dolby Atmos · Digital Plus") } else { ("audio/dolby-atmos", "Dolby Atmos") })
    } else if truehd {
        Some(("audio/dolby-truehd", "Dolby TrueHD"))
    } else if ddp {
        Some(("audio/dolby-digital-plus", "Dolby Digital Plus"))
    } else {
        None
    };
    let dts = if dtsx {
        Some(if dtsma { ("audio/dts-x-hd-ma", "DTS:X · HD MA") } else if dtshd { ("audio/dts-x-hd", "DTS:X · HD") } else { ("audio/dts-x", "DTS:X") })
    } else if dtsma {
        Some(("audio/dts-hd-ma", "DTS-HD MA"))
    } else if dtshd {
        Some(("audio/dts-hd", "DTS-HD"))
    } else {
        None
    };
    let none = dolby.is_none() && dts.is_none();
    for (s, a) in dolby.into_iter().chain(dts) {
        out.push(b(s, a));
    }
    if none && (has(&aud, "FLAC") || ac == "flac") {
        out.push(b("audio/flac", "FLAC"));
    }
    out
}

/// TMDB watch-provider id → the pack's service logo
pub fn service(provider: i64) -> Option<String> {
    let n = match provider {
        8 => "netflix",
        1899 | 384 => "hbo-max",
        337 => "disney-plus",
        9 | 119 => "prime-video",
        350 => "apple-tv-plus",
        15 => "hulu",
        531 => "paramount-plus",
        386 => "peacock",
        283 => "crunchyroll",
        _ => return None,
    };
    Some(format!("{RAW}streaming/{n}.png"))
}

/// a track's language → its flag
pub fn flag(code: &str) -> Option<String> {
    let k = code.to_lowercase();
    let k = k.trim();
    let name = |k: &str| -> Option<&'static str> {
        Some(match k {
            "ar" | "ara" => "arabic",
            "bn" | "ben" => "bengali",
            "bg" | "bul" => "bulgarian",
            "zh" | "chi" | "zho" | "zh-cn" | "zh-tw" | "chs" | "cht" => "chinese",
            "hr" | "hrv" => "croatian",
            "cs" | "cze" | "ces" => "czech",
            "da" | "dan" => "danish",
            "nl" | "dut" | "nld" => "dutch",
            "en" | "eng" | "en-us" | "en-gb" => "english",
            "et" | "est" => "estonian",
            "fi" | "fin" => "finnish",
            "fr" | "fre" | "fra" | "fr-ca" => "french",
            "de" | "ger" | "deu" => "german",
            "el" | "gre" | "ell" => "greek",
            "gu" | "guj" => "gujarati",
            "he" | "heb" | "iw" => "hebrew",
            "hi" | "hin" => "hindi",
            "hu" | "hun" => "hungarian",
            "id" | "ind" => "indonesian",
            "it" | "ita" => "italian",
            "ja" | "jpn" => "japanese",
            "kn" | "kan" => "kannada",
            "ko" | "kor" => "korean",
            "es-419" | "es-la" | "es-mx" | "lat" => "latino",
            "lv" | "lav" => "latvian",
            "lt" | "lit" => "lithuanian",
            "ms" | "may" | "msa" => "malay",
            "ml" | "mal" => "malayalam",
            "mr" | "mar" => "marathi",
            "no" | "nor" | "nb" | "nob" | "nn" => "norwegian",
            "fa" | "per" | "fas" => "persian",
            "pl" | "pol" => "polish",
            "pt-br" | "pob" | "pt_br" => "portuguese-brazil",
            "pt" | "por" | "pt-pt" => "portuguese",
            "pa" | "pan" => "punjabi",
            "ro" | "rum" | "ron" => "romanian",
            "ru" | "rus" => "russian",
            "sr" | "srp" | "scc" => "serbian",
            "sk" | "slo" | "slk" => "slovak",
            "sl" | "slv" => "slovenian",
            "es" | "spa" | "es-es" => "spanish",
            "sv" | "swe" => "swedish",
            "ta" | "tam" => "tamil",
            "te" | "tel" => "telugu",
            "th" | "tha" => "thai",
            "tr" | "tur" => "turkish",
            "uk" | "ukr" => "ukrainian",
            "vi" | "vie" => "vietnamese",
            _ => return None,
        })
    };
    let n = name(k).or_else(|| name(k.split(['-', '_']).next().unwrap_or("")))?;
    Some(format!("{RAW}language/{n}.png"))
}
