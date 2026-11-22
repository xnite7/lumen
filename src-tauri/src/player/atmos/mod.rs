//! Atmos on headphones.
//!
//! mpv plays the picture; the sound comes from here instead, as a WAV that
//! mpv reads over a tiny local web server:
//!
//!   ffmpeg (copies the TrueHD track out of the film, from the asked moment)
//!     → truehd (decodes it: up to 16 sounds, and where each one is, moving)
//!     → panning onto a dome of 29 measured spots around and above you
//!     → each spot through its measured room response (SADIE II)
//!     → two ears, 48 kHz
//!
//! Seeking: mpv asks for the WAV from another byte; that byte is a moment in
//! the film, and the chain starts again from there. Plain TrueHD (5.1 / 7.1,
//! no Atmos) goes through the same dome, each speaker where it stands.

mod dome;
mod render;

use dome::SPOTS;
use render::{room_dir, Binaural, Brirs, Panner, BLOCK};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const RATE: u32 = 48000;
const NS: usize = SPOTS.len();
const DOME: &[u8] = include_bytes!("../assets/sadie-d1-dome.wav");

/// what to play: the film's address (and how to ask for it), and which stream
#[derive(Clone)]
pub struct Source {
    pub url: String,
    pub headers: String,
    pub ua: String,
    /// ffmpeg's stream index of the TrueHD track
    pub stream: i64,
    pub duration: f64,
}

struct State {
    src: Option<Source>,
    gen: u64,
}

pub struct Server {
    pub port: u16,
    state: Arc<Mutex<State>>,
    brirs: Arc<Brirs>,
    /// it's heard objects (real Atmos) since it started
    pub objects: Arc<AtomicBool>,
}

static SERVER: OnceLock<Option<Server>> = OnceLock::new();

pub fn server() -> Option<&'static Server> {
    SERVER
        .get_or_init(|| {
            let l = TcpListener::bind("127.0.0.1:0").ok()?;
            let port = l.local_addr().ok()?.port();
            let state = Arc::new(Mutex::new(State { src: None, gen: 0 }));
            let brirs = Arc::new(Brirs::from_wav(DOME));
            let objects = Arc::new(AtomicBool::new(false));
            let (st, br, ob) = (state.clone(), brirs.clone(), objects.clone());
            std::thread::spawn(move || {
                for c in l.incoming().flatten() {
                    let (st, br, ob) = (st.clone(), br.clone(), ob.clone());
                    std::thread::spawn(move || {
                        let _ = serve(c, &st, &br, &ob);
                    });
                }
            });
            Some(Server { port, state, brirs, objects })
        })
        .as_ref()
}

impl Server {
    /// Start serving this film's sound; returns the address mpv should open.
    pub fn start(&self, src: Source) -> String {
        let mut g = self.state.lock().unwrap();
        g.gen += 1;
        g.src = Some(src);
        self.objects.store(false, Ordering::Relaxed);
        let _ = &self.brirs;
        format!("http://127.0.0.1:{}/atmos-{}.wav", self.port, g.gen)
    }
    pub fn stop(&self) {
        let mut g = self.state.lock().unwrap();
        g.gen += 1;
        g.src = None;
    }
}

