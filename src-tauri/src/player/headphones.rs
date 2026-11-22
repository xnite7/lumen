//! Headphone surround: each speaker of a 5.1 / 7.1 mix is played through the
//! room response measured for that spot around a listener's head, so on
//! headphones the sound comes from around you (and out of your head), not
//! from inside it. mpv does the work with ffmpeg's `headphone` filter.
//!
//! The responses: SADIE II (University of York), a Neumann KU100 head in a
//! studio, 48 kHz (see assets/SADIE-NOTICE.txt, Apache 2.0).

use crate::error::{AppError, AppResult};
use std::path::{Path, PathBuf};

/// 26 channels: for each speaker below, its left ear then its right ear
const BRIR: &[u8] = include_bytes!("assets/sadie-d1-brir.wav");
const ORDER: [&str; 13] = ["FL", "FR", "FC", "LFE", "BL", "BR", "SL", "SR", "BC", "TFL", "TFR", "TBL", "TBR"];

/// The speakers of mpv's channel layout ("5.1(side)", "7.1", or spelled out
/// "fl-fr-fc-lfe-sl-sr"). None for stereo / mono (nothing to place).
pub fn speakers(layout: &str) -> Option<Vec<&'static str>> {
    let l = layout.trim().to_lowercase();
    let named: Option<&[&'static str]> = match l.as_str() {
        "5.0" => Some(&["FL", "FR", "FC", "BL", "BR"]),
        "5.0(side)" => Some(&["FL", "FR", "FC", "SL", "SR"]),
        "5.1" => Some(&["FL", "FR", "FC", "LFE", "BL", "BR"]),
        "5.1(side)" => Some(&["FL", "FR", "FC", "LFE", "SL", "SR"]),
        "quad" => Some(&["FL", "FR", "BL", "BR"]),
        "quad(side)" => Some(&["FL", "FR", "SL", "SR"]),
        "6.0" => Some(&["FL", "FR", "FC", "BC", "SL", "SR"]),
        "6.1" => Some(&["FL", "FR", "FC", "LFE", "BC", "SL", "SR"]),
        "7.0" => Some(&["FL", "FR", "FC", "BL", "BR", "SL", "SR"]),
        "7.1" => Some(&["FL", "FR", "FC", "LFE", "BL", "BR", "SL", "SR"]),
        _ => None,
    };
    if let Some(n) = named {
        return Some(n.to_vec());
    }
    // spelled out: every channel has to be one we can place
    if l.contains('-') {
        let mut out = vec![];
        for c in l.split('-') {
            let up = c.to_uppercase();
            let s = ORDER.iter().find(|o| **o == up)?;
            out.push(*s);
        }
        return (out.len() > 2).then_some(out);
    }
    None
}

/// The responses for these speakers, in that order, as the filter wants them
/// (one file, two channels per speaker). Written once per layout.
pub fn ir_file(cache: &Path, map: &[&str]) -> AppResult<PathBuf> {
    let path = cache.join(format!("couch-hp-{}.wav", map.join("_").to_lowercase()));
    if path.exists() {
        return Ok(path);
    }
    let bad = || AppError::msg("the headphone surround data is damaged");
    // our own file: 44-byte header, float32, 26 channels
    let data = BRIR.get(44..).ok_or_else(bad)?;
    let all = ORDER.len() * 2;
    let frames = data.len() / (4 * all);
    let cols: Vec<usize> = map.iter().map(|m| ORDER.iter().position(|o| o == m).ok_or_else(bad)).collect::<AppResult<_>>()?;
    let c = cols.len() * 2;
    let mut out = Vec::with_capacity(44 + frames * c * 4);
    let size = (frames * c * 4) as u32;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + size).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes()); // float
    out.extend_from_slice(&(c as u16).to_le_bytes());
    out.extend_from_slice(&48000u32.to_le_bytes());
    out.extend_from_slice(&(48000 * 4 * c as u32).to_le_bytes());
    out.extend_from_slice(&((4 * c) as u16).to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&size.to_le_bytes());
    for f in 0..frames {
        for &col in &cols {
            for ear in 0..2 {
                let i = (f * all + col * 2 + ear) * 4;
                out.extend_from_slice(&data[i..i + 4]);
            }
        }
    }
    std::fs::write(&path, out).map_err(|e| AppError::msg(e.to_string()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn layouts() {
        assert_eq!(super::speakers("7.1").map(|v| v.len()), Some(8));
        assert_eq!(super::speakers("fl-fr-fc-lfe-sl-sr").map(|v| v.len()), Some(6));
        assert!(super::speakers("stereo").is_none());
    }
    #[test]
    fn picks_channels() {
        let dir = std::env::temp_dir();
        let p = super::ir_file(&dir, &["SL", "SR"]).unwrap();
        let b = std::fs::read(p).unwrap();
        assert_eq!(u16::from_le_bytes([b[22], b[23]]), 4);
    }
}
