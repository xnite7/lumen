//! Subtitle auto-sync: listen to a stretch of the film, find where people
//! talk, and line the subtitle file's lines up with it.
//!
//! - audio: ffmpeg (shipped beside couch.exe) reads ~2.5 minutes from where
//!   you are, as 16 kHz mono, voice band only
//! - speech: 10 ms frames, louder than the quiet parts by a margin = talking
//! - match: every shift of ±60 s (10 ms steps), and the usual frame-rate
//!   mix-ups (23.976 / 24 / 25), scored by how well "subtitle on screen"
//!   lines up with "someone talking"
//! - result: a retimed copy of the subtitle file (the original bytes, only
//!   the timestamps rewritten, so any text encoding survives), loaded and picked

use crate::error::{AppError, AppResult};
use std::path::PathBuf;

/// what's playing, as the sync needs it
pub struct Source {
    pub url: String,
    pub headers: String,
    pub ua: String,
    pub pos: f64,
    /// the chosen subtitle file (a URL or a path)
    pub sub: String,
}

pub struct Found {
    /// video time = ratio × subtitle time + offset
    pub offset: f64,
    pub ratio: f64,
    /// how sure (how far the best match stands above the rest)
    pub z: f64,
    pub file: PathBuf,
}

const FRAME: f64 = 0.01;
const WINDOW: f64 = 150.;
const SEARCH: f64 = 60.;

pub fn ffmpeg() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    for n in ["ffmpeg.exe", "ffmpeg"] {
        let p = dir.join(n);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// The talking in [start, start + WINDOW): one flag per 10 ms.
fn speech(ff: &PathBuf, src: &Source, start: f64) -> AppResult<Vec<bool>> {
    let mut cmd = std::process::Command::new(ff);
    cmd.args(["-nostdin", "-hide_banner", "-loglevel", "error"]);
    if src.url.starts_with("http") {
        if !src.ua.is_empty() {
            cmd.args(["-user_agent", &src.ua]);
        }
        if !src.headers.is_empty() {
            // mpv's list is comma-separated (commas inside escaped); ffmpeg wants CRLF lines
            let h: String = src.headers.replace("\\,", "\u{1}").split(',').map(|x| format!("{}\r\n", x.replace('\u{1}', ","))).collect();
            cmd.args(["-headers", &h]);
        }
    }
    cmd.args(["-ss", &format!("{start:.2}"), "-i", &src.url, "-t", &format!("{WINDOW}"), "-vn", "-sn", "-dn", "-ac", "1", "-ar", "16000", "-af", "highpass=f=250,lowpass=f=3400", "-f", "s16le", "pipe:1"]);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no console window
    }
    let out = cmd.output().map_err(|e| AppError::msg(format!("couldn't start ffmpeg: {e}")))?;
    if out.stdout.len() < 16000 * 2 * 20 {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::msg(format!("couldn't hear enough of the film{}", if err.trim().is_empty() { String::new() } else { format!(" ({})", err.lines().last().unwrap_or("").trim()) })));
    }
    let samples: Vec<f32> = out.stdout.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.).collect();
    let per = 160; // 10 ms at 16 kHz
    let db: Vec<f32> = samples
        .chunks(per)
        .map(|c| {
            let e = c.iter().map(|x| x * x).sum::<f32>() / c.len().max(1) as f32;
            10. * (e + 1e-10).log10()
        })
        .collect();
    let mut sorted = db.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let floor = sorted[sorted.len() / 5];
    let loud = sorted[sorted.len() * 95 / 100];
    // between the quiet floor and the loud end: talking sits above ~40% of the way
    let th = floor + (loud - floor) * 0.4;
    let mut s: Vec<bool> = db.iter().map(|d| *d > th).collect();
    // close little gaps inside a sentence, drop clicks
    fill(&mut s, true, 20);
    fill(&mut s, false, 12);
    Ok(s)
}

/// runs of `!v` shorter than `n` frames become `v`
fn fill(s: &mut [bool], v: bool, n: usize) {
    let mut i = 0;
    while i < s.len() {
        if s[i] == v {
            i += 1;
            continue;
        }
        let a = i;
        while i < s.len() && s[i] != v {
            i += 1;
        }
        let bounded = a > 0 && i < s.len();
        if bounded && i - a < n {
            s[a..i].iter_mut().for_each(|x| *x = v);
        }
    }
}

/// a timestamp: 01:02:03,456 / 01:02:03.456 / 02:03.456
fn ts(t: &str) -> Option<f64> {
    let t = t.trim().replace(',', ".");
    let p: Vec<&str> = t.split(':').collect();
    let (h, m, s) = match p.len() {
        3 => (p[0].parse::<f64>().ok()?, p[1].parse::<f64>().ok()?, p[2].parse::<f64>().ok()?),
        2 => (0., p[0].parse::<f64>().ok()?, p[1].parse::<f64>().ok()?),
        _ => return None,
    };
    Some(h * 3600. + m * 60. + s)
}

