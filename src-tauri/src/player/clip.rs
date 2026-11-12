//! UI video on the GPU (billboard clips, card previews, trailers): an mpv of
//! its own drawing straight into a small window that the app places under
//! a see-through spot in its UI. Decoded and scaled on the graphics card,
//! unlike `surface` (software, drawn as an image).
use libmpv2::events::Event;
use libmpv2::Mpv;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

pub struct Clip {
    mpv: Arc<Mpv>,
    wid: i64,
    /// a picture is on screen for the current file
    ready: AtomicBool,
    /// bumped per `play`, so a late "ready" for an old file is ignored
    gen: AtomicU64,
    on_ready: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

impl Clip {
    /// `wid`: the window mpv draws into.
    pub fn new(wid: i64) -> Result<Arc<Self>, String> {
        let mpv = Mpv::with_initializer(|i| {
            i.set_property("wid", wid)?;
            i.set_property("vo", "gpu-next,gpu,")?;
            i.set_property("hwdec", "auto-safe")?;
            i.set_property("mute", "yes")?;
            i.set_property("loop-file", "inf")?;
            i.set_property("keep-open", "yes")?;
            i.set_property("idle", "yes")?;
            i.set_property("force-window", "yes")?;
            i.set_property("osc", "no")?;
            i.set_property("osd-level", "0")?;
            i.set_property("input-default-bindings", "no")?;
            i.set_property("input-vo-keyboard", "no")?;
            i.set_property("input-cursor", "no")?;
            i.set_property("cursor-autohide", "no")?;
            i.set_property("background-color", "#000000")?;
            // fill the spot (crop the edges), like object-fit: cover
            i.set_property("panscan", "1.0")?;
            i.set_property("audio-client-name", "couch")?;
            i.set_property("terminal", "no")?;
            i.set_property("ytdl", "yes")?;
            i.set_property("ytdl-format", "bestvideo[height<=1080]+bestaudio/best[height<=1080]/best")?;
            if let Some(y) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("yt-dlp.exe"))).filter(|p| p.exists()) {
                i.set_property("script-opts", format!("ytdl_hook-ytdl_path={}", y.to_string_lossy()))?;
            }
            Ok(())
        })
        .map_err(|e| format!("{e:?}"))?;
        let me = Arc::new(Self { mpv: Arc::new(mpv), wid, ready: AtomicBool::new(false), gen: AtomicU64::new(0), on_ready: Mutex::new(None) });
        let w = Arc::downgrade(&me);
        let mpv = me.mpv.clone();
        std::thread::Builder::new()
            .name("couch-clip".into())
            .spawn(move || loop {
                let ev = mpv.wait_event(0.5);
                let Some(me) = w.upgrade() else { break };
                match ev {
                    // the first picture of a file is up
                    Some(Ok(Event::PlaybackRestart)) => {
                        if !me.ready.swap(true, Ordering::Relaxed) {
                            me.fit();
                            if let Some(f) = me.on_ready.lock().as_ref() {
                                f();
                            }
                        }
                    }
                    Some(Ok(Event::Shutdown)) => break,
                    _ => {}
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(me)
    }

    /// Called (on mpv's event thread) when a new file has its first picture up.
    pub fn on_ready(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.on_ready.lock() = Some(Box::new(f));
    }

    pub fn play(&self, url: &str, muted: bool) {
        self.gen.fetch_add(1, Ordering::Relaxed);
        self.ready.store(false, Ordering::Relaxed);
        let _ = self.mpv.set_property("mute", muted);
        let _ = self.mpv.command("loadfile", &[url, "replace"]);
        let _ = self.mpv.set_property("pause", false);
    }

    pub fn stop(&self) {
        self.ready.store(false, Ordering::Relaxed);
        let _ = self.mpv.command("stop", &[]);
    }

    pub fn set_paused(&self, paused: bool) {
        let _ = self.mpv.set_property("pause", paused);
    }

    pub fn set_muted(&self, muted: bool) {
        let _ = self.mpv.set_property("mute", muted);
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// mpv's own window: exactly the size of the spot it was moved to.
    pub fn fit(&self) {
        super::refit_video(Some(self.wid as isize));
    }
}
