//! libmpv, embedded under the webview.

use crate::core::CoreRef;
use crate::error::{AppError, AppResult};
use libmpv2::events::{Event, PropertyData};
use libmpv2::Format;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::sync::Arc;

static CROP_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
struct State {
    pos: f64,
    duration: f64,
    paused: bool,
    buffering: bool,
    cache_ahead: f64,
    buffering_pct: i64,
    volume: f64,
    muted: bool,
    speed: f64,
    aid: String,
    sid: String,
    ended: bool,
    idle: bool,
    sub_overlay: bool,
    sub_delay: f64,
    width: i64,
    dw: i64,
    dh: i64,
    height: i64,
    gamma: String,
    primaries: String,
    codec: String,
    hwdec: String,
    fps: f64,
}

impl State {
    fn json(&self) -> Value {
        json!({
            "pos": self.pos,
            "duration": self.duration,
            "paused": self.paused,
            "buffering": self.buffering,
            "cacheAhead": self.cache_ahead,
            "bufferingPct": self.buffering_pct,
            "volume": self.volume,
            "muted": self.muted,
            "speed": self.speed,
            "aid": self.aid,
            "sid": self.sid,
            "ended": self.ended,
            "idle": self.idle,
            "subOverlay": self.sub_overlay,
            "subDelay": self.sub_delay,
            "video": {
                "width": self.width,
                "dw": self.dw,
                "dh": self.dh,
                "height": self.height,
                "hdr": matches!(self.gamma.as_str(), "pq" | "hlg"),
                "gamma": self.gamma,
                "primaries": self.primaries,
                "codec": self.codec,
                "hwdec": self.hwdec,
                "fps": self.fps,
            }
        })
    }
}

pub struct Mpv {
    core: CoreRef,
    inner: Mutex<Option<Arc<libmpv2::Mpv>>>,
    state: Arc<Mutex<State>>,
    /// what's loaded (url, header fields, user agent), for seek thumbnails
    last: Mutex<Option<(String, String, String)>>,
    /// a second, picture-less mpv that only makes seek-bar thumbnails
    thumbs: Mutex<Option<ThumbWorker>>,
}

/// Even volume: titles are levelled to this (EBU R128, dialogue-heavy
/// mixes sit around −24 LUFS, so most films move only a little)
const LOUD_TARGET: f64 = -23.0;
static LOUD_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// what each title measured last time (so it starts levelled)
static LOUD: std::sync::LazyLock<parking_lot::Mutex<std::collections::HashMap<String, f64>>> = std::sync::LazyLock::new(|| parking_lot::Mutex::new(std::collections::HashMap::new()));

fn loud_key(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}
fn loud_gain(lufs: f64) -> f64 {
    (LOUD_TARGET - lufs).clamp(-8.0, 10.0)
}

/// Every few seconds: how loud has this title been so far (integrated, so
/// one explosion doesn't swing it), and the gain eased toward what it needs.
fn loudness(m: std::sync::Weak<libmpv2::Mpv>, gen: u64, key: String, mut gain: f64) {
    let mut since = 0u32;
    loop {
        std::thread::sleep(Duration::from_secs(4));
        if LOUD_GEN.load(std::sync::atomic::Ordering::Relaxed) != gen {
            return;
        }
        let Some(mpv) = m.upgrade() else { return };
        if mpv.get_property::<bool>("pause").unwrap_or(true) {
            continue;
        }
        since += 4;
        let direct = mpv.get_property::<String>("af-metadata/loud/lavfi.r128.I").ok().and_then(|v| v.trim().parse::<f64>().ok());
        let meta: String = mpv.get_property("af-metadata/loud").unwrap_or_default();
        let Some(i) = direct.or_else(|| meta.split(['\n', ',', '{', '}']).find_map(|kv| kv.trim().trim_matches('"').strip_prefix("lavfi.r128.I").map(|v| v.trim_start_matches(['"', '=', ':', ' ']).trim_matches('"').to_string()).and_then(|v| v.trim().parse::<f64>().ok()))) else { continue };
        // the first half minute is too little to go on (unless we knew it already)
        if since < 30 || !i.is_finite() || i < -70.0 {
            continue;
        }
        let want = loud_gain(i);
        // a dB at most every few seconds: nobody hears it move
        let step = (want - gain).clamp(-1.0, 1.0);
        if step.abs() >= 0.2 {
            gain += step;
            let _ = mpv.command("af-command", &["gain", "volume", &format!("{gain:.2}dB")]);
        }
        LOUD.lock().insert(key.clone(), i);
        // kept for next time (a minute in is a fair measure)
        if since % 60 == 0 {
            let db = &crate::state::get().core.db;
            let mut m = db.get::<std::collections::HashMap<String, f64>>("loudness").unwrap_or_default();
            if m.len() > 400 {
                m.clear();
            }
            m.insert(key.clone(), i);
            let _ = db.set("loudness", &m);
        }
    }
}

/// the Atmos sound's name in mpv's track list, and the film's own track it replaced
const ATMOS_TITLE: &str = "Atmos · couch";
static ATMOS_FROM: parking_lot::Mutex<Option<i64>> = parking_lot::Mutex::new(None);

/// the still-maker: its own mpv, and the film it has open
struct ThumbWorker {
    mpv: Arc<libmpv2::Mpv>,
    url: String,
}

/// seek-bar stills are made per this many seconds
const THUMB_STEP: f64 = 2.0;
const THUMB_CAP: usize = 16 * 1024 * 1024;
static THUMB_REQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// stills in memory: (film, step) → JPEG, oldest first, 16 MiB at most
static THUMB_CACHE: parking_lot::Mutex<(std::collections::VecDeque<((String, u64), Vec<u8>)>, usize)> = parking_lot::Mutex::new((std::collections::VecDeque::new(), 0));

fn thumb_cache_get(key: &str, bucket: u64) -> Option<Vec<u8>> {
    let g = THUMB_CACHE.lock();
    g.0.iter().find(|(k, _)| k.0 == key && k.1 == bucket).map(|(_, b)| b.clone())
}
fn thumb_cache_put(key: &str, bucket: u64, b: Vec<u8>) {
    let mut g = THUMB_CACHE.lock();
    g.1 += b.len();
    g.0.push_back(((key.to_string(), bucket), b));
    while g.1 > THUMB_CAP {
        match g.0.pop_front() {
            Some((_, old)) => g.1 -= old.len(),
            None => break,
        }
    }
}

/// the newest JPEG the image output wrote, once it's finished writing
fn wait_frame(dir: &std::path::Path, max: Duration) -> Option<std::path::PathBuf> {
    let deadline = std::time::Instant::now() + max;
    while std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(15));
        let newest = std::fs::read_dir(dir).ok().and_then(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "jpg")).max());
        if let Some(f) = newest {
            // finished when its size stops changing
            let a = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
            std::thread::sleep(Duration::from_millis(12));
            let b = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
            if a == b && b > 0 {
                return Some(f);
            }
        }
    }
    None
}

