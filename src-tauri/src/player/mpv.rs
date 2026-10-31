//! libmpv, embedded under the webview.

use crate::core::CoreRef;
use crate::error::{AppError, AppResult};
use libmpv2::events::{Event, PropertyData};
use libmpv2::Format;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Manager;

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
    ("video-params/h", Format::Int64),
    ("video-params/gamma", Format::String),
    ("video-params/primaries", Format::String),
    ("video-codec", Format::String),
    ("hwdec-current", Format::String),
    ("estimated-vf-fps", Format::Double),
];

impl Mpv {
    pub fn new(core: CoreRef) -> Self {
        Self { core, inner: Mutex::new(None), state: Arc::new(Mutex::new(State { volume: 100.0, speed: 1.0, ..Default::default() })) }
    }

    fn window_handle(&self) -> Option<i64> {
        let w = self.core.app.get_webview_window("main")?;
        #[cfg(target_os = "windows")]
        {
            return w.hwnd().ok().map(|h| h.0 as i64);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = w;
            None
        }
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
                ("audio-client-name", "Lumen".into()),
                ("user-agent", crate::http::UA.into()),
                ("deband", if p.deband { "yes".into() } else { "no".into() }),
                ("target-colorspace-hint", if p.hdr_passthrough { "yes".into() } else { "no".into() }),
                ("video-sync", if p.interpolation { "display-resample".into() } else { "audio".into() }),
                ("interpolation", if p.interpolation { "yes".into() } else { "no".into() }),
                ("scale", "ewa_lanczossharp".into()),
                ("cscale", "ewa_lanczossharp".into()),
                ("dscale", "mitchell".into()),
                ("sub-font-size", "46".into()),
                ("sub-border-size", "2.4".into()),
                ("screenshot-format", "png".into()),
            ];
            // Set LUMEN_MPV_LOG=1 to get a full mpv log next to the cache.
            let mut opts = opts;
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
                                ("aid", PropertyData::Str(v)) => s.aid = v.to_string(),
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

    pub fn load(&self, url: &str, start: Option<f64>, headers: &[(String, String)]) -> AppResult<()> {
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
        let _ = mpv.set_property("start", start.map(|t| format!("+{t:.2}")).unwrap_or_else(|| "none".into()).as_str());
        let _ = mpv.set_property("sub-visibility", false);
        self.apply_aspect(&mpv, &s.playback.aspect);
        {
            let mut st = self.state.lock();
            st.ended = false;
            st.pos = start.unwrap_or(0.0);
            st.duration = 0.0;
            st.sub_overlay = !s.playback.native_ass;
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

    fn apply_aspect(&self, mpv: &libmpv2::Mpv, mode: &str) {
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
    }

    pub fn stop(&self) {
        if let Some(mpv) = self.inner.lock().clone() {
            let _ = mpv.command("stop", &[]);
        }
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
            "equalizer" => {
                for k in ["brightness", "contrast", "saturation", "gamma"] {
                    if let Some(v) = num(k) {
                        let _ = mpv.set_property(k, v as i64);
                    }
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
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetClassNameW, GetParent, SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    unsafe extern "system" fn visit(child: HWND, parent: LPARAM) -> BOOL {
        let mut buf = [0u16; 64];
        let n = GetClassNameW(child, &mut buf);
        let class = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        let is_direct = GetParent(child).map(|p| p.0 as isize == parent.0).unwrap_or(false);
        if is_direct && class.to_ascii_lowercase().starts_with("mpv") {
            let _ = SetWindowPos(child, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
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


/// mpv's error codes, in words a person can act on.
fn friendly(e: &libmpv2::Error) -> String {
    let raw = format!("{e:?}");
    let code: Option<i32> = raw.strip_prefix("Raw(").and_then(|r| r.trim_end_matches(')').parse().ok());
    match code {
        Some(-13) => "This stream wouldn't open. The link may have expired, or the server isn't answering. Try another stream.".into(),
        Some(-16) => "There's nothing playable in this stream. Try another one.".into(),
        Some(-17) => "This file is in a format the player doesn't recognise. Try another stream.".into(),
        Some(-14) => "No audio device is available. Check your sound output.".into(),
        Some(-15) => "The video renderer couldn't start. Turn off Settings → Playback → Video inside Lumen and try again.".into(),
        _ => format!("The player hit an error ({raw}). Try another stream."),
    }
}