fn wav_header(frames: u64) -> [u8; 44] {
    let data = (frames * 8).min(u32::MAX as u64 - 64) as u32;
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data).to_le_bytes());
    h[8..16].copy_from_slice(b"WAVEfmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&3u16.to_le_bytes()); // float
    h[22..24].copy_from_slice(&2u16.to_le_bytes());
    h[24..28].copy_from_slice(&RATE.to_le_bytes());
    h[28..32].copy_from_slice(&(RATE * 8).to_le_bytes());
    h[32..34].copy_from_slice(&8u16.to_le_bytes());
    h[34..36].copy_from_slice(&32u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    h
}

/// One request from mpv: the WAV from some byte on.
fn serve(mut c: TcpStream, st: &Arc<Mutex<State>>, brirs: &Arc<Brirs>, objects: &Arc<AtomicBool>) -> std::io::Result<()> {
    let _ = c.set_nodelay(true);
    let mut req = Vec::new();
    let mut buf = [0u8; 2048];
    while !req.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = c.read(&mut buf)?;
        if n == 0 || req.len() > 16384 {
            return Ok(());
        }
        req.extend_from_slice(&buf[..n]);
    }
    let text = String::from_utf8_lossy(&req).to_string();
    let head = text.starts_with("HEAD ");
    let (src, gen) = {
        let g = st.lock().unwrap();
        (g.src.clone(), g.gen)
    };
    let Some(src) = src.filter(|_| text.contains(&format!("/atmos-{gen}.wav"))) else {
        c.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
        return Ok(());
    };
    let frames = (src.duration * RATE as f64).ceil() as u64;
    let total = 44 + frames * 8;
    let start: u64 = text
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("range:"))
        .and_then(|l| l.split('=').nth(1))
        .and_then(|r| r.split('-').next())
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(0);
    if start >= total {
        c.write_all(format!("HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{total}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes())?;
        return Ok(());
    }
    let len = total - start;
    let status = if start > 0 { format!("206 Partial Content\r\nContent-Range: bytes {start}-{}/{total}", total - 1) } else { "200 OK".to_string() };
    c.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: audio/wav\r\nAccept-Ranges: bytes\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n").as_bytes())?;
    if head {
        return Ok(());
    }
    let mut sent = 0u64;
    if start < 44 {
        let h = wav_header(frames);
        let part = &h[start as usize..];
        c.write_all(part)?;
        sent += part.len() as u64;
    }
    // the samples, from the frame that byte falls in
    let at = start.max(44) - 44;
    let mut frame = at / 8;
    let mut skip = (at % 8) as usize;
    let alive = || st.lock().map(|g| g.gen == gen).unwrap_or(false);
    let mut tries = 0;
    while sent < len && alive() {
        let mut w = Sink { c: &mut c, skip, left: len - sent, sent: 0, frames: 0 };
        let r = pipeline(&src, frame as f64 / RATE as f64, &mut w, brirs, objects, &alive);
        sent += w.sent;
        frame += w.frames;
        skip = 0;
        match r {
            // the client went away (a seek, or the end): done
            Err(PipeEnd::Client) => return Ok(()),
            // the source ran out before the film did (or ffmpeg dropped the
            // connection while paused): carry on from where it got to
            Err(PipeEnd::Source(e)) => {
                if w.frames == 0 {
                    tries += 1;
                    if tries > 3 {
                        eprintln!("atmos: giving up: {e}");
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(400 * tries));
                } else {
                    tries = 0;
                }
            }
            Ok(()) => {
                if w.frames == 0 {
                    break;
                }
            }
        }
    }
    // past the end of the sound (or given up): silence to the end, so mpv never stalls
    let zeros = [0u8; 8192];
    while sent < len && alive() {
        let n = (len - sent).min(zeros.len() as u64) as usize;
        c.write_all(&zeros[..n])?;
        sent += n as u64;
    }
    Ok(())
}

/// where the samples go: the socket, minus the first `skip` bytes, never past `left`
struct Sink<'a> {
    c: &'a mut TcpStream,
    skip: usize,
    left: u64,
    sent: u64,
    frames: u64,
}

impl Sink<'_> {
    fn frames(&mut self, f: &[[f32; 2]]) -> bool {
        let mut bytes = Vec::with_capacity(f.len() * 8);
        for s in f {
            // a gentle ceiling: never a hard clip
            for v in s {
                let x = v.abs();
                let y = if x < 0.9 { x } else { 0.9 + 0.1 * ((x - 0.9) / 0.1).tanh() };
                bytes.extend_from_slice(&(y.copysign(*v)).to_le_bytes());
            }
        }
        self.frames += f.len() as u64;
        let mut b = &bytes[..];
        if self.skip > 0 {
            let k = self.skip.min(b.len());
            b = &b[k..];
            self.skip -= k;
        }
        let n = (b.len() as u64).min(self.left - self.sent) as usize;
        if n == 0 {
            return self.sent < self.left;
        }
        if self.c.write_all(&b[..n]).is_err() {
            return false;
        }
        self.sent += n as u64;
        self.sent < self.left
    }
}

enum PipeEnd {
    Client,
    Source(String),
}