const OBSERVED: &[(&str, Format)] = &[
    ("pause", Format::Flag),
    ("time-pos", Format::Double),
    ("duration", Format::Double),
    ("paused-for-cache", Format::Flag),
    ("cache-buffering-state", Format::Int64),
    ("demuxer-cache-time", Format::Double),
    ("volume", Format::Double),
    ("mute", Format::Flag),
    ("speed", Format::Double),
    ("aid", Format::String),
    ("sid", Format::String),
    ("sub-text", Format::String),
    ("sub-delay", Format::Double),
    ("eof-reached", Format::Flag),
    ("idle-active", Format::Flag),
    ("track-list/count", Format::Int64),
    ("video-params/w", Format::Int64),
    ("video-out-params/dw", Format::Int64),
    ("video-out-params/dh", Format::Int64),
    ("video-params/h", Format::Int64),
    ("video-params/gamma", Format::String),
    ("video-params/primaries", Format::String),
    ("video-codec", Format::String),
    ("hwdec-current", Format::String),
    ("estimated-vf-fps", Format::Double),
];

impl Mpv {
    pub fn new(core: CoreRef) -> Self {
        Self { core, inner: Mutex::new(None), state: Arc::new(Mutex::new(State { volume: 100.0, speed: 1.0, ..Default::default() })), last: Mutex::new(None), thumbs: Mutex::new(None) }
    }

    fn window_handle(&self) -> Option<i64> {
        self.core.host.main_window().map(|h| h as i64)
    }

