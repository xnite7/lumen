//! Tobii eye tracker, through Tobii's Stream Engine.
//!
//! The DLL is loaded at runtime from wherever Tobii's own software put it
//! (or a path you set), so Lumen builds and runs fine on machines without a
//! tracker. Gaze, head pose and presence are turned into a few things that
//! are actually useful while watching:
//!
//! - look away -> pause, look back -> resume (with a grace period)
//! - controls appear when you look at the bottom of the screen
//! - dwell on a poster -> its preview opens
//! - the hero artwork shifts with your head like a window (head3d)
//!
//! Presence is inferred from gaze validity when the device's licence doesn't
//! expose the presence stream, so the pause feature works on consumer units.

use crate::core::CoreRef;
use crate::error::{AppError, AppResult};
use parking_lot::Mutex;
use serde_json::json;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Manager;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Version {
    major: i32,
    minor: i32,
    revision: i32,
    build: i32,
}

#[repr(C)]
struct GazePoint {
    timestamp_us: i64,
    validity: i32,
    position_xy: [f32; 2],
}

#[repr(C)]
struct HeadPose {
    timestamp_us: i64,
    position_validity: i32,
    position_xyz: [f32; 3],
    rotation_validity_xyz: [i32; 3],
    rotation_xyz: [f32; 3],
}

type Api = c_void;
type Device = c_void;
type UrlReceiver = unsafe extern "C" fn(*const c_char, *mut c_void);
type GazeCb = unsafe extern "C" fn(*const GazePoint, *mut c_void);
type HeadCb = unsafe extern "C" fn(*const HeadPose, *mut c_void);
type PresenceCb = unsafe extern "C" fn(i32, i64, *mut c_void);

enum Ev {
    Gaze(Option<(f32, f32)>),
    Head(Option<[f32; 3]>),
    Presence(bool),
}

static SINK: Mutex<Option<mpsc::Sender<Ev>>> = Mutex::new(None);

unsafe extern "C" fn on_url(url: *const c_char, user: *mut c_void) {
    let out = &mut *(user as *mut Vec<String>);
    if let Ok(s) = CStr::from_ptr(url).to_str() {
        out.push(s.to_string());
    }
}

unsafe extern "C" fn on_gaze(g: *const GazePoint, _: *mut c_void) {
    let g = &*g;
    let v = (g.validity == 1).then(|| (g.position_xy[0], g.position_xy[1]));
    if let Some(tx) = SINK.lock().as_ref() {
        let _ = tx.send(Ev::Gaze(v));
    }
}

unsafe extern "C" fn on_head(h: *const HeadPose, _: *mut c_void) {
    let h = &*h;
    let v = (h.position_validity == 1).then(|| h.position_xyz);
    if let Some(tx) = SINK.lock().as_ref() {
        let _ = tx.send(Ev::Head(v));
    }
}

unsafe extern "C" fn on_presence(status: i32, _ts: i64, _: *mut c_void) {
    // 1 = away, 2 = present (0 = unknown, ignored)
    if status == 0 {
        return;
    }
    if let Some(tx) = SINK.lock().as_ref() {
        let _ = tx.send(Ev::Presence(status == 2));
    }
}