/// Speaker positions in Dolby's room (x right, y front, z up) by TrueHD channel name.
fn label_pos(name: &str) -> Option<[f32; 3]> {
    const W: f32 = 42. / 62.;
    Some(match name {
        "L" => [-1., 1., 0.],
        "R" => [1., 1., 0.],
        "C" => [0., 1., 0.],
        "LFE" | "LFE2" => [0., 1., -1.],
        "Ls" | "Lsd" => [-1., 0., 0.],
        "Rs" | "Rsd" => [1., 0., 0.],
        "Lb" => [-1., -1., 0.],
        "Rb" => [1., -1., 0.],
        "Cb" => [0., -1., 0.],
        "Lsc" => [-0.5, 1., 0.],
        "Rsc" => [0.5, 1., 0.],
        "Lw" => [-1., W, 0.],
        "Rw" => [1., W, 0.],
        "Tfl" => [-1., 1., 1.],
        "Tfr" => [1., 1., 1.],
        "Tfc" => [0., 1., 1.],
        "Tsl" => [-1., 0., 1.],
        "Tsr" => [1., 0., 1.],
        "Tbl" => [-1., -1., 1.],
        "Tbr" => [1., -1., 1.],
        "Tc" => [0., 0., 1.],
        _ => return None,
    })
}

