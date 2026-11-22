//! Atmos on headphones: every sound (bed speaker or moving object) is panned
//! between the three nearest measured spots of a dome around the listener
//! (VBAP), and each spot is heard through its measured room response (BRIR).
//! The convolution is uniformly partitioned in the frequency domain, all
//! spots summed before the inverse transform: two inverse FFTs per block.

use super::dome::{SPOTS, TRIS};
use realfft::num_complex::Complex32;
use realfft::{RealFftPlanner, RealToComplex, ComplexToReal};
use std::sync::Arc;

const B: usize = 512;
const N: usize = 2 * B;
const BINS: usize = B + 1;

/// a direction as a unit vector: x front, y left, z up
fn unit(az: f32, el: f32) -> [f32; 3] {
    let (a, e) = (az.to_radians(), el.to_radians());
    [e.cos() * a.cos(), e.cos() * a.sin(), e.sin()]
}

/// Dolby's room coordinates (x: −1 left … 1 right, y: −1 back … 1 front,
/// z: 0 ear level … 1 ceiling) as a direction from the listener
pub fn room_dir(x: f32, y: f32, z: f32) -> [f32; 3] {
    let v = [y, -x, z.max(0.)];
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n < 1e-4 { [1., 0., 0.] } else { [v[0] / n, v[1] / n, v[2] / n] }
}

/// The dome and its triangles, ready for panning.
pub struct Panner {
    inv: Vec<[[f32; 3]; 3]>,
}

impl Panner {
    pub fn new() -> Self {
        let v: Vec<[f32; 3]> = SPOTS.iter().map(|&(a, e)| unit(a, e)).collect();
        let inv = TRIS
            .iter()
            .map(|t| {
                let (a, b, c) = (v[t[0] as usize], v[t[1] as usize], v[t[2] as usize]);
                // rows are the three spots; we need its inverse (g = p · L⁻¹)
                let m = [a, b, c];
                let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
                let d = if det.abs() < 1e-9 { 1e-9 } else { det };
                [
                    [(m[1][1] * m[2][2] - m[1][2] * m[2][1]) / d, (m[0][2] * m[2][1] - m[0][1] * m[2][2]) / d, (m[0][1] * m[1][2] - m[0][2] * m[1][1]) / d],
                    [(m[1][2] * m[2][0] - m[1][0] * m[2][2]) / d, (m[0][0] * m[2][2] - m[0][2] * m[2][0]) / d, (m[0][2] * m[1][0] - m[0][0] * m[1][2]) / d],
                    [(m[1][0] * m[2][1] - m[1][1] * m[2][0]) / d, (m[0][1] * m[2][0] - m[0][0] * m[2][1]) / d, (m[0][0] * m[1][1] - m[0][1] * m[1][0]) / d],
                ]
            })
            .collect();
        Self { inv }
    }

    /// gains for every spot (power-normalised, at most three non-zero)
    pub fn gains(&self, p: [f32; 3]) -> [f32; SPOTS.len()] {
        let mut out = [0f32; SPOTS.len()];
        let mut best: Option<(usize, [f32; 3], f32)> = None;
        for (ti, m) in self.inv.iter().enumerate() {
            // g_j = Σ_i p_i · inv[i][j]
            let g = [
                p[0] * m[0][0] + p[1] * m[1][0] + p[2] * m[2][0],
                p[0] * m[0][1] + p[1] * m[1][1] + p[2] * m[2][1],
                p[0] * m[0][2] + p[1] * m[1][2] + p[2] * m[2][2],
            ];
            let low = g[0].min(g[1]).min(g[2]);
            if best.as_ref().map_or(true, |b| low > b.2) {
                best = Some((ti, g, low));
            }
            if low >= -1e-4 {
                break;
            }
        }
        if let Some((ti, g, _)) = best {
            let g = [g[0].max(0.), g[1].max(0.), g[2].max(0.)];
            let n = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt().max(1e-6);
            for k in 0..3 {
                out[TRIS[ti][k] as usize] += g[k] / n;
            }
        }
        out
    }
}

/// The dome's room responses, cut into partitions and transformed.
pub struct Brirs {
    /// [spot][ear][partition] → spectrum
    h: Vec<[Vec<Vec<Complex32>>; 2]>,
    parts: usize,
}

