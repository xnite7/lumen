//! Intros and credits by ear, for shows TheIntroDB doesn't know.
//!
//! Episodes of a show open (and close) with the same music. couch listens
//! to the first minutes of an episode as a compact fingerprint (every 0.1 s:
//! which of 16 voice-band pitch bands are rising against their neighbours),
//! and compares it with an earlier episode's: the longest stretch they share
//! is the intro. Once found, that stretch is kept as the show's signature and
//! simply looked for in every later episode. Same for the end (credits).

use super::subsync::{ffmpeg, Source};
use crate::error::{AppError, AppResult};
use realfft::RealFftPlanner;

const RATE: usize = 8000;
const HOP: usize = 800; // 0.1 s
const WIN: usize = 2048;
pub const STEP: f64 = HOP as f64 / RATE as f64;
/// two frames "sound the same" from this many of 15 bits alike
const ALIKE: u32 = 12;

/// The fingerprint of `[start, start + dur)` of the film.
pub fn listen(src: &Source, start: f64, dur: f64) -> AppResult<Vec<u16>> {
    let ff = ffmpeg().ok_or_else(|| AppError::msg("needs ffmpeg"))?;
    let mut cmd = std::process::Command::new(ff);
    cmd.args(["-nostdin", "-hide_banner", "-loglevel", "error"]);
    super::subsync::add_http(&mut cmd, src);
    cmd.args(["-ss", &format!("{start:.2}"), "-i", &src.url, "-t", &format!("{dur:.2}"), "-vn", "-sn", "-dn", "-ac", "1", "-ar", "8000", "-f", "s16le", "pipe:1"]);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| AppError::msg(e.to_string()))?;
    let pcm: Vec<f32> = out.stdout.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.).collect();
    if pcm.len() < RATE * 10 {
        return Err(AppError::msg("couldn't hear the film"));
    }
    Ok(fingerprint(&pcm))
}

pub fn fingerprint(pcm: &[f32]) -> Vec<u16> {
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(WIN);
    let hann: Vec<f32> = (0..WIN).map(|i| 0.5 - 0.5 * (2. * std::f32::consts::PI * i as f32 / WIN as f32).cos()).collect();
    // 17 edges, log-spaced over the voice / music band
    let edges: Vec<usize> = (0..17).map(|i| {
        let f = 250. * (3500f32 / 250.).powf(i as f32 / 16.);
        ((f / RATE as f32) * WIN as f32).round() as usize
    }).collect();
    let mut buf = vec![0f32; WIN];
    let mut spec = fft.make_output_vec();
    let mut prev: Option<[f32; 16]> = None;
    let mut out = vec![];
    let mut i = 0;
    while i + WIN <= pcm.len() {
        for k in 0..WIN {
            buf[k] = pcm[i + k] * hann[k];
        }
        let _ = fft.process(&mut buf, &mut spec);
        let mut e = [0f32; 16];
        for b in 0..16 {
            let s: f32 = spec[edges[b]..edges[b + 1].max(edges[b] + 1)].iter().map(|c| c.norm_sqr()).sum();
            e[b] = (s + 1e-9).ln();
        }
        let mut h = 0u16;
        if let Some(p) = prev {
            for b in 0..15 {
                let d = (e[b] - e[b + 1]) - (p[b] - p[b + 1]);
                if d > 0. {
                    h |= 1 << b;
                }
            }
        }
        prev = Some(e);
        out.push(h);
        i += HOP;
    }
    out
}

fn alike(a: u16, b: u16) -> bool {
    15 - ((a ^ b) & 0x7fff).count_ones() >= ALIKE
}