    fn ensure(&self) -> AppResult<Arc<libmpv2::Mpv>> {
        if let Some(m) = self.inner.lock().as_ref() {
            return Ok(m.clone());
        }
        let s = self.core.settings();
        let p = s.playback.clone();
        let wid = if p.embed_video { self.window_handle() } else { None };
        let langs = s.subtitles.languages.join(",");
        let alang = p.audio_languages.join(",");
        let log_path = self.core.cache_dir.join("mpv.log").to_string_lossy().to_string();
        let mpv = libmpv2::Mpv::with_initializer(|init| {
            if let Some(wid) = wid {
                init.set_property("wid", wid)?;
            }
            let opts: Vec<(&str, String)> = vec![
                // Fall back to the older renderer, then allow a software GL
                // context, rather than failing playback outright.
                ("vo", format!("{},gpu,", p.vo)),
                ("gpu-sw", "yes".into()),
                ("hwdec", p.hwdec.clone()),
                ("keep-open", "yes".into()),
                ("idle", "yes".into()),
                ("force-window", "yes".into()),
                ("osc", "no".into()),
                ("osd-level", "0".into()),
                ("input-default-bindings", "no".into()),
                ("input-vo-keyboard", "no".into()),
                ("input-cursor", "no".into()),
                ("cursor-autohide", "no".into()),
                ("background-color", "#000000".into()),
                ("cache", "yes".into()),
                ("demuxer-max-bytes", format!("{}MiB", p.cache_mb)),
                ("demuxer-max-back-bytes", "150MiB".into()),
                ("demuxer-readahead-secs", p.readahead_secs.to_string()),
                ("network-timeout", "30".into()),
                ("hr-seek", "yes".into()),
                ("alang", alang.clone()),
                ("slang", langs.clone()),
                ("sub-auto", "fuzzy".into()),
                ("ytdl", "no".into()),
                ("volume", p.volume.to_string()),
                ("volume-max", if p.volume_boost { "200".into() } else { "100".into() }),
                ("audio-client-name", "couch".into()),
                ("user-agent", crate::http::UA.into()),
                ("deband", if p.deband { "yes".into() } else { "no".into() }),
                ("target-colorspace-hint", if p.hdr_passthrough { "yes".into() } else { "no".into() }),
                ("video-sync", if p.interpolation { "display-resample".into() } else { "audio".into() }),
                ("interpolation", if p.interpolation { "yes".into() } else { "no".into() }),
                ("scale", "ewa_lanczossharp".into()),
                ("cscale", "ewa_lanczossharp".into()),
                ("dscale", "mitchell".into()),
                ("sub-font-size", "46".into()),
                // subtitles in Figtree (the new couch ships it in fonts/ beside the .exe)
                ("sub-font", "Figtree".into()),
                ("sub-border-size", "2.4".into()),
                ("screenshot-format", "png".into()),
            ];
            // Set LUMEN_MPV_LOG=1 to get a full mpv log next to the cache.
            let mut opts = opts;
            if let Some(fd) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("fonts"))).filter(|d| d.exists()) {
                opts.push(("sub-fonts-dir", fd.to_string_lossy().to_string()));
            }
            if std::env::var("LUMEN_MPV_LOG").is_ok() {
                opts.push(("log-file", log_path.clone()));
            }
            for (k, v) in opts {
                // Unknown options (older libmpv) are skipped, not fatal.
                let _ = init.set_property(k, v.as_str());
            }
            Ok(())
        })
        .map_err(|e| AppError::msg(format!("mpv failed to start: {e}")))?;

        for (i, (name, fmt)) in OBSERVED.iter().enumerate() {
            let f = match fmt {
                Format::Flag => Format::Flag,
                Format::Double => Format::Double,
                Format::Int64 => Format::Int64,
                _ => Format::String,
            };
            let _ = mpv.observe_property(name, f, i as u64);
        }
        let mpv = Arc::new(mpv);
        *self.inner.lock() = Some(mpv.clone());
        self.spawn_events(mpv.clone());
        Ok(mpv)
    }

    fn spawn_events(&self, mpv: Arc<libmpv2::Mpv>) {
        let state = self.state.clone();
        let core = self.core.clone();
        let wid = self.window_handle();
        std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || {
                let mut last_emit = Instant::now() - Duration::from_secs(1);
                let mut dirty = false;
                loop {
                    let ev = mpv.wait_event(0.25);
                    let mut force = false;
                    match ev {
                        Some(Ok(Event::PropertyChange { name, change, .. })) => {
                            let mut s = state.lock();
                            dirty = true;
                            match (name, change) {
                                ("pause", PropertyData::Flag(b)) => {
                                    s.paused = b;
                                    force = true;
                                }
                                ("time-pos", PropertyData::Double(v)) => s.pos = v,
                                ("duration", PropertyData::Double(v)) => s.duration = v,
                                ("paused-for-cache", PropertyData::Flag(b)) => {
                                    s.buffering = b;
                                    force = true;
                                }
                                ("cache-buffering-state", PropertyData::Int64(v)) => s.buffering_pct = v,
                                ("demuxer-cache-time", PropertyData::Double(v)) => s.cache_ahead = v,
                                ("volume", PropertyData::Double(v)) => s.volume = v,
                                ("mute", PropertyData::Flag(b)) => s.muted = b,
                                ("speed", PropertyData::Double(v)) => s.speed = v,
                                ("aid", PropertyData::Str(v)) => {
                                    s.aid = v.to_string();
                                    force = true;
                                }
                                ("sid", PropertyData::Str(v)) => {
                                    s.sid = v.to_string();
                                    force = true;
                                }
                                ("sub-delay", PropertyData::Double(v)) => s.sub_delay = v,
                                ("eof-reached", PropertyData::Flag(b)) => {
                                    s.ended = b;
                                    force = true;
                                }
                                ("idle-active", PropertyData::Flag(b)) => s.idle = b,
                                ("video-params/w", PropertyData::Int64(v)) => s.width = v,
                                ("video-out-params/dw", PropertyData::Int64(v)) => s.dw = v,
                                ("video-out-params/dh", PropertyData::Int64(v)) => s.dh = v,
                                ("video-params/h", PropertyData::Int64(v)) => s.height = v,
                                ("video-params/gamma", PropertyData::Str(v)) => s.gamma = v.to_string(),
                                ("video-params/primaries", PropertyData::Str(v)) => s.primaries = v.to_string(),
                                ("video-codec", PropertyData::Str(v)) => s.codec = v.to_string(),
                                ("hwdec-current", PropertyData::Str(v)) => s.hwdec = v.to_string(),
                                ("estimated-vf-fps", PropertyData::Double(v)) => s.fps = v,
                                ("sub-text", PropertyData::Str(t)) => {
                                    let t = t.to_string();
                                    drop(s);
                                    core.emit("player://sub", json!({ "text": t }));
                                    continue;
                                }
                                ("track-list/count", _) => {
                                    drop(s);
                                    core.emit("player://tracks", read_tracks(&mpv));
                                    continue;
                                }
                                _ => {}
                            }
                        }
                        Some(Ok(Event::FileLoaded)) => {
                            state.lock().ended = false;
                            core.emit("player://loaded", json!({}));
                            core.emit("player://tracks", read_tracks(&mpv));
                            #[cfg(target_os = "windows")]
                            if let Some(w) = wid {
                                zorder::send_mpv_to_back(w);
                            }
                            force = true;
                        }
                        Some(Ok(Event::VideoReconfig)) => {
                            #[cfg(target_os = "windows")]
                            if let Some(w) = wid {
                                zorder::send_mpv_to_back(w);
                            }
                        }
                        Some(Ok(Event::EndFile(reason))) => {
                            core.emit("player://end", json!({ "reason": format!("{reason:?}") }));
                            force = true;
                        }
                        Some(Ok(Event::Shutdown)) => break,
                        Some(Err(e)) => {
                            core.emit("player://error", json!({ "message": friendly(&e) }));
                        }
                        _ => {}
                    }
                    if dirty && (force || last_emit.elapsed() >= Duration::from_millis(200)) {
                        let js = state.lock().json();
                        core.emit("player://state", js);
                        last_emit = Instant::now();
                        dirty = false;
                    }
                }
                let _ = wid;
            })
            .expect("mpv event thread");
    }

    pub fn load(&self, url: &str, start: Option<f64>, start_pct: Option<f64>, headers: &[(String, String)]) -> AppResult<()> {
        let mpv = self.ensure()?;
        let s = self.core.settings();
        let mut fields: Vec<String> = headers
            .iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("user-agent"))
            .map(|(k, v)| format!("{k}: {}", v.replace(',', "\\,")))
            .collect();
        fields.dedup();
        let _ = mpv.set_property("http-header-fields", fields.join(",").as_str());
        let ua = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("user-agent")).map(|(_, v)| v.clone()).unwrap_or_else(|| crate::http::UA.into());
        let _ = mpv.set_property("user-agent", ua.as_str());
        *self.last.lock() = Some((url.to_string(), fields.join(","), ua.clone()));
        let start_opt = match (start, start_pct) {
            (Some(t), _) => format!("+{t:.2}"),
            (None, Some(p)) => format!("{:.2}%", p * 100.0),
            _ => "none".into(),
        };
        let _ = mpv.set_property("start", start_opt.as_str());
        let _ = mpv.set_property("sub-visibility", false);
        self.apply_aspect(&mpv, &s.playback.aspect);
        {
            let mut st = self.state.lock();
            st.ended = false;
            st.pos = start.unwrap_or(0.0);
            st.duration = 0.0;
            st.sub_overlay = !s.playback.native_ass;
        }
        // even volume: measure this film as it plays, and level it
        let _ = mpv.command("af", &["remove", "@loud"]);
        let _ = mpv.command("af", &["remove", "@gain"]);
        if s.playback.even_volume {
            let key = loud_key(url);
            let known = LOUD.lock().get(&key).copied().or_else(|| self.core.db.get::<std::collections::HashMap<String, f64>>("loudness").and_then(|m| m.get(&key).copied()));
            let start = known.map_or(0.0, loud_gain);
            let _ = mpv.command("af", &["add", "@loud:lavfi=[ebur128=metadata=1:peak=none]"]);
            let _ = mpv.command("af", &["add", &format!("@gain:lavfi=[volume={start:.2}dB]")]);
            let gen = LOUD_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let m = Arc::downgrade(&mpv);
            std::thread::spawn(move || loudness(m, gen, key, start));
        }
        // a new film starts clear (the last one may have been left paused, blurred)
        let _ = mpv.command("change-list", &["glsl-shader-opts", "add", "couch_blur=0"]);
        // headphone surround is set up again for the new film's channels
        let _ = mpv.command("af", &["remove", "@hp"]);
        *ATMOS_FROM.lock() = None;
        if let Some(sv) = super::atmos::server() {
            sv.stop();
        }
        mpv.command("loadfile", &[url, "replace"]).map_err(|e| AppError::msg(format!("mpv: {e}")))?;
        let _ = mpv.set_property("pause", false);
        Ok(())
    }

    pub fn add_subtitles(&self, subs: &[Value]) {
        let Some(mpv) = self.inner.lock().clone() else { return };
        for s in subs {
            let (Some(url), lang) = (s.get("url").and_then(|u| u.as_str()), s.get("lang").and_then(|l| l.as_str()).unwrap_or("und")) else { continue };
            let title = format!("{} · {}", lang, s.get("addon").and_then(|a| a.as_str()).unwrap_or("addon"));
            let _ = mpv.command("sub-add", &[url, "auto", &title, lang]);
        }
        self.autoselect_sub(&mpv);
    }

    /// Pick a subtitle track by the language preference order, preferring
    /// embedded tracks (perfect sync) over addon ones for the same language.
    fn autoselect_sub(&self, mpv: &libmpv2::Mpv) {
        let s = self.core.settings().subtitles;
        if !s.enabled_by_default {
            return;
        }
        let tracks = read_tracks(mpv);
        let subs: Vec<&Value> = tracks.as_array().into_iter().flatten().filter(|t| t["type"] == "sub").collect();
        if subs.iter().any(|t| t["selected"] == true) {
            return;
        }
        for lang in &s.languages {
            let matches = |t: &&&Value| {
                let l = t["lang"].as_str().unwrap_or("").to_lowercase();
                l == *lang || (lang.len() == 3 && l.len() == 2 && lang.starts_with(&l)) || l.starts_with(&lang[..2.min(lang.len())])
            };
            let pick = subs.iter().filter(matches).find(|t| t["external"] == false && t["forced"] == false).or_else(|| subs.iter().find(matches));
            if let Some(t) = pick {
                if let Some(id) = t["id"].as_i64() {
                    let _ = mpv.set_property("sid", id);
                    self.update_sub_mode(mpv);
                }
                return;
            }
        }
    }

    fn update_sub_mode(&self, mpv: &libmpv2::Mpv) {
        let native_ass = self.core.settings().playback.native_ass;
        let tracks = read_tracks(mpv);
        let selected_codec = tracks
            .as_array()
            .into_iter()
            .flatten()
            .find(|t| t["type"] == "sub" && t["selected"] == true)
            .and_then(|t| t["codec"].as_str().map(|s| s.to_string()))
            .unwrap_or_default();
        // Heavy typesetting (anime) is mpv's job; everything else gets the
        // glass overlay with SDH styling and the RTL fix.
        let native = native_ass && matches!(selected_codec.as_str(), "ass" | "ssa");
        let _ = mpv.set_property("sub-visibility", native);
        self.state.lock().sub_overlay = !native;
    }

    fn apply_aspect(&self, mpv: &Arc<libmpv2::Mpv>, mode: &str) {
        // any earlier crop goes (and one still being measured stops); "crop" finds it again
        let gen = CROP_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let _ = mpv.command("vf", &["remove", "@couchcrop"]);
        let _ = mpv.command("vf", &["remove", "@couchcd"]);
        match mode {
            "fill" => {
                let _ = mpv.set_property("keepaspect", true);
                let _ = mpv.set_property("panscan", 1.0f64);
            }
            "stretch" => {
                let _ = mpv.set_property("keepaspect", false);
                let _ = mpv.set_property("panscan", 0.0f64);
            }
            _ => {
                let _ = mpv.set_property("keepaspect", true);
                let _ = mpv.set_property("panscan", 0.0f64);
            }
        }
        // ("crop", cutting black bars out of the picture itself, is gone: an old
        // saved "crop" plays as "fit")
        let _ = gen;
    }

    pub fn stop(&self) {
        if let Some(mpv) = self.inner.lock().clone() {
            let _ = mpv.command("stop", &[]);
            for key in ["left", "right", "top", "bottom"] {
                let _ = mpv.set_property(&format!("video-margin-ratio-{key}"), 0.0f64);
            }
        }
        *self.last.lock() = None;
        // the still-maker lets go of the film too (if it's busy, the next film
        // replaces it anyway)
        if let Some(mut g) = self.thumbs.try_lock() {
            if let Some(w) = g.as_mut() {
                let _ = w.mpv.command("stop", &[]);
                w.url.clear();
            }
        }
    }

    /// A small still of the film at `t` seconds, for the seek bar.
    ///
    /// A second mpv (no window, no sound) keeps the same film open the whole
    /// time, paused; each still is an exact seek there, written out as a JPEG
    /// by its image output. Stills live in memory (up to 16 MiB, oldest out
    /// first), per 2 s, so going back over the bar is instant. A newer request
    /// skips the ones still waiting: the bar never queues up behind itself.
    pub fn thumb(&self, t: f64) -> Option<Vec<u8>> {
        use sha2::{Digest, Sha256};
        let (url, fields, ua) = self.last.lock().clone()?;
        let key: String = Sha256::digest(url.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect();
        let bucket = (t.max(0.0) / THUMB_STEP).floor() as u64;
        if let Some(b) = thumb_cache_get(&key, bucket) {
            return Some(b);
        }
        let ticket = THUMB_REQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        // one at a time: holding the lock is what queues requests
        let mut slot = self.thumbs.lock();
        // someone pointed somewhere newer meanwhile: theirs first (this one may
        // well be cached by now)
        if let Some(b) = thumb_cache_get(&key, bucket) {
            return Some(b);
        }
        if THUMB_REQ.load(std::sync::atomic::Ordering::SeqCst) > ticket + 1 {
            return None;
        }
        let dir = self.core.cache_dir.join("thumbs").join("live");
        if slot.as_ref().map_or(true, |w| w.url != url) {
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).ok()?;
            let m = match slot.take() {
                Some(w) => w.mpv,
                None => {
                    let m = libmpv2::Mpv::with_initializer(|init| {
                        for (k, v) in [
                            ("vo", "image"), ("vo-image-format", "jpg"), ("vo-image-jpeg-quality", "72"),
                            ("ao", "null"), ("aid", "no"), ("sid", "no"), ("hwdec", "auto-copy-safe"), ("hr-seek", "yes"),
                            ("hr-seek-framedrop", "yes"), ("vd-lavc-skiploopfilter", "all"), ("vd-lavc-fast", "yes"),
                            ("vf", "scale=360:-2"), ("idle", "yes"), ("keep-open", "always"), ("pause", "yes"), ("ytdl", "no"), ("osc", "no"),
                            ("input-default-bindings", "no"), ("cache", "yes"), ("demuxer-max-bytes", "48MiB"), ("demuxer-readahead-secs", "1"),
                            ("network-timeout", "8"), ("sub-auto", "no"), ("audio-display", "no"),
                        ] {
                            let _ = init.set_property(k, v);
                        }
                        Ok(())
                    })
                    .ok()?;
                    Arc::new(m)
                }
            };
            let _ = m.set_property("vo-image-outdir", dir.to_string_lossy().as_ref());
            let _ = m.set_property("http-header-fields", fields.as_str());
            let _ = m.set_property("user-agent", ua.as_str());
            let _ = m.set_property("start", format!("{:.2}", bucket as f64 * THUMB_STEP + THUMB_STEP / 2.).as_str());
            m.command("loadfile", &[url.as_str(), "replace"]).ok()?;
            *slot = Some(ThumbWorker { mpv: m, url: url.clone() });
            // the first frame (opening the file) is the still we want
            let f = wait_frame(&dir, Duration::from_secs(10))?;
            let bytes = std::fs::read(&f).ok()?;
            let _ = std::fs::remove_file(&f);
            if bytes.len() > 500 {
                thumb_cache_put(&key, bucket, bytes.clone());
                return Some(bytes);
            }
            return None;
        }
        let w = slot.as_ref()?;
        // clear old frames, then an exact seek: a paused mpv draws (and writes) the new frame
        if let Ok(d) = std::fs::read_dir(&dir) {
            for e in d.flatten() {
                let _ = std::fs::remove_file(e.path());
            }
        }
        let at = format!("{:.3}", bucket as f64 * THUMB_STEP + THUMB_STEP / 2.);
        w.mpv.command("seek", &[at.as_str(), "absolute+exact"]).ok()?;
        let f = wait_frame(&dir, Duration::from_secs(6))?;
        let bytes = std::fs::read(&f).ok()?;
        let _ = std::fs::remove_file(&f);
        (bytes.len() > 500).then(|| {
            thumb_cache_put(&key, bucket, bytes.clone());
            bytes
        })
    }

    /// Open the film in the still-maker as soon as it plays, so the first
    /// point at the bar is already quick.
    pub fn thumbs_warm(&self, pos: f64) {
        let _ = self.thumb(pos);
    }

    pub fn state(&self) -> Value {
        self.state.lock().json()
    }

    pub fn tracks(&self) -> Value {
        match self.inner.lock().clone() {
            Some(m) => read_tracks(&m),
            None => json!([]),
        }
    }

    /// Atmos off: back to the film's own track, the Atmos one gone.
    fn atmos_off(&self, mpv: &libmpv2::Mpv) {
        let from = ATMOS_FROM.lock().take();
        if let Some(s) = super::atmos::server() {
            s.stop();
        }
        let n: i64 = mpv.get_property("track-list/count").unwrap_or(0);
        let ours: Vec<i64> = (0..n)
            .filter(|i| mpv.get_property::<String>(&format!("track-list/{i}/title")).ok().as_deref() == Some(ATMOS_TITLE))
            .filter_map(|i| mpv.get_property::<i64>(&format!("track-list/{i}/id")).ok())
            .collect();
        if from.is_none() && ours.is_empty() {
            return;
        }
        let on_ours = ours.iter().any(|id| mpv.get_property::<i64>("aid").ok() == Some(*id));
        if let (Some(f), true) = (from, on_ours) {
            let _ = mpv.set_property("aid", f);
        }
        for id in ours {
            let _ = mpv.command("audio-remove", &[&id.to_string()]);
        }
    }

    /// What subtitle auto-sync needs: the file playing (and how to fetch it),
    /// where we are, and the chosen subtitle file.
    pub fn sync_source(&self) -> AppResult<super::subsync::Source> {
        let mpv = self.ensure()?;
        let (url, headers, ua) = self.last.lock().clone().ok_or_else(|| AppError::msg("nothing's playing"))?;
        let pos: f64 = mpv.get_property("time-pos").unwrap_or(0.0);
        let n: i64 = mpv.get_property("track-list/count").unwrap_or(0);
        let mut sub = None;
        for i in 0..n {
            let g = |k: &str| mpv.get_property::<String>(&format!("track-list/{i}/{k}")).ok();
            let sel = mpv.get_property::<bool>(&format!("track-list/{i}/selected")).unwrap_or(false);
            if sel && g("type").as_deref() == Some("sub") {
                sub = Some((g("external-filename"), mpv.get_property::<i64>(&format!("track-list/{i}/ff-index")).ok()));
            }
        }
        let (sub, embedded) = match sub {
            None => return Err(AppError::msg("pick a subtitle first")),
            // built in: by its stream in the file
            Some((None, ff)) => (String::new(), ff),
            Some((Some(f), _)) => (f, None),
        };
        Ok(super::subsync::Source { url, headers, ua, pos, sub, embedded })
    }

    /// The film playing, for listening to it: (source, length, bytes per second)
    pub fn film_source(&self) -> AppResult<(super::subsync::Source, f64, f64)> {
        let mpv = self.ensure()?;
        let (url, headers, ua) = self.last.lock().clone().ok_or_else(|| AppError::msg("nothing's playing"))?;
        let dur: f64 = mpv.get_property("duration").unwrap_or(0.0);
        let size: f64 = mpv.get_property::<i64>("file-size").map(|v| v as f64).unwrap_or(0.0);
        let rate = if dur > 0.0 { size / dur } else { 0.0 };
        Ok((super::subsync::Source { url, headers, ua, pos: 0.0, sub: String::new(), embedded: None }, dur, rate))
    }

    /// Built-in subtitles: moved by a delay.
    pub fn set_sub_delay(&self, d: f64) -> AppResult<()> {
        let mpv = self.ensure()?;
        mpv.set_property("sub-delay", d).map_err(|e| AppError::msg(format!("mpv: {e}")))
    }

    /// Load the retimed copy and switch to it.
    pub fn add_synced(&self, file: &std::path::Path) -> AppResult<()> {
        let mpv = self.ensure()?;
        let lang: String = mpv.get_property("current-tracks/sub/lang").unwrap_or_default();
        let title: String = mpv.get_property("current-tracks/sub/title").unwrap_or_else(|_| "subtitles".into());
        let title = format!("{} · synced", title.trim_end_matches(" · synced"));
        let _ = mpv.set_property("sub-delay", 0.0f64);
        let p = file.to_string_lossy().to_string();
        let r = if lang.is_empty() { mpv.command("sub-add", &[&p, "select", &title]) } else { mpv.command("sub-add", &[&p, "select", &title, &lang]) };
        r.map_err(|e| AppError::msg(format!("mpv: {e}")))
    }

    pub fn command(&self, cmd: &str, a: &Value) -> AppResult<Value> {
        let mpv = self.ensure()?;
        let num = |k: &str| a.get(k).and_then(|v| v.as_f64());
        let r = match cmd {
            "pause" => match a.get("value").and_then(|v| v.as_bool()) {
                Some(b) => mpv.set_property("pause", b),
                None => mpv.command("cycle", &["pause"]),
            },
            "seek" => {
                let to = num("to").unwrap_or(0.0);
                let mode = if a.get("relative").and_then(|v| v.as_bool()).unwrap_or(false) { "relative" } else { "absolute" };
                mpv.command("seek", &[&format!("{to:.3}"), mode])
            }
            "volume" => mpv.set_property("volume", num("value").unwrap_or(100.0)),
            "mute" => match a.get("value").and_then(|v| v.as_bool()) {
                Some(b) => mpv.set_property("mute", b),
                None => mpv.command("cycle", &["mute"]),
            },
            "speed" => mpv.set_property("speed", num("value").unwrap_or(1.0)),
            "aid" => mpv.set_property("aid", a.get("value").map(|v| v.to_string().trim_matches('"').to_string()).unwrap_or("auto".into()).as_str()),
            "sid" => {
                let v = a.get("value").map(|v| v.to_string().trim_matches('"').to_string()).unwrap_or("no".into());
                let r = mpv.set_property("sid", v.as_str());
                self.update_sub_mode(&mpv);
                r
            }
            "sub-delay" => mpv.set_property("sub-delay", num("value").unwrap_or(0.0)),
            "audio-delay" => mpv.set_property("audio-delay", num("value").unwrap_or(0.0)),
            "sub-add" => {
                let url = a.get("url").and_then(|u| u.as_str()).unwrap_or("");
                mpv.command("sub-add", &[url, "select"])
            }
            "aspect" => {
                self.apply_aspect(&mpv, a.get("value").and_then(|v| v.as_str()).unwrap_or("fit"));
                Ok(())
            }
            "zoom" => mpv.set_property("video-zoom", num("value").unwrap_or(0.0)),
            "frame-step" => mpv.command("frame-step", &[]),
            "frame-back-step" => mpv.command("frame-back-step", &[]),
            "chapter" => mpv.command("add", &["chapter", &format!("{}", num("delta").unwrap_or(1.0) as i64)]),
            "screenshot" => mpv.command("screenshot", &["video"]),
            // a still of the current frame (the couch row's card art)
            // mini player: the picture only fills a corner of the window
            "margins" => {
                for (k, key) in [("l", "left"), ("r", "right"), ("t", "top"), ("b", "bottom")] {
                    let _ = mpv.set_property(&format!("video-margin-ratio-{key}"), num(k).unwrap_or(0.0).clamp(0.0, 0.95));
                }
                Ok(())
            }
            "snap" => mpv.command("screenshot-to-file", &[a.get("path").and_then(|p| p.as_str()).unwrap_or(""), "video"]),
            "equalizer" => {
                for k in ["brightness", "contrast", "saturation", "gamma"] {
                    if let Some(v) = num(k) {
                        let _ = mpv.set_property(k, v as i64);
                    }
                }
                Ok(())
            }
            // head-tracked 3D (see depth3d.rs)
            "3d-on" => {
                let layout = if a.get("layout").and_then(|v| v.as_str()) == Some("ou") { "ou" } else { "sbs" };
                let path = self.core.cache_dir.join(format!("couch-3d-{layout}.glsl"));
                std::fs::write(&path, super::depth3d::shader(layout)).map_err(|e| AppError::msg(e.to_string()))?;
                let p = path.to_string_lossy().to_string();
                let _ = mpv.command("change-list", &["glsl-shaders", "remove", &self.core.cache_dir.join("couch-3d-sbs.glsl").to_string_lossy()]);
                let _ = mpv.command("change-list", &["glsl-shaders", "remove", &self.core.cache_dir.join("couch-3d-ou.glsl").to_string_lossy()]);
                let _ = mpv.set_property("glsl-shader-opts", "head=0.0,strength=1.0");
                // full-width SBS / full-height OU: each eye is the real shape
                if let Some(r) = num("aspect").filter(|r| *r > 0.0) {
                    let _ = mpv.set_property("video-aspect-override", format!("{r:.4}").as_str());
                }
                mpv.command("change-list", &["glsl-shaders", "append", &p])
            }
            "3d-off" => {
                for l in ["sbs", "ou"] {
                    let _ = mpv.command("change-list", &["glsl-shaders", "remove", &self.core.cache_dir.join(format!("couch-3d-{l}.glsl")).to_string_lossy()]);
                }
                mpv.set_property("video-aspect-override", "-1")
            }
            "3d-view" => {
                let x = num("x").unwrap_or(0.0).clamp(-1.0, 1.0);
                let k = num("strength").unwrap_or(1.0).clamp(0.0, 2.0);
                mpv.set_property("glsl-shader-opts", format!("head={x:.3},strength={k:.2}").as_str())
            }
            // headphone surround: every speaker of a 5.1 / 7.1 mix placed around
            // your head (measured room responses, SADIE II), mixed down for two ears
            "headphones" => {
                let on = a.get("value").and_then(|v| v.as_bool()).unwrap_or(false);
                let _ = mpv.command("af", &["remove", "@hp"]);
                if !on {
                    self.atmos_off(&mpv);
                    return Ok(json!({ "on": false }));
                }
                // the sound playing now
                let n: i64 = mpv.get_property("track-list/count").unwrap_or(0);
                let cur = (0..n).find(|i| {
                    mpv.get_property::<String>(&format!("track-list/{i}/type")).ok().as_deref() == Some("audio") && mpv.get_property::<bool>(&format!("track-list/{i}/selected")).unwrap_or(false)
                });
                let g = |i: i64, k: &str| mpv.get_property::<String>(&format!("track-list/{i}/{k}")).ok();
                let title = cur.and_then(|i| g(i, "title")).unwrap_or_default();
                let codec = cur.and_then(|i| g(i, "codec")).unwrap_or_default();
                let profile = cur.and_then(|i| g(i, "codec-profile")).unwrap_or_default();
                // already on the Atmos sound
                if title == ATMOS_TITLE {
                    let objects = super::atmos::server().map_or(false, |s| s.objects.load(std::sync::atomic::Ordering::Relaxed));
                    return Ok(json!({ "on": true, "atmos": true, "objects": objects }));
                }
                // TrueHD (Atmos or not): decoded here, every sound placed around and above you
                if codec == "truehd" && super::atmos::available() {
                    if let (Some(i), Some((url, headers, ua)), Some(server)) = (cur, self.last.lock().clone(), super::atmos::server()) {
                        let stream: i64 = mpv.get_property(&format!("track-list/{i}/ff-index")).unwrap_or(-1);
                        let duration: f64 = mpv.get_property("duration").unwrap_or(0.0);
                        let id: i64 = mpv.get_property(&format!("track-list/{i}/id")).unwrap_or(0);
                        if stream >= 0 && duration > 0.0 {
                            self.atmos_off(&mpv);
                            let addr = server.start(super::atmos::Source { url, headers, ua, stream, duration });
                            *ATMOS_FROM.lock() = Some(id);
                            mpv.command("audio-add", &[&addr, "select", ATMOS_TITLE]).map_err(|e| AppError::msg(format!("mpv: {e}")))?;
                            return Ok(json!({ "on": true, "atmos": true, "atmosTrack": profile.to_lowercase().contains("atmos") }));
                        }
                    }
                }
                // another track picked while the Atmos sound was on: that one, the usual way
                self.atmos_off(&mpv);
                let layout: String = mpv.get_property("audio-params/channels").unwrap_or_default();
                let Some(map) = super::headphones::speakers(&layout) else {
                    return Ok(json!({ "on": false, "why": "stereo" }));
                };
                let path = super::headphones::ir_file(&self.core.cache_dir, &map)?;
                let p = path.to_string_lossy().replace('\\', "/").replace(':', "\\:");
                // (the headphone filter turns everything down 3 dB per speaker; gain puts it back.
                // the graph has its own [labels], so it goes in mpv's %length% quoting)
                let g = format!("aresample=48000[main];amovie='{p}'[ir];[main][ir]headphone=map={}:hrir=multich:type=freq:gain={},alimiter=limit=0.97:level=false", map.join("|"), 3 * map.len());
                let graph = format!("@hp:lavfi=graph=%{}%{g}", g.len());
                mpv.command("af", &["add", &graph]).map_err(|e| AppError::msg(format!("mpv: {e}")))?;
                return Ok(json!({ "on": true, "speakers": map.len() }));
            }
            // night mode: loud bits quieter, quiet bits (dialogue) louder, so
            // nobody's woken by an explosion. A compressor, then gentle levelling
            "night" => {
                let on = a.get("value").and_then(|v| v.as_bool()).unwrap_or(false);
                let _ = mpv.command("af", &["remove", "@night"]);
                if on {
                    mpv.command("af", &["add", "@night:lavfi=[acompressor=threshold=-24dB:ratio=6:attack=5:release=200:makeup=6dB,dynaudnorm=f=250:g=15:m=8]"])
                } else {
                    Ok(())
                }
            }
            // paused: the picture softly blurs, most at the bottom left (where the
            // title sits), fading out toward the top right. 0 = off
            "pause-blur" => {
                let v = num("value").unwrap_or(0.0).clamp(0.0, 1.0);
                let path = self.core.cache_dir.join("couch-pause-blur.glsl");
                let p = path.to_string_lossy().to_string();
                let on: Vec<String> = mpv.get_property::<String>("glsl-shaders").map(|l| l.split([';', ',']).map(String::from).collect()).unwrap_or_default();
                if !on.iter().any(|x| x == &p) {
                    if v <= 0.0 {
                        return Ok(Value::Null);
                    }
                    std::fs::write(&path, PAUSE_BLUR).map_err(|e| AppError::msg(e.to_string()))?;
                    let _ = mpv.command("change-list", &["glsl-shaders", "append", &p]);
                }
                mpv.command("change-list", &["glsl-shader-opts", "add", &format!("couch_blur={v:.3}")])
            }
            // ambient mode: the film's light spills into the black bars around
            // it, every frame (x, y, w, h: where the picture is, fractions of
            // the window; s: strength 0..1, 0 = off)
            // ambient mode: mpv fills the black bars with a big, soft blur of
            // the picture itself, every frame (s: 0 = off, else on)
            "ambient" => {
                let on = num("s").unwrap_or(0.0) > 0.0;
                if on {
                    let _ = mpv.set_property("background-blur-radius", 0.25f64);
                }
                mpv.set_property("border-background", if on { "blur" } else { "color" })
            }
            // the control bar's glass: the film behind it blurred, in its
            // rounded shape (x, y, w, h, r: fractions of the window; a: 0..1)
            "bar-blur" => {
                let f = |k: &str| num(k).unwrap_or(0.0);
                let path = self.core.cache_dir.join("couch-bar-blur.glsl");
                let p = path.to_string_lossy().to_string();
                let on: Vec<String> = mpv.get_property::<String>("glsl-shaders").map(|l| l.split([';', ',']).map(String::from).collect()).unwrap_or_default();
                if !on.iter().any(|x| x == &p) {
                    if f("a") <= 0.0 {
                        return Ok(Value::Null);
                    }
                    std::fs::write(&path, BAR_BLUR).map_err(|e| AppError::msg(e.to_string()))?;
                    let _ = mpv.command("change-list", &["glsl-shaders", "append", &p]);
                }
                let opts = format!(
                    "couch_bx={:.5},couch_by={:.5},couch_bw={:.5},couch_bh={:.5},couch_br={:.5},couch_ba={:.3}",
                    f("x"),
                    f("y"),
                    f("w"),
                    f("h"),
                    f("r"),
                    f("a").clamp(0.0, 1.0)
                );
                for kv in opts.split(',') {
                    let _ = mpv.command("change-list", &["glsl-shader-opts", "add", kv]);
                }
                Ok(())
            }
            "deband" => mpv.set_property("deband", a.get("value").and_then(|v| v.as_bool()).unwrap_or(true)),
            "chapters" => {
                let n: i64 = mpv.get_property("chapter-list/count").unwrap_or(0);
                let list: Vec<Value> = (0..n)
                    .map(|i| {
                        json!({
                            "title": mpv.get_property::<String>(&format!("chapter-list/{i}/title")).unwrap_or_default(),
                            "time": mpv.get_property::<f64>(&format!("chapter-list/{i}/time")).unwrap_or(0.0),
                        })
                    })
                    .collect();
                return Ok(json!(list));
            }
            _ => return Err(AppError::msg(format!("unknown player command {cmd}"))),
        };
        r.map_err(|e| AppError::msg(format!("mpv {cmd}: {e}")))?;
        Ok(Value::Null)
    }
}