impl Brirs {
    /// `wav`: float32, 2 channels per spot (left ear, right ear), 48 kHz
    pub fn from_wav(wav: &[u8]) -> Self {
        let data = &wav[44..];
        let ch = SPOTS.len() * 2;
        let frames = data.len() / (4 * ch);
        let parts = frames.div_ceil(B);
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(N);
        let mut h = Vec::with_capacity(SPOTS.len());
        for s in 0..SPOTS.len() {
            let mut ears: [Vec<Vec<Complex32>>; 2] = [vec![], vec![]];
            for ear in 0..2 {
                for p in 0..parts {
                    let mut buf = vec![0f32; N];
                    for i in 0..B {
                        let f = p * B + i;
                        if f < frames {
                            let o = (f * ch + s * 2 + ear) * 4;
                            buf[i] = f32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
                        }
                    }
                    let mut spec = fft.make_output_vec();
                    let _ = fft.process(&mut buf, &mut spec);
                    ears[ear].push(spec);
                }
            }
            h.push(ears);
        }
        Self { h, parts }
    }
}

/// Spots in, two ears out, one block of B samples at a time.
pub struct Binaural {
    brirs: Arc<Brirs>,
    fft: Arc<dyn RealToComplex<f32>>,
    ifft: Arc<dyn ComplexToReal<f32>>,
    /// per spot: the last block (overlap-save) and its spectra, newest first
    prev: Vec<Vec<f32>>,
    fdl: Vec<Vec<Vec<Complex32>>>,
    head: usize,
    /// blocks since a spot last had sound (skip silent ones)
    quiet: Vec<usize>,
    acc: [Vec<Complex32>; 2],
    scratch: Vec<f32>,
}

impl Binaural {
    pub fn new(brirs: Arc<Brirs>) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(N);
        let ifft = planner.plan_fft_inverse(N);
        let n = SPOTS.len();
        let parts = brirs.parts;
        Self {
            brirs,
            fft,
            ifft,
            prev: vec![vec![0.; B]; n],
            fdl: vec![vec![vec![Complex32::new(0., 0.); BINS]; parts]; n],
            head: 0,
            quiet: vec![usize::MAX / 2; n],
            acc: [vec![Complex32::new(0., 0.); BINS], vec![Complex32::new(0., 0.); BINS]],
            scratch: vec![0.; N],
        }
    }

    /// `input[spot]` holds B samples each; writes B frames into `out` (left, right)
    pub fn process(&mut self, input: &[Vec<f32>], out: &mut [[f32; 2]]) {
        let parts = self.brirs.parts;
        self.head = (self.head + parts - 1) % parts;
        for a in self.acc.iter_mut() {
            a.iter_mut().for_each(|c| *c = Complex32::new(0., 0.));
        }
        for s in 0..SPOTS.len() {
            let x = &input[s];
            let loud = x.iter().any(|v| v.abs() > 1e-7);
            if loud {
                self.quiet[s] = 0;
            } else {
                self.quiet[s] = self.quiet[s].saturating_add(1);
            }
            // silent for longer than the response lasts: nothing left to hear
            if self.quiet[s] > parts + 1 {
                self.prev[s].iter_mut().for_each(|v| *v = 0.);
                continue;
            }
            self.scratch[..B].copy_from_slice(&self.prev[s]);
            self.scratch[B..].copy_from_slice(x);
            self.prev[s].copy_from_slice(x);
            let slot = &mut self.fdl[s][self.head];
            let _ = self.fft.process(&mut self.scratch, slot);
            for ear in 0..2 {
                let acc = &mut self.acc[ear];
                for p in 0..parts {
                    let xs = &self.fdl[s][(self.head + p) % parts];
                    let hs = &self.brirs.h[s][ear][p];
                    for k in 0..BINS {
                        acc[k] += xs[k] * hs[k];
                    }
                }
            }
        }
        let norm = 1. / N as f32;
        for ear in 0..2 {
            // the real FFT wants the edge bins purely real
            self.acc[ear][0].im = 0.;
            self.acc[ear][BINS - 1].im = 0.;
            let _ = self.ifft.process(&mut self.acc[ear], &mut self.scratch);
            for i in 0..B {
                out[i][ear] = self.scratch[B + i] * norm;
            }
        }
    }
}

pub const BLOCK: usize = B;