/// The longest stretch two fingerprints share: (start in a, start in b,
/// length), all in frames. Small slips inside the stretch are forgiven.
pub fn common(a: &[u16], b: &[u16], min: usize) -> Option<(usize, usize, usize)> {
    let mut best: Option<(usize, usize, usize)> = None;
    let (na, nb) = (a.len() as i64, b.len() as i64);
    for d in -(na - 1)..nb {
        // b index = a index + d
        let (i0, i1) = ((-d).max(0), na.min(nb - d));
        if i1 - i0 < min as i64 {
            continue;
        }
        let (mut run_start, mut miss, mut good) = (None::<i64>, 0, 0i64);
        let mut last_good = 0i64;
        for i in i0..i1 {
            let ok = alike(a[i as usize], b[(i + d) as usize]);
            match (ok, run_start) {
                (true, None) => {
                    run_start = Some(i);
                    miss = 0;
                    good = 1;
                    last_good = i;
                }
                (true, Some(_)) => {
                    miss = 0;
                    good += 1;
                    last_good = i;
                }
                (false, Some(s)) => {
                    miss += 1;
                    // a gap of more than a second ends it
                    if miss > 10 {
                        let len = (last_good - s + 1) as usize;
                        if good as f64 > len as f64 * 0.55 && best.map_or(true, |b| len > b.2) {
                            best = Some((s as usize, (s + d) as usize, len));
                        }
                        run_start = None;
                    }
                }
                (false, None) => {}
            }
        }
        if let Some(s) = run_start {
            let len = (last_good - s + 1) as usize;
            if good as f64 > len as f64 * 0.55 && best.map_or(true, |b| len > b.2) {
                best = Some((s as usize, (s + d) as usize, len));
            }
        }
    }
    best.filter(|b| b.2 >= min)
}

/// Where a known signature sits in `hay` (frame index), if it's clearly there.
pub fn find(hay: &[u16], sig: &[u16]) -> Option<usize> {
    if sig.len() < 50 || hay.len() < sig.len() {
        return None;
    }
    let mut best = (0usize, 0f64);
    for p in 0..=(hay.len() - sig.len()) {
        let hits = sig.iter().zip(&hay[p..]).filter(|(x, y)| alike(**x, **y)).count();
        let score = hits as f64 / sig.len() as f64;
        if score > best.1 {
            best = (p, score);
        }
    }
    (best.1 > 0.5).then_some(best.0)
}

#[cfg(test)]
mod tests {
    fn noise(seed: &mut u64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|_| {
                *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((*seed >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 0.4
            })
            .collect()
    }
    /// "music": a few drifting tones with a beat (so it has a shape to recognise)
    fn theme(n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / 8000.;
                let env = 0.6 + 0.4 * (t * 7.).sin().abs();
                env * (0.3 * (t * 2. * 3.1416 * (440. + 60. * (t * 0.5).sin())).sin() + 0.2 * (t * 2. * 3.1416 * 1320.).sin() * (t * 3.).cos() + 0.15 * (t * 2. * 3.1416 * (660. + 200. * (t * 0.9).cos())).sin())
            })
            .collect()
    }
    #[test]
    fn finds_the_intro() {
        let mut s = 9u64;
        let th = theme(8000 * 40); // a 40 s intro
        // episode A: 75 s of talk, the intro, more talk; B: 20 s of talk first
        let mut a = noise(&mut s, 8000 * 75);
        a.extend(th.iter().map(|x| x + 0.02));
        a.extend(noise(&mut s, 8000 * 60));
        let mut b = noise(&mut s, 8000 * 20);
        b.extend(th.iter());
        b.extend(noise(&mut s, 8000 * 90));
        let (fa, fb) = (super::fingerprint(&a), super::fingerprint(&b));
        let (ia, ib, len) = super::common(&fa, &fb, 150).expect("no intro");
        let (ta, tb, tl) = (ia as f64 * super::STEP, ib as f64 * super::STEP, len as f64 * super::STEP);
        assert!((ta - 75.).abs() < 1.5 && (tb - 20.).abs() < 1.5 && (tl - 40.).abs() < 3., "{ta} {tb} {tl}");
        // and a later episode, by the signature alone
        let mut c = noise(&mut s, 8000 * 130);
        c.extend(th.iter());
        c.extend(noise(&mut s, 8000 * 30));
        let fc = super::fingerprint(&c);
        let p = super::find(&fc, &fa[ia..ia + len]).expect("not found");
        assert!((p as f64 * super::STEP - 130.).abs() < 1.5, "{}", p as f64 * super::STEP);
        // nothing shared: nothing found
        let n1 = super::fingerprint(&noise(&mut s, 8000 * 120));
        let n2 = super::fingerprint(&noise(&mut s, 8000 * 120));
        assert!(super::common(&n1, &n2, 150).is_none());
    }
}
