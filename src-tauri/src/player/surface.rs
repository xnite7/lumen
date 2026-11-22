//! A small video for the UI itself (billboard clips, hover previews): its
//! own muted mpv, rendered in software into a frame the UI draws like an
//! image. Separate from the main player, which draws straight to the window.
use libmpv2::Mpv;
use libmpv2_sys as sys;
use parking_lot::{Condvar, Mutex};
use std::ffi::{c_void, CString};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// BGRA, opaque
    pub bgra: Vec<u8>,
}

struct Shared {
    dirty: Mutex<bool>,
    cv: Condvar,
}

pub struct Surface {
    mpv: Mpv,
    ctx: *mut sys::mpv_render_context,
    shared: Arc<Shared>,
    size: Mutex<(u32, u32)>,
    frame: Mutex<Option<Arc<Frame>>>,
    /// bumps with every new frame
    pub serial: AtomicU64,
    stop: AtomicBool,
    on_frame: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

unsafe impl Send for Surface {}
unsafe impl Sync for Surface {}

extern "C" fn update_cb(data: *mut c_void) {
    let s = unsafe { &*(data as *const Shared) };
    *s.dirty.lock() = true;
    s.cv.notify_one();
}

impl Surface {
    pub fn new(width: u32, height: u32) -> Result<Arc<Self>, String> {
        let mpv = Mpv::with_initializer(|i| {
            i.set_property("vo", "libmpv")?;
            i.set_property("hwdec", "auto-copy")?;
            i.set_property("mute", "yes")?;
            i.set_property("loop-file", "inf")?;
            i.set_property("keep-open", "yes")?;
            i.set_property("osc", "no")?;
            i.set_property("ytdl", "yes")?;
            i.set_property("ytdl-format", "bestvideo[height<=720]+bestaudio/best[height<=720]/best")?;
            // trailers come from YouTube: yt-dlp ships next to couch
            if let Some(y) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("yt-dlp.exe"))).filter(|p| p.exists()) {
                i.set_property("script-opts", format!("ytdl_hook-ytdl_path={}", y.to_string_lossy()))?;
            }
            i.set_property("audio-display", "no")?;
            i.set_property("terminal", "no")?;
            Ok(())
        })
        .map_err(|e| format!("{e:?}"))?;
        let shared = Arc::new(Shared { dirty: Mutex::new(false), cv: Condvar::new() });
        let api = CString::new("sw").unwrap();
        let mut params = [
            sys::mpv_render_param { type_: sys::mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE, data: api.as_ptr() as *mut c_void },
            sys::mpv_render_param { type_: 0, data: std::ptr::null_mut() },
        ];
        let mut ctx: *mut sys::mpv_render_context = std::ptr::null_mut();
        let rc = unsafe { sys::mpv_render_context_create(&mut ctx, mpv.ctx.as_ptr(), params.as_mut_ptr()) };
        if rc < 0 || ctx.is_null() {
            return Err(format!("mpv sw render: {rc}"));
        }
        unsafe { sys::mpv_render_context_set_update_callback(ctx, Some(update_cb), Arc::as_ptr(&shared) as *mut c_void) };
        let me = Arc::new(Self {
            mpv,
            ctx,
            shared,
            size: Mutex::new((width.max(16), height.max(16))),
            frame: Mutex::new(None),
            serial: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            on_frame: Mutex::new(None),
        });
        let w = Arc::downgrade(&me);
        std::thread::Builder::new()
            .name("couch-surface".into())
            .spawn(move || loop {
                let Some(s) = w.upgrade() else { break };
                if s.stop.load(Ordering::Relaxed) {
                    break;
                }
                {
                    let mut d = s.shared.dirty.lock();
                    if !*d {
                        s.shared.cv.wait_for(&mut d, std::time::Duration::from_millis(250));
                    }
                    if !*d {
                        continue;
                    }
                    *d = false;
                }
                let flags = unsafe { sys::mpv_render_context_update(s.ctx) };
                if flags & (sys::mpv_render_update_flag_MPV_RENDER_UPDATE_FRAME as u64) == 0 {
                    continue;
                }
                s.render();
            })
            .map_err(|e| e.to_string())?;
        Ok(me)
    }

    fn render(&self) {
        let (w, h) = *self.size.lock();
        let stride = (w as usize) * 4;
        let mut buf = vec![0u8; stride * h as usize];
        let mut size = [w as i32, h as i32];
        let fmt = CString::new("bgr0").unwrap();
        let mut stride_v: usize = stride;
        let mut params = [
            sys::mpv_render_param { type_: sys::mpv_render_param_type_MPV_RENDER_PARAM_SW_SIZE, data: size.as_mut_ptr() as *mut c_void },
            sys::mpv_render_param { type_: sys::mpv_render_param_type_MPV_RENDER_PARAM_SW_FORMAT, data: fmt.as_ptr() as *mut c_void },
            sys::mpv_render_param { type_: sys::mpv_render_param_type_MPV_RENDER_PARAM_SW_STRIDE, data: &mut stride_v as *mut usize as *mut c_void },
            sys::mpv_render_param { type_: sys::mpv_render_param_type_MPV_RENDER_PARAM_SW_POINTER, data: buf.as_mut_ptr() as *mut c_void },
            sys::mpv_render_param { type_: 0, data: std::ptr::null_mut() },
        ];
        let rc = unsafe { sys::mpv_render_context_render(self.ctx, params.as_mut_ptr()) };
        if rc < 0 {
            return;
        }
        for px in buf.chunks_exact_mut(4) {
            px[3] = 255;
        }
        *self.frame.lock() = Some(Arc::new(Frame { width: w, height: h, bgra: buf }));
        self.serial.fetch_add(1, Ordering::Relaxed);
        if let Some(f) = self.on_frame.lock().as_ref() {
            f();
        }
    }

    /// Called (on the render thread) whenever a new frame is ready.
    pub fn on_frame(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.on_frame.lock() = Some(Box::new(f));
    }

    pub fn frame(&self) -> Option<Arc<Frame>> {
        self.frame.lock().clone()
    }

    pub fn set_size(&self, w: u32, h: u32) {
        *self.size.lock() = (w.max(16), h.max(16));
    }

    pub fn play(&self, url: &str, start: Option<f64>) {
        *self.frame.lock() = None;
        if let Some(s) = start {
            let _ = self.mpv.set_property("start", format!("{s}"));
        } else {
            let _ = self.mpv.set_property("start", "none");
        }
        let _ = self.mpv.command("loadfile", &[url, "replace"]);
        let _ = self.mpv.set_property("pause", false);
    }

    pub fn set_paused(&self, paused: bool) {
        let _ = self.mpv.set_property("pause", paused);
    }

    pub fn set_muted(&self, muted: bool) {
        let _ = self.mpv.set_property("mute", muted);
    }

    pub fn stop(&self) {
        let _ = self.mpv.command("stop", &[]);
        *self.frame.lock() = None;
    }

    pub fn is_playing(&self) -> bool {
        self.mpv.get_property::<bool>("idle-active").map(|i| !i).unwrap_or(false)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.shared.cv.notify_all();
        unsafe { sys::mpv_render_context_free(self.ctx) };
    }
}