fn candidates(custom: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if !custom.is_empty() {
        v.push(PathBuf::from(custom));
    }
    if let Ok(p) = std::env::var("LUMEN_TOBII_DLL") {
        v.push(PathBuf::from(p));
    }
    #[cfg(target_os = "windows")]
    {
        let name = "tobii_stream_engine.dll";
        if let Ok(exe) = std::env::current_exe() {
            if let Some(d) = exe.parent() {
                v.push(d.join(name));
            }
        }
        for root in [std::env::var("ProgramFiles").ok(), std::env::var("ProgramFiles(x86)").ok()].into_iter().flatten() {
            let root = PathBuf::from(root);
            for sub in ["Tobii\\Tobii EyeX", "Tobii\\Tobii Experience", "Tobii\\Tobii Game Hub", "Tobii\\Tobii Stream Engine", "Tobii\\Tobii Service"] {
                v.push(root.join(sub).join(name));
            }
            // MSIX installs (Tobii Experience from the Store).
            if let Ok(rd) = std::fs::read_dir(root.join("WindowsApps")) {
                for e in rd.flatten() {
                    let n = e.file_name().to_string_lossy().to_string();
                    if n.starts_with("TobiiAB.") {
                        v.push(e.path().join(name));
                        v.push(e.path().join("platform").join(name));
                    }
                }
            }
        }
        if let Some(home) = dirs::home_dir() {
            v.push(home.join(".tobii").join(name));
        }
    }
    #[cfg(target_os = "linux")]
    {
        v.push(PathBuf::from("/usr/lib/tobii/libtobii_stream_engine.so"));
        v.push(PathBuf::from("/usr/lib/libtobii_stream_engine.so"));
    }
    v
}

pub struct Tobii {
    core: CoreRef,
    running: Arc<AtomicBool>,
    status: Mutex<String>,
}

impl Tobii {
    pub fn new(core: CoreRef) -> Arc<Self> {
        Arc::new(Self { core, running: Arc::new(AtomicBool::new(false)), status: Mutex::new("off".into()) })
    }

    pub fn status(&self) -> String {
        self.status.lock().clone()
    }

    pub fn find_dll(&self) -> Option<PathBuf> {
        let custom = self.core.settings.read().tobii.dll_path.clone();
        candidates(&custom).into_iter().find(|p| p.exists())
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn start(self: &Arc<Self>) -> AppResult<()> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let Some(dll) = self.find_dll() else {
            self.running.store(false, Ordering::SeqCst);
            *self.status.lock() = "Stream Engine not found".into();
            return Err(AppError::msg("Couldn't find tobii_stream_engine.dll — set its path in Settings → Eye tracking"));
        };
        let (tx, rx) = mpsc::channel::<Ev>();
        *SINK.lock() = Some(tx);
        let this = self.clone();
        let running = self.running.clone();
        std::thread::Builder::new()
            .name("tobii".into())
            .spawn(move || {
                if let Err(e) = unsafe { device_loop(&dll, running.clone(), &this) } {
                    *this.status.lock() = e.clone();
                    this.core.emit("tobii://status", json!({ "status": e }));
                }
                running.store(false, Ordering::SeqCst);
                *SINK.lock() = None;
            })
            .map_err(|e| AppError::msg(e.to_string()))?;
        let this = self.clone();
        std::thread::Builder::new().name("tobii-ui".into()).spawn(move || this.dispatch(rx)).map_err(|e| AppError::msg(e.to_string()))?;
        Ok(())
    }