fn read_tracks(mpv: &libmpv2::Mpv) -> Value {
    let n: i64 = mpv.get_property("track-list/count").unwrap_or(0);
    let s = |i: i64, k: &str| mpv.get_property::<String>(&format!("track-list/{i}/{k}")).ok();
    let b = |i: i64, k: &str| mpv.get_property::<bool>(&format!("track-list/{i}/{k}")).unwrap_or(false);
    let list: Vec<Value> = (0..n)
        .map(|i| {
            json!({
                "id": mpv.get_property::<i64>(&format!("track-list/{i}/id")).unwrap_or(0),
                "type": s(i, "type"),
                "lang": s(i, "lang"),
                "title": s(i, "title"),
                "codec": s(i, "codec"),
                "selected": b(i, "selected"),
                "external": b(i, "external"),
                "default": b(i, "default"),
                "forced": b(i, "forced"),
                "channels": mpv.get_property::<i64>(&format!("track-list/{i}/demux-channel-count")).ok(),
                "width": mpv.get_property::<i64>(&format!("track-list/{i}/demux-w")).ok(),
                "height": mpv.get_property::<i64>(&format!("track-list/{i}/demux-h")).ok(),
            })
        })
        .collect();
    json!(list)
}

#[cfg(target_os = "windows")]
mod zorder {
    //! mpv creates its own child window inside ours. Newer child windows go
    //! on top, which would put the video OVER the webview; push it to the
    //! bottom so the UI floats above it.
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetClassNameW, GetClientRect, GetParent, SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    // Also stretches mpv's window to exactly fill ours. Without this it can
    // keep a stale size after fullscreen or a DPI change: right width, wrong
    // height, and "fill" crops the bottom off.
    unsafe extern "system" fn visit(child: HWND, parent: LPARAM) -> BOOL {
        let mut buf = [0u16; 64];
        let n = GetClassNameW(child, &mut buf);
        let class = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        let is_direct = GetParent(child).map(|p| p.0 as isize == parent.0).unwrap_or(false);
        if is_direct && class.to_ascii_lowercase().starts_with("mpv") {
            // the picture never takes the mouse: clicks and moves go to the
            // UI drawn over it (the new app has no webview above mpv)
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(child, false);
            let mut r = RECT::default();
            let ph = HWND(parent.0 as *mut _);
            if GetClientRect(ph, &mut r).is_ok() && r.right > 0 && r.bottom > 0 {
                let _ = SetWindowPos(child, Some(HWND_BOTTOM), 0, 0, r.right - r.left, r.bottom - r.top, SWP_NOACTIVATE);
            } else {
                let _ = SetWindowPos(child, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        }
        BOOL(1)
    }

    pub fn send_mpv_to_back(main: i64) {
        unsafe {
            let h = HWND(main as isize as *mut _);
            let _ = EnumChildWindows(Some(h), Some(visit), LPARAM(main as isize));
        }
    }
}


/// The window changed size (fullscreen, maximise, DPI): make mpv's child
/// window match it again. Done twice, because Windows settles the size a
/// moment after the event.
pub fn refit(main: Option<isize>) {
    #[cfg(target_os = "windows")]
    if let Some(h) = main {
        let main = h as i64;
        zorder::send_mpv_to_back(main);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(180));
            zorder::send_mpv_to_back(main);
        });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = main;
}