/// Atmos bed speakers (Dolby's numbering), as positions.
fn bed_pos(i: usize) -> [f32; 3] {
    const W: f32 = 42. / 62.;
    const P: [[f32; 3]; 17] = [
        [-1., 1., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [0., 1., -1.],
        [-1., 0., 0.],
        [1., 0., 0.],
        [-1., -1., 0.],
        [1., -1., 0.],
        [-1., 1., 1.],
        [1., 1., 1.],
        [-1., 0., 1.],
        [1., 0., 1.],
        [-1., -1., 1.],
        [1., -1., 1.],
        [-1., W, 0.],
        [1., W, 0.],
        [0., 1., -1.],
    ];
    P.get(i).copied().unwrap_or([0., 1., 0.])
}

/// One sound (a channel of the decoded TrueHD): its spot gains, gliding.
#[derive(Clone)]
struct Voice {
    cur: [f32; NS],
    step: [f32; NS],
    left: usize,
}

impl Default for Voice {
    fn default() -> Self {
        Self { cur: [0.; NS], step: [0.; NS], left: 0 }
    }
}

struct Update {
    at: u64,
    ch: usize,
    gains: [f32; NS],
    ramp: usize,
}

/// Decoded channels in, two ears out.
struct Scene {
    panner: Panner,
    bin: Binaural,
    voices: Vec<Voice>,
    pending: Vec<Update>,
    /// samples so far (the clock updates are scheduled on)
    clock: u64,
    spots: Vec<Vec<f32>>,
    fill: usize,
    out: Vec<[f32; 2]>,
    placed_by_labels: bool,
}

impl Scene {
    fn new(brirs: Arc<Brirs>) -> Self {
        Self {
            panner: Panner::new(),
            bin: Binaural::new(brirs),
            voices: vec![Voice::default(); 16],
            pending: vec![],
            clock: 0,
            spots: vec![vec![0.; BLOCK]; NS],
            fill: 0,
            out: vec![[0.; 2]; BLOCK],
            placed_by_labels: false,
        }
    }

    fn set(&mut self, ch: usize, gains: [f32; NS], ramp: usize) {
        let v = &mut self.voices[ch];
        if ramp == 0 {
            v.cur = gains;
            v.left = 0;
        } else {
            for k in 0..NS {
                v.step[k] = (gains[k] - v.cur[k]) / ramp as f32;
            }
            v.left = ramp;
        }
    }

    fn gains_at(&self, pos: [f32; 3], gain: f32) -> [f32; NS] {
        let mut g = self.panner.gains(room_dir(pos[0], pos[1], pos[2]));
        g.iter_mut().for_each(|x| *x *= gain);
        g
    }

    /// One access unit's metadata (what moved, and when).
    fn metadata(&mut self, au: &truehd::process::decode::DecodedAccessUnit) {
        for oamd in &au.oamd {
            let Some(oe) = &oamd.object_element else { continue };
            let beds: Vec<usize> = oamd.program_assignment.bed_assignment.first().map(|b| b.to_index_vec()).unwrap_or_default();
            let pos = oamd.get_damf_pos();
            let info = oe.md_update_info.block_update_info.first();
            let ramp = info.map_or(0, |b| b.ramp_duration as usize);
            let at = self.clock + oe.md_update_info.sample_offset as u64 + info.map_or(0, |b| b.block_offset_factor_bits as u64 * 32) + oamd.evo_sample_offset;
            for i in 0..oamd.object_count.min(16) {
                let Some(od) = oe.object_data.get(i).and_then(|d| d.first()) else { continue };
                let g = if od.b_object_not_active || od.object_basic_info.object_gain == -128 { 0. } else { 10f32.powf(od.object_basic_info.object_gain as f32 / 20.) };
                let p = if od.b_object_in_bed_or_isf {
                    beds.get(i).map_or([0., 1., 0.], |b| bed_pos(*b))
                } else {
                    pos.get(i).and_then(|v| v.first()).map_or([0., 1., 0.], |p| [p[0] as f32, p[1] as f32, p[2] as f32])
                };
                let gains = self.gains_at(p, g);
                self.pending.push(Update { at, ch: i, gains, ramp });
            }
        }
    }

    /// No Atmos: each channel where its speaker stands (once).
    fn by_labels(&mut self, au: &truehd::process::decode::DecodedAccessUnit) {
        if self.placed_by_labels {
            return;
        }
        self.placed_by_labels = true;
        for (i, l) in au.channel_labels.iter().enumerate().take(16) {
            let name = format!("{l:?}");
            let p = label_pos(&name).unwrap_or([0., 1., 0.]);
            let gains = self.gains_at(p, 1.);
            self.set(i, gains, 0);
        }
    }

    /// `n` samples of `channels` (None = silence, a lost access unit)
    fn push(&mut self, au: Option<&truehd::process::decode::DecodedAccessUnit>, n: usize, sink: &mut Sink) -> bool {
        let chans = au.map_or(0, |a| a.channel_count.min(16));
        for s in 0..n {
            if !self.pending.is_empty() {
                let now = self.clock;
                let mut due = vec![];
                self.pending.retain(|u| {
                    if u.at <= now {
                        due.push((u.ch, u.gains, u.ramp));
                        false
                    } else {
                        true
                    }
                });
                for (ch, g, r) in due {
                    self.set(ch, g, r);
                }
            }
            if let Some(a) = au {
                for c in 0..chans {
                    let v = &mut self.voices[c];
                    if v.left > 0 {
                        for k in 0..NS {
                            v.cur[k] += v.step[k];
                        }
                        v.left -= 1;
                    }
                    let x = a.pcm_data[s][c] as f32 / 8_388_608.;
                    if x != 0. {
                        for k in 0..NS {
                            let g = v.cur[k];
                            if g != 0. {
                                self.spots[k][self.fill] += x * g;
                            }
                        }
                    }
                }
            }
            self.clock += 1;
            self.fill += 1;
            if self.fill == BLOCK {
                self.bin.process(&self.spots, &mut self.out);
                self.spots.iter_mut().for_each(|b| b.iter_mut().for_each(|x| *x = 0.));
                self.fill = 0;
                if !sink.frames(&self.out) {
                    return false;
                }
            }
        }
        true
    }

    /// what's left in the last, part-filled block
    fn flush(&mut self, sink: &mut Sink) {
        if self.fill > 0 {
            let n = self.fill;
            self.bin.process(&self.spots, &mut self.out);
            let _ = sink.frames(&self.out[..n]);
            self.fill = 0;
        }
    }
}

/// TrueHD access units before the first one that starts fresh (a major sync):
/// the decoder can't use them, so they become silence (keeps the timing exact)
fn lead_units(b: &[u8]) -> usize {
    let (mut o, mut n) = (0usize, 0usize);
    while o + 8 <= b.len() {
        if b[o + 4..o + 8] == [0xF8, 0x72, 0x6F, 0xBA] {
            return n;
        }
        let len = (((b[o] as usize & 0x0f) << 8) | b[o + 1] as usize) * 2;
        if len < 4 {
            return n;
        }
        o += len;
        n += 1;
    }
    n
}

fn pipeline(src: &Source, from: f64, sink: &mut Sink, brirs: &Arc<Brirs>, objects: &Arc<AtomicBool>, alive: &dyn Fn() -> bool) -> Result<(), PipeEnd> {
    use truehd::process::{decode::Decoder, extract::Extractor, parse::Parser, MAX_PRESENTATIONS};
    let ff = super::subsync::ffmpeg().ok_or_else(|| PipeEnd::Source("no ffmpeg".into()))?;
    let mut cmd = std::process::Command::new(ff);
    cmd.args(["-nostdin", "-hide_banner", "-loglevel", "error"]);
    if src.url.starts_with("http") {
        if !src.ua.is_empty() {
            cmd.args(["-user_agent", &src.ua]);
        }
        if !src.headers.is_empty() {
            let h: String = src.headers.replace("\\,", "\u{1}").split(',').map(|x| format!("{}\r\n", x.replace('\u{1}', ","))).collect();
            cmd.args(["-headers", &h]);
        }
        cmd.args(["-reconnect", "1", "-reconnect_streamed", "1", "-reconnect_delay_max", "4"]);
    }
    // a quick jump to just before, then exactly to the moment (to the access unit)
    let pre = (from - 3.).max(0.);
    cmd.args(["-ss", &format!("{pre:.3}"), "-i", &src.url, "-ss", &format!("{:.3}", from - pre), "-map", &format!("0:{}", src.stream), "-c", "copy", "-f", "truehd", "pipe:1"]);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| PipeEnd::Source(e.to_string()))?;
    let mut out = child.stdout.take().ok_or_else(|| PipeEnd::Source("no output".into()))?;
    let kill = |child: &mut std::process::Child| {
        let _ = child.kill();
        let _ = child.wait();
    };
    let mut scene = Scene::new(brirs.clone());
    let mut ex = Extractor::default();
    let mut parser = Parser::default();
    let req = [true; MAX_PRESENTATIONS];
    parser.set_required_presentations(&req);
    let mut dec = Decoder::default();
    let mut buf = vec![0u8; 64 * 1024];
    let mut first = true;
    let mut spa = 40usize; // samples per access unit at 48 kHz
    let mut pick: Option<usize> = None;
    let mut got_any = false;
    loop {
        if !alive() {
            kill(&mut child);
            return Err(PipeEnd::Client);
        }
        let n = match out.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        if first {
            first = false;
            let lead = lead_units(&buf[..n]);
            if lead > 0 && !scene.push(None, lead * spa, sink) {
                kill(&mut child);
                return Err(PipeEnd::Client);
            }
        }
        ex.push_bytes(&buf[..n]);
        while let Some(f) = ex.next() {
            let f = match f {
                Ok(f) => f,
                Err(truehd::utils::errors::ExtractError::InsufficientData) => break,
                // a damaged frame: the extractor finds its feet again by itself
                Err(_) => continue,
            };
            let decoded = match parser.parse(&f) {
                Ok(au) => match dec.decode_presentations(&au, &req) {
                    Ok(d) => Some(d),
                    Err(_) => {
                        dec.reset_for_next_major_sync();
                        None
                    }
                },
                Err(_) => {
                    parser.reset_for_next_major_sync();
                    None
                }
            };
            let ok = match decoded {
                Some(d) => {
                    // the Atmos presentation (the one carrying the moving sounds), else the widest
                    let idx = pick.unwrap_or_else(|| {
                        let i = d.iter().enumerate().filter_map(|(i, p)| p.as_ref().map(|p| (i, p))).max_by_key(|(_, p)| (!p.oamd.is_empty(), p.channel_count)).map_or(0, |x| x.0);
                        pick = Some(i);
                        i
                    });
                    match d[idx].as_ref() {
                        Some(p) => {
                            if p.sampling_frequency != RATE {
                                kill(&mut child);
                                return Err(PipeEnd::Source(format!("{} Hz isn't supported", p.sampling_frequency)));
                            }
                            got_any = true;
                            spa = p.sample_length.max(1);
                            if p.oamd.is_empty() {
                                if !objects.load(Ordering::Relaxed) {
                                    scene.by_labels(p);
                                }
                            } else {
                                objects.store(true, Ordering::Relaxed);
                                scene.metadata(p);
                            }
                            scene.push(Some(p), p.sample_length, sink)
                        }
                        None => scene.push(None, spa, sink),
                    }
                }
                None => scene.push(None, spa, sink),
            };
            if !ok {
                kill(&mut child);
                return Err(PipeEnd::Client);
            }
        }
    }
    scene.flush(sink);
    kill(&mut child);
    if got_any && sink.sent >= sink.left {
        Ok(())
    } else {
        Err(PipeEnd::Source("the sound stopped early".into()))
    }
}

/// is this ffmpeg-able TrueHD? (cheap check for the button)
pub fn available() -> bool {
    super::subsync::ffmpeg().is_some()
}

#[cfg(test)]
mod tests {
    #[test]
    fn lead() {
        // two access units of 8 bytes before a major sync
        let mut b = vec![0x10, 0x04, 0, 0, 0, 0, 0, 0, 0x10, 0x04, 0, 0, 0, 0, 0, 0];
        b.extend_from_slice(&[0x10, 0x04, 0, 0, 0xF8, 0x72, 0x6F, 0xBA]);
        assert_eq!(super::lead_units(&b), 2);
    }
}