    /// Turns raw samples into UI events + presence pause, rate-limited so a
    /// 90 Hz tracker doesn't flood the webview.
    fn dispatch(self: Arc<Self>, rx: mpsc::Receiver<Ev>) {
        let mut last_gaze_emit = Instant::now();
        let mut last_head_emit = Instant::now();
        let mut last_valid = Instant::now();
        let mut present = true;
        let mut auto_paused = false;
        let mut smoothed: Option<(f32, f32)> = None;
        loop {
            let ev = match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(e) => Some(e),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(_) => break,
            };
            if !self.running.load(Ordering::SeqCst) {
                break;
            }
            let cfg = self.core.settings.read().tobii.clone();
            let mut presence_now: Option<bool> = None;
            match ev {
                Some(Ev::Gaze(Some((x, y)))) => {
                    last_valid = Instant::now();
                    presence_now = Some(true);
                    // Light smoothing kills saccade jitter without lag you can feel.
                    let s = smoothed.map(|(sx, sy)| (sx * 0.6 + x * 0.4, sy * 0.6 + y * 0.4)).unwrap_or((x, y));
                    smoothed = Some(s);
                    if last_gaze_emit.elapsed() >= Duration::from_millis(33) {
                        if let Some((cx, cy, inside)) = self.to_client(s.0, s.1) {
                            self.core.emit("tobii://gaze", json!({ "x": cx, "y": cy, "inside": inside }));
                        }
                        last_gaze_emit = Instant::now();
                    }
                }
                Some(Ev::Gaze(None)) => {}
                Some(Ev::Head(Some(p))) => {
                    if last_head_emit.elapsed() >= Duration::from_millis(33) {
                        self.core.emit("tobii://head", json!({ "x": p[0], "y": p[1], "z": p[2] }));
                        last_head_emit = Instant::now();
                    }
                }
                Some(Ev::Head(None)) => {}
                Some(Ev::Presence(p)) => presence_now = Some(p),
                None => {}
            }
            if presence_now.is_none() && last_valid.elapsed() > Duration::from_millis(cfg.presence_grace_ms as u64) {
                presence_now = Some(false);
            }
            if let Some(p) = presence_now {
                if p != present {
                    present = p;
                    self.core.emit("tobii://presence", json!({ "present": p }));
                    if cfg.presence_pause {
                        let player = crate::state::player(&self.core.app);
                        let st = player.state();
                        let playing = !st["paused"].as_bool().unwrap_or(true) && player.session().is_some();
                        if !p && playing {
                            let _ = player.command("pause", &json!({ "value": true }));
                            auto_paused = true;
                        } else if p && auto_paused {
                            let _ = player.command("pause", &json!({ "value": false }));
                            auto_paused = false;
                        }
                    }
                }
            }
        }
    }

    /// Normalised display coords -> CSS pixels inside the main window.
    fn to_client(&self, nx: f32, ny: f32) -> Option<(f64, f64, bool)> {
        let w = self.core.app.get_webview_window("main")?;
        let mon = w.current_monitor().ok()??;
        let scale = w.scale_factor().ok()?;
        let (mp, ms) = (mon.position(), mon.size());
        let gx = mp.x as f64 + nx as f64 * ms.width as f64;
        let gy = mp.y as f64 + ny as f64 * ms.height as f64;
        let ip = w.inner_position().ok()?;
        let is = w.inner_size().ok()?;
        let (cx, cy) = (gx - ip.x as f64, gy - ip.y as f64);
        let inside = cx >= 0.0 && cy >= 0.0 && cx <= is.width as f64 && cy <= is.height as f64;
        Some((cx / scale, cy / scale, inside))
    }
}

