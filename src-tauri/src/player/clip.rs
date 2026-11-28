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
    /// the file has started (its first picture decoded); "ready" waits until
    /// it's actually moving, so nothing fades over a frozen or black frame
    armed: AtomicBool,
    /// held on its first picture (paused) until it's let go: for the scenes
    /// game, where picture and sound have to start together
    held: AtomicBool,
    /// the first picture of the current file has been drawn
    restarted: AtomicBool,
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
            if let Some(y) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf())).and_then(|d| ["yt-dlp.exe", "yt-dlp"].iter().map(|n| d.join(n)).find(|p| p.exists())) {
                i.set_property("script-opts", format!("ytdl_hook-ytdl_path={}", y.to_string_lossy()))?;
            }
            Ok(())
        })
        .map_err(|e| format!("{e:?}"))?;
        let me = Arc::new(Self { mpv: Arc::new(mpv), wid, ready: AtomicBool::new(false), gen: AtomicU64::new(0), on_ready: Mutex::new(None), armed: AtomicBool::new(false), held: AtomicBool::new(false), restarted: AtomicBool::new(false) });
        let w = Arc::downgrade(&me);
        let mpv = me.mpv.clone();
        std::thread::Builder::new()
            .name("couch-clip".into())
            .spawn(move || loop {
                let armed = w.upgrade().map_or(false, |m| m.armed.load(Ordering::Relaxed) && !m.ready.load(Ordering::Relaxed));
                let ev = mpv.wait_event(if armed { 0.05 } else { 0.5 });
                let Some(me) = w.upgrade() else { break };
                match ev {
                    // a file has started (this one: a new load clears `armed` first)
                    Some(Ok(Event::FileLoaded)) => me.armed.store(true, Ordering::Relaxed),
                    Some(Ok(Event::PlaybackRestart)) => {
                        me.armed.store(true, Ordering::Relaxed);
                        me.restarted.store(true, Ordering::Relaxed);
                    }
                    Some(Ok(Event::Shutdown)) => break,
                    _ => {}
                }
                // ready once the picture is really moving: time is passing, not stuck loading
                if me.armed.load(Ordering::Relaxed) && !me.ready.load(Ordering::Relaxed) {
                    let t: f64 = me.mpv.get_property("time-pos").unwrap_or(0.0);
                    let waiting: bool = me.mpv.get_property("paused-for-cache").unwrap_or(false);
                    let w_: i64 = me.mpv.get_property("width").unwrap_or(0);
                    // (held: ready as soon as the first picture is drawn, still paused)
                    let go = if me.held.load(Ordering::Relaxed) { me.restarted.load(Ordering::Relaxed) } else { t > 0.25 };
                    if go && !waiting && w_ > 0 && !me.ready.swap(true, Ordering::Relaxed) {
                        me.fit();
                        if let Some(f) = me.on_ready.lock().as_ref() {
                            f();
                        }
                    }
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
        self.play_with(url, muted, true);
    }

    /// Play once (it holds on the last frame; `ended` then says so) or loop.
    pub fn play_with(&self, url: &str, muted: bool, looping: bool) {
        self.gen.fetch_add(1, Ordering::Relaxed);
        self.ready.store(false, Ordering::Relaxed);
        self.armed.store(false, Ordering::Relaxed);
        self.held.store(false, Ordering::Relaxed);
        self.restarted.store(false, Ordering::Relaxed);
        let _ = self.mpv.set_property("loop-file", if looping { "inf" } else { "no" });
        let _ = self.mpv.set_property("mute", muted);
        let _ = self.mpv.set_property("volume", 100i64);
        let _ = self.mpv.command("loadfile", &[url, "replace"]);
        let _ = self.mpv.set_property("pause", false);
    }

    pub fn stop(&self) {
        self.ready.store(false, Ordering::Relaxed);
        self.armed.store(false, Ordering::Relaxed);
        let _ = self.mpv.command("stop", &[]);
    }

    pub fn set_paused(&self, paused: bool) {
        let _ = self.mpv.set_property("pause", paused);
    }

    /// Right after `play_with`: load it paused and call it ready on its first
    /// picture. `set_paused(false)` then starts picture and sound together.
    pub fn hold(&self) {
        self.held.store(true, Ordering::Relaxed);
        let _ = self.mpv.set_property("pause", true);
    }

    pub fn set_muted(&self, muted: bool) {
        let _ = self.mpv.set_property("mute", muted);
    }

    /// a play-once file reached its end
    pub fn ended(&self) -> bool {
        self.mpv.get_property::<bool>("eof-reached").unwrap_or(false)
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// mpv's own window: exactly the size of the spot it was moved to.
    pub fn fit(&self) {
        super::refit_video(Some(self.wid as isize));
    }
}