/// mpv's error codes, in words a person can act on.
fn friendly(e: &libmpv2::Error) -> String {
    let raw = format!("{e:?}");
    let code: Option<i32> = raw.strip_prefix("Raw(").and_then(|r| r.trim_end_matches(')').parse().ok());
    match code {
        Some(-13) => "This stream wouldn't open. The link may have expired, or the server isn't answering. Try another stream.".into(),
        Some(-16) => "There's nothing playable in this stream. Try another one.".into(),
        Some(-17) => "This file is in a format the player doesn't recognise. Try another stream.".into(),
        Some(-14) => "No audio device is available. Check your sound output.".into(),
        Some(-15) => "The video renderer couldn't start. Turn off Settings → Playback → Video inside couch and try again.".into(),
        _ => format!("The player hit an error ({raw}). Try another stream."),
    }
}


/// "crop": cut away black bars that are part of the picture itself (a 2.39:1
/// film inside a 16:9 file), measured from the frames, never more. The picture
/// is then fitted as usual, so nothing of the film is lost.
#[allow(dead_code)]
fn crop_bars(mpv: &libmpv2::Mpv, gen: u64) {
    let current = || CROP_GEN.load(std::sync::atomic::Ordering::Relaxed) == gen;
    // wait until it's really playing (we need its length)
    let mut dur = 0.0f64;
    for _ in 0..80 {
        if !current() {
            return;
        }
        dur = mpv.get_property("duration").unwrap_or(0.0);
        if dur > 0.0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    if dur <= 0.0 {
        return;
    }
    // bars baked into the picture (a 4:3 show inside a 16:9 file, a scope film
    // with black top and bottom): measured on stills from a few places in it.
    // A bar has to be black in every one, so a dark scene doesn't count.
    let mut cut: Option<[f64; 4]> = None; // left, right, top, bottom (fractions)
    for f in [0.18, 0.37, 0.55, 0.73] {
        if !current() {
            return;
        }
        let Some(jpg) = crate::state::player().thumb(dur * f) else { continue };
        let Ok(im) = image::load_from_memory(&jpg) else { continue };
        let im = im.to_luma8();
        let (w, h) = (im.width() as usize, im.height() as usize);
        if w < 32 || h < 32 {
            continue;
        }
        let px = |x: usize, y: usize| im.get_pixel(x as u32, y as u32)[0];
        // a line is bar when (nearly) all of it is dark: allow a few bright specks (a channel logo)
        let dark_col = |x: usize| (0..h).filter(|&y| px(x, y) > 30).count() * 50 < h;
        let dark_row = |y: usize| (0..w).filter(|&x| px(x, y) > 30).count() * 50 < w;
        let left = (0..w / 2).take_while(|&x| dark_col(x)).count();
        let right = (0..w / 2).take_while(|&x| dark_col(w - 1 - x)).count();
        let top = (0..h / 2).take_while(|&y| dark_row(y)).count();
        let bottom = (0..h / 2).take_while(|&y| dark_row(h - 1 - y)).count();
        let here = [left as f64 / w as f64, right as f64 / w as f64, top as f64 / h as f64, bottom as f64 / h as f64];
        cut = Some(match cut {
            None => here,
            Some(c) => [c[0].min(here[0]), c[1].min(here[1]), c[2].min(here[2]), c[3].min(here[3])],
        });
    }
    let Some([l, r, t, b]) = cut else { return };
    if !current() {
        return;
    }
    // one pixel in from each edge (jpeg fuzz), slivers ignored, never more than 40%
    let clean = |v: f64| if v < 0.015 || v > 0.4 { 0.0 } else { (v + 0.004).min(0.4) };
    let (l, r, t, b) = (clean(l), clean(r), clean(t), clean(b));
    if l + r + t + b <= 0.0 {
        return;
    }
    let (fw, fh) = (1.0 - l - r, 1.0 - t - b);
    let _ = mpv.command("vf", &["add", &format!("@couchcrop:lavfi=[crop=w=trunc(iw*{fw:.4}/2)*2:h=trunc(ih*{fh:.4}/2)*2:x=trunc(iw*{l:.4}):y=trunc(ih*{t:.4})]")]);
}


/// The control bar's glass: inside a rounded rectangle (given as fractions of
/// the window, live), the picture is blurred and lifted a little, like frosted
/// glass; outside it nothing changes. On the finished picture (OUTPUT), so it
/// lines up with couch's bar exactly, black bars and all.
const BAR_BLUR: &str = r#"//!PARAM couch_bx
//!TYPE DYNAMIC float
0.0
//!PARAM couch_by
//!TYPE DYNAMIC float
0.0
//!PARAM couch_bw
//!TYPE DYNAMIC float
0.0
//!PARAM couch_bh
//!TYPE DYNAMIC float
0.0
//!PARAM couch_br
//!TYPE DYNAMIC float
0.0
//!PARAM couch_ba
//!TYPE DYNAMIC float
0.0

//!HOOK OUTPUT
//!BIND HOOKED
//!DESC couch bar glass
vec4 hook() {
    vec4 c = HOOKED_tex(HOOKED_pos);
    if (couch_ba <= 0.001 || couch_bw <= 0.0)
        return c;
    vec2 px = HOOKED_pos * HOOKED_size;
    vec2 lo = vec2(couch_bx, couch_by) * HOOKED_size;
    vec2 sz = vec2(couch_bw, couch_bh) * HOOKED_size;
    float rad = couch_br * HOOKED_size.y;
    // rounded rectangle: <0 inside
    vec2 q = abs(px - (lo + sz * 0.5)) - (sz * 0.5 - vec2(rad));
    float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - rad;
    if (d > 1.5)
        return c;
    float inside = clamp(0.5 - d / 1.5, 0.0, 1.0) * couch_ba;
    float r = 22.0 * HOOKED_size.y / 1080.0;
    vec4 acc = c;
    float n = 1.0;
    for (int i = 1; i < 48; i++) {
        float fi = float(i);
        float rr = sqrt(fi / 48.0) * r;
        float a = fi * 2.39996323;
        acc += HOOKED_tex(HOOKED_pos + vec2(cos(a), sin(a)) * rr * HOOKED_pt);
        n += 1.0;
    }
    vec4 b = acc / n;
    // frosted: a touch brighter and less saturated
    float l = dot(b.rgb, vec3(0.299, 0.587, 0.114));
    b.rgb = mix(b.rgb, vec3(l), 0.18) * 1.06 + 0.015;
    return mix(c, b, inside);
}
"#;

/// The paused picture's blur: a golden-angle disc blur on the film's picture,
/// strength (`couch_blur`, 0..1) changed live without recompiling.
const PAUSE_BLUR: &str = r#"//!PARAM couch_blur
//!TYPE DYNAMIC float
//!MINIMUM 0.0
//!MAXIMUM 1.0
0.0

//!HOOK MAIN
//!BIND HOOKED
//!DESC couch pause blur
vec4 hook() {
    vec4 c = HOOKED_tex(HOOKED_pos);
    if (couch_blur <= 0.001)
        return c;
    // 0 at the bottom left corner, 1 at the top right
    float d = (HOOKED_pos.x + (1.0 - HOOKED_pos.y)) * 0.5;
    float w = 0.3 + 0.7 * (1.0 - smoothstep(0.1, 0.85, d));
    // (on the film itself, before it's scaled and before any black bars: the
    // edges blur into themselves, never into black, so no dark rim)
    float r = couch_blur * w * 26.0 * HOOKED_size.x / 1920.0;
    vec4 acc = c;
    float n = 1.0;
    for (int i = 1; i < 56; i++) {
        float fi = float(i);
        float rr = sqrt(fi / 56.0) * r;
        float a = fi * 2.39996323;
        acc += HOOKED_tex(HOOKED_pos + vec2(cos(a), sin(a)) * rr * HOOKED_pt);
        n += 1.0;
    }
    return acc / n;
}
"#;