fn fmt(t: f64, vtt: bool) -> String {
    let ms = (t.max(0.) * 1000.).round() as i64;
    format!("{:02}:{:02}:{:02}{}{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, if vtt { '.' } else { ',' }, ms % 1000)
}

/// the cue times in an SRT / VTT file
fn cues(raw: &[u8]) -> Vec<(f64, f64)> {
    String::from_utf8_lossy(raw)
        .lines()
        .filter_map(|l| {
            let (a, b) = l.split_once("-->")?;
            let b = b.trim().split_whitespace().next()?;
            Some((ts(a)?, ts(b)?))
        })
        .filter(|(a, b)| b > a)
        .collect()
}

/// the same file with every timestamp moved (text untouched, byte for byte)
fn retime(raw: &[u8], ratio: f64, offset: f64) -> Vec<u8> {
    let vtt = raw.starts_with(b"WEBVTT") || raw.starts_with(b"\xEF\xBB\xBFWEBVTT");
    let mut out = Vec::with_capacity(raw.len() + 64);
    for (i, line) in raw.split(|b| *b == b'\n').enumerate() {
        if i > 0 {
            out.push(b'\n');
        }
        let s = String::from_utf8_lossy(line);
        if let Some((a, rest)) = s.split_once("-->") {
            let rest_t = rest.trim_start();
            let (b, tail) = rest_t.split_once(char::is_whitespace).map_or((rest_t.trim_end_matches('\r'), ""), |(b, t)| (b, t));
            if let (Some(x), Some(y)) = (ts(a), ts(b.trim_end_matches('\r'))) {
                let cr = if line.ends_with(b"\r") && tail.is_empty() { "\r" } else { "" };
                let tail = if tail.is_empty() { String::new() } else { format!(" {tail}") };
                out.extend_from_slice(format!("{} --> {}{}{}", fmt(x * ratio + offset, vtt), fmt(y * ratio + offset, vtt), tail, cr).as_bytes());
                continue;
            }
        }
        out.extend_from_slice(line);
    }
    out
}

/// Score every shift for one frame-rate ratio: (score, offset).
fn best_for(talk: &[f64], start: f64, cues: &[(f64, f64)], ratio: f64) -> (f64, f64, f64, f64) {
    // (subtitle times are scaled by the ratio first, so the shift left over is
    // as small as with no ratio at all)
    let pull = 0.;
    // the subtitle grid (ratio-scaled subtitle time), wide enough for every shift
    let lo = start - pull - SEARCH - 1.;
    let n_grid = ((WINDOW + 2. * SEARCH + 2.) / FRAME) as usize;
    let mut grid = vec![-1f64; n_grid];
    for (a, b) in cues {
        let (a, b) = (a * ratio, b * ratio);
        // video time of a frame = grid time + shift: lay cues out in "ratio-scaled sub time"
        let ia = ((a - lo) / FRAME).floor().max(0.) as usize;
        let ib = (((b - lo) / FRAME).ceil().max(0.) as usize).min(n_grid);
        for g in grid.iter_mut().take(ib).skip(ia.min(n_grid)) {
            *g = 1.;
        }
    }
    let steps = (2. * SEARCH / FRAME) as i64;
    let mut best = (f64::MIN, 0.);
    let mut scores = Vec::with_capacity(steps as usize + 1);
    let mut sum = 0.;
    let mut sum2 = 0.;
    for k in 0..=steps {
        let shift = pull - SEARCH + k as f64 * FRAME;
        // frame i (video time start + i·FRAME) ↔ scaled sub time start + i·FRAME − shift
        let off = ((start - shift - lo) / FRAME).round() as i64;
        let mut sc = 0.;
        for (i, t) in talk.iter().enumerate() {
            let g = off + i as i64;
            let u = if g >= 0 && (g as usize) < n_grid { grid[g as usize] } else { -1. };
            sc += t * u;
        }
        sum += sc;
        sum2 += sc * sc;
        scores.push(sc);
        if sc > best.0 {
            best = (sc, shift);
        }
    }
    // the top is often a flat stretch (the subtitle on screen a little longer
    // than the voice): take its middle, not its first step
    let bi = ((best.1 - (pull - SEARCH)) / FRAME).round() as usize;
    let tol = talk.len() as f64 * 0.004;
    let (mut a, mut b) = (bi, bi);
    while a > 0 && scores[a - 1] >= best.0 - tol {
        a -= 1;
    }
    while b + 1 < scores.len() && scores[b + 1] >= best.0 - tol {
        b += 1;
    }
    best.1 = pull - SEARCH + (a + b) as f64 / 2. * FRAME;
    let n = (steps + 1) as f64;
    let mean = sum / n;
    let sd = (sum2 / n - mean * mean).max(1e-9).sqrt();
    (best.0, best.1, (best.0 - mean) / sd, ratio)
}

pub fn run(src: &Source, raw: &[u8], cache: &std::path::Path) -> AppResult<Found> {
    let ff = ffmpeg().ok_or_else(|| AppError::msg("auto-sync needs ffmpeg, which comes with the next couch update"))?;
    let all = cues(raw);
    if all.len() < 10 {
        return Err(AppError::msg("that subtitle file has almost no lines to match"));
    }
    let start = (src.pos - 10.).max(0.);
    let mut talk = speech(&ff, src, start)?;
    let mut st = start;
    let near = |st: f64| all.iter().filter(|(a, _)| *a > st - SEARCH && *a < st + WINDOW + SEARCH).count();
    // a quiet stretch (or one with hardly any lines): try further on
    if talk.iter().filter(|x| **x).count() < 600 || near(st) < 6 {
        st = start + WINDOW;
        talk = speech(&ff, src, st)?;
    }
    if near(st) < 6 {
        return Err(AppError::msg("hardly any subtitle lines around here to match. try again during some talking"));
    }
    let t: Vec<f64> = talk.iter().map(|x| if *x { 1. } else { -1. }).collect();
    let ratios = [1.0, 23.976 / 25., 25. / 23.976, 24. / 25., 25. / 24., 23.976 / 24., 24. / 23.976];
    let results: Vec<(f64, f64, f64, f64)> = std::thread::scope(|sc| {
        let hs: Vec<_> = ratios.iter().map(|r| {
            let (t, all) = (&t, &all);
            sc.spawn(move || best_for(t, st, all, *r))
        }).collect();
        hs.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let mut best = results.iter().cloned().fold((f64::MIN, 0., 0., 1.), |a, b| if b.0 > a.0 { b } else { a });
    // a different frame rate has to win clearly, not just by luck
    if (best.3 - 1.).abs() > 1e-6 {
        if let Some(plain) = results.iter().find(|r| (r.3 - 1.).abs() < 1e-6) {
            if best.0 < plain.0 * 1.04 {
                best = *plain;
            }
        }
    }
    let (_, offset, z, ratio) = best;
    if z < 4.5 {
        return Err(AppError::msg("couldn't find a sure match here. try again during some talking"));
    }
    let file = cache.join(format!("couch-synced-{}.{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0), if raw.starts_with(b"WEBVTT") { "vtt" } else { "srt" }));
    std::fs::write(&file, retime(raw, ratio, offset)).map_err(|e| AppError::msg(e.to_string()))?;
    Ok(Found { offset, ratio, z, file })
}

#[cfg(test)]
mod tests {
    #[test]
    fn retime_keeps_text() {
        let raw = b"1\r\n00:00:01,000 --> 00:00:02,500\r\n\xC7\xE1\r\n\r\n";
        let out = super::retime(raw, 1.0, 1.5);
        assert_eq!(out, b"1\r\n00:00:02,500 --> 00:00:04,000\r\n\xC7\xE1\r\n\r\n".to_vec());
        assert_eq!(super::cues(raw), vec![(1.0, 2.5)]);
    }
    #[test]
    fn finds_a_shift() {
        // lines of random length at random gaps (like real dialogue); the
        // subtitle file runs 2.3 s late; the talking is a little ragged
        let mut seed = 7u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as f64 / (1u64 << 31) as f64
        };
        let mut lines = vec![];
        let mut t = 0.;
        while t < 4000. {
            t += 0.5 + rnd() * 6.;
            let d = 1. + rnd() * 4.;
            lines.push((t, t + d));
            t += d;
        }
        let start = 1800.;
        let n = (super::WINDOW / super::FRAME) as usize;
        let talk: Vec<f64> = (0..n)
            .map(|i| {
                let x = start + i as f64 * 0.01;
                let on = lines.iter().any(|(a, b)| x >= a + 0.15 && x < b - 0.2);
                if on ^ (rnd() < 0.08) { 1. } else { -1. }
            })
            .collect();
        let subs: Vec<(f64, f64)> = lines.iter().map(|(a, b)| (a + 2.3, b + 2.3)).collect();
        let (_, off, z, _) = super::best_for(&talk, start, &subs, 1.0);
        assert!((off + 2.3).abs() < 0.15, "offset {off}");
        assert!(z > 4.5, "z {z}");
        // the same subtitles made for 25 fps (sped up): the ratio is found too
        let fast: Vec<(f64, f64)> = lines.iter().map(|(a, b)| (a * 23.976 / 25., b * 23.976 / 25.)).collect();
        let (_, off2, _, _) = super::best_for(&talk, start, &fast, 25. / 23.976);
        assert!(off2.abs() < 0.15, "offset {off2}");
    }
}