unsafe fn device_loop(dll: &PathBuf, running: Arc<AtomicBool>, me: &Tobii) -> Result<(), String> {
    let lib = libloading::Library::new(dll).map_err(|e| format!("Couldn't load Stream Engine: {e}"))?;
    let sym = |n: &[u8]| -> Result<*const c_void, String> {
        lib.get::<*const c_void>(n).map(|s| *s).map_err(|_| format!("Stream Engine is missing {}", String::from_utf8_lossy(&n[..n.len() - 1])))
    };
    let get_version: unsafe extern "C" fn(*mut Version) -> i32 = std::mem::transmute(sym(b"tobii_get_api_version\0")?);
    let api_create: unsafe extern "C" fn(*mut *mut Api, *const c_void, *const c_void) -> i32 = std::mem::transmute(sym(b"tobii_api_create\0")?);
    let api_destroy: unsafe extern "C" fn(*mut Api) -> i32 = std::mem::transmute(sym(b"tobii_api_destroy\0")?);
    let enumerate: unsafe extern "C" fn(*mut Api, UrlReceiver, *mut c_void) -> i32 = std::mem::transmute(sym(b"tobii_enumerate_local_device_urls\0")?);
    let device_destroy: unsafe extern "C" fn(*mut Device) -> i32 = std::mem::transmute(sym(b"tobii_device_destroy\0")?);
    let gaze_sub: unsafe extern "C" fn(*mut Device, GazeCb, *mut c_void) -> i32 = std::mem::transmute(sym(b"tobii_gaze_point_subscribe\0")?);
    let process: unsafe extern "C" fn(*mut Device) -> i32 = std::mem::transmute(sym(b"tobii_device_process_callbacks\0")?);
    let reconnect: unsafe extern "C" fn(*mut Device) -> i32 = std::mem::transmute(sym(b"tobii_device_reconnect\0")?);
    let head_sub: Option<unsafe extern "C" fn(*mut Device, HeadCb, *mut c_void) -> i32> = sym(b"tobii_head_pose_subscribe\0").ok().map(|p| std::mem::transmute(p));
    let presence_sub: Option<unsafe extern "C" fn(*mut Device, PresenceCb, *mut c_void) -> i32> = sym(b"tobii_user_presence_subscribe\0").ok().map(|p| std::mem::transmute(p));
    let wait_ptr = sym(b"tobii_wait_for_callbacks\0")?;

    let mut ver = Version::default();
    get_version(&mut ver);
    let v4 = ver.major >= 4;

    let mut api: *mut Api = std::ptr::null_mut();
    if api_create(&mut api, std::ptr::null(), std::ptr::null()) != 0 || api.is_null() {
        return Err("Stream Engine failed to start".into());
    }
    let mut urls: Vec<String> = Vec::new();
    enumerate(api, on_url, &mut urls as *mut _ as *mut c_void);
    let Some(url) = urls.first().cloned() else {
        api_destroy(api);
        return Err("No eye tracker connected".into());
    };
    let curl = CString::new(url.clone()).unwrap();
    let mut dev: *mut Device = std::ptr::null_mut();
    let rc = if v4 {
        let create: unsafe extern "C" fn(*mut Api, *const c_char, i32, *mut *mut Device) -> i32 = std::mem::transmute(sym(b"tobii_device_create\0")?);
        create(api, curl.as_ptr(), 1, &mut dev) // 1 = interactive field of use
    } else {
        let create: unsafe extern "C" fn(*mut Api, *const c_char, *mut *mut Device) -> i32 = std::mem::transmute(sym(b"tobii_device_create\0")?);
        create(api, curl.as_ptr(), &mut dev)
    };
    if rc != 0 || dev.is_null() {
        api_destroy(api);
        return Err(format!("Couldn't open the tracker (error {rc})"));
    }
    if gaze_sub(dev, on_gaze, std::ptr::null_mut()) != 0 {
        device_destroy(dev);
        api_destroy(api);
        return Err("Tracker refused the gaze stream".into());
    }
    if let Some(f) = head_sub {
        f(dev, on_head, std::ptr::null_mut());
    }
    if let Some(f) = presence_sub {
        f(dev, on_presence, std::ptr::null_mut());
    }
    *me.status.lock() = format!("Connected · Stream Engine {}.{}", ver.major, ver.minor);
    me.core.emit("tobii://status", json!({ "status": me.status(), "connected": true }));

    let devs = [dev];
    while running.load(Ordering::SeqCst) {
        let wrc = if v4 {
            let wait: unsafe extern "C" fn(i32, *const *mut Device) -> i32 = std::mem::transmute(wait_ptr);
            wait(1, devs.as_ptr())
        } else {
            let wait: unsafe extern "C" fn(*mut c_void, i32, *const *mut Device) -> i32 = std::mem::transmute(wait_ptr);
            wait(std::ptr::null_mut(), 1, devs.as_ptr())
        };
        // TOBII_ERROR_TIMED_OUT (6) just means no data this tick.
        if wrc != 0 && wrc != 6 {
            std::thread::sleep(Duration::from_millis(200));
        }
        let prc = process(dev);
        if prc == 3 {
            // CONNECTION_FAILED: tracker unplugged or service restarted.
            std::thread::sleep(Duration::from_millis(500));
            reconnect(dev);
        }
    }
    device_destroy(dev);
    api_destroy(api);
    *me.status.lock() = "off".into();
    Ok(())
}
