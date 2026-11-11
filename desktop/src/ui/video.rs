//! UI video (billboard clips, card previews): one muted mpv rendered in
//! software, shown like an image. Whoever asked last owns it.
use futures::StreamExt;
use gpui::{App, AppContext, Context, Entity, Global, RenderImage, Window};
use std::sync::Arc;
use std::time::Instant;

pub struct Video {
    surface: Option<Arc<couch::player::surface::Surface>>,
    /// who's showing ("hero:tt…", "card:tt…")
    pub owner: String,
    image: Option<Arc<RenderImage>>,
    serial: u64,
    /// when the first frame of this clip arrived
    pub since: Option<Instant>,
    old: Vec<Arc<RenderImage>>,
}

pub struct VideoHandle(pub Entity<Video>);
impl Global for VideoHandle {}

pub fn get(cx: &App) -> Entity<Video> {
    cx.global::<VideoHandle>().0.clone()
}

pub fn init(cx: &mut App) {
    let v = cx.new(|cx: &mut Context<Video>| {
        let surface = couch::player::surface::Surface::new(640, 360).map_err(|e| log::warn!("ui video off: {e}")).ok();
        if let Some(s) = &surface {
            let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
            s.on_frame(move || {
                let _ = tx.unbounded_send(());
            });
            cx.spawn(async move |this, cx| {
                while rx.next().await.is_some() {
                    // a burst of frames is one repaint
                    while let Ok(Some(_)) = rx.try_next() {}
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        Video { surface, owner: String::new(), image: None, serial: 0, since: None, old: vec![] }
    });
    cx.set_global(VideoHandle(v));
}

impl Video {
    /// Play `url` for `owner` (muted, looping).
    pub fn play(&mut self, owner: &str, url: &str, cx: &mut Context<Self>) {
        let Some(s) = &self.surface else { return };
        // the billboard gets a sharper picture than a card
        if owner.starts_with("hero:") {
            s.set_size(1280, 720);
        } else {
            s.set_size(640, 360);
        }
        self.owner = owner.to_string();
        self.since = None;
        if let Some(i) = self.image.take() {
            self.old.push(i);
        }
        s.play(url, None);
        cx.notify();
    }

    /// Stop, if `owner` is still the one showing.
    pub fn stop(&mut self, owner: &str, cx: &mut Context<Self>) {
        if self.owner != owner {
            return;
        }
        self.owner.clear();
        self.since = None;
        if let Some(s) = &self.surface {
            s.stop();
        }
        if let Some(i) = self.image.take() {
            self.old.push(i);
        }
        cx.notify();
    }

    pub fn set_muted(&self, muted: bool) {
        if let Some(s) = &self.surface {
            s.set_muted(muted);
        }
    }

    /// The current frame for `owner` (None while loading or not theirs).
    pub fn frame(&mut self, owner: &str, window: &mut Window) -> Option<Arc<RenderImage>> {
        // free the frames that were replaced (GPU atlas)
        for i in self.old.drain(..) {
            let _ = window.drop_image(i);
        }
        if self.owner != owner {
            return None;
        }
        let s = self.surface.as_ref()?;
        let serial = s.serial.load(std::sync::atomic::Ordering::Relaxed);
        if serial != self.serial {
            self.serial = serial;
            if let Some(f) = s.frame() {
                if let Some(buf) = image::RgbaImage::from_raw(f.width, f.height, f.bgra.clone()) {
                    let img = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(buf)]));
                    if let Some(prev) = self.image.replace(img) {
                        let _ = window.drop_image(prev);
                    }
                    if self.since.is_none() {
                        self.since = Some(Instant::now());
                    }
                }
            }
        }
        self.image.clone()
    }
}

// ---------------------------------------------------------- card previews
// Rest on a card for a moment and it plays a clip in place. Each hover starts
// fresh; leaving the card stops it.

struct Pending(String);
impl Global for Pending {}

pub fn hover(m: &crate::api::Meta, on: bool, cx: &mut App) {
    let owner = format!("card:{}", m.id);
    if !on {
        if cx.try_global::<Pending>().map_or(false, |p| p.0 == owner) {
            cx.set_global(Pending(String::new()));
        }
        get(cx).update(cx, |v, cx| v.stop(&owner, cx));
        return;
    }
    cx.set_global(Pending(owner.clone()));
    let delay = couch::state::get().core.settings().general.hover_delay_ms.max(300) as u64;
    let (id, name, year) = (m.id.clone(), m.name.clone(), m.year());
    cx.spawn(async move |cx| {
        gpui::Timer::after(std::time::Duration::from_millis(delay)).await;
        let still = cx.update(|cx| cx.try_global::<Pending>().map_or(false, |p| p.0 == owner)).unwrap_or(false);
        if !still {
            return;
        }
        let url = match std::env::var("COUCH_TEST_CLIP") {
            Ok(f) => Some(f),
            Err(_) => crate::api::call::<Option<String>>("clip_url", serde_json::json!({ "id": id, "title": name, "year": year })).await.ok().flatten(),
        };
        let Some(url) = url else { return };
        let _ = cx.update(|cx| {
            if cx.try_global::<Pending>().map_or(false, |p| p.0 == owner) {
                get(cx).update(cx, |v, cx| v.play(&owner, &url, cx));
            }
        });
    })
    .detach();
}
