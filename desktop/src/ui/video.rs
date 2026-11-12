//! UI video (billboard clips, card previews, trailers). On Windows it plays
//! on the graphics card: its own mpv draws into a little window that sits
//! under a see-through spot in couch's UI (see `spot` and `paint_bg`).
//! Elsewhere (and as a fallback) it's one mpv rendered in software and shown
//! like an image. Whoever asked last owns it.
use futures::StreamExt;
use gpui::{canvas, div, img, prelude::*, px, AnyElement, App, AppContext, Bounds, Context, Entity, Global, ObjectFit, Pixels, RenderImage, Window};
use std::cell::Cell;
use std::sync::Arc;
use std::time::Instant;

/// What a clip shows right now.
#[derive(Clone)]
pub enum Pic {
    /// a frame drawn by couch (software)
    Image(Arc<RenderImage>),
    /// the GPU window is showing it under a see-through spot
    Native,
}

pub struct Video {
    surface: Option<Arc<couch::player::surface::Surface>>,
    #[cfg(target_os = "windows")]
    native: Option<Arc<couch::player::clip::Clip>>,
    /// who's showing ("hero:tt…", "card:tt…", "trailer:tt…")
    pub owner: String,
    image: Option<Arc<RenderImage>>,
    serial: u64,
    /// when the first frame of this clip arrived
    pub since: Option<Instant>,
    old: Vec<Arc<RenderImage>>,
    ready_tx: Option<futures::channel::mpsc::UnboundedSender<()>>,
    pub paused: bool,
}

/// Is this kind of clip muted? (trailers never; billboard / previews by setting)
pub fn is_muted(owner: &str) -> bool {
    let g = couch::state::get().core.settings().general;
    if owner.starts_with("trailer:") {
        false
    } else if owner.starts_with("hero:") {
        g.hero_muted
    } else {
        g.preview_muted
    }
}

/// The speaker on the billboard / a preview: flips its sound switch (kept).
pub fn toggle_sound(owner: &str, cx: &mut App) {
    let core = couch::state::get().core.clone();
    let mut s = core.settings();
    let hero = owner.starts_with("hero:");
    let now = if hero { &mut s.general.hero_muted } else { &mut s.general.preview_muted };
    *now = !*now;
    let muted = *now;
    let v = serde_json::to_value(&s).unwrap_or_default();
    *core.settings.write() = s;
    couch::rt::spawn(async move {
        let _ = couch::dispatch::invoke("settings_set", serde_json::json!({ "settings": v })).await;
    });
    let vid = get(cx);
    vid.update(cx, |x, cx| {
        if x.owner.starts_with(if hero { "hero:" } else { "card:" }) {
            x.set_muted(muted);
        }
        cx.notify();
    });
}

/// A round speaker button for a playing clip.
pub fn sound_button(owner: String, size: f32) -> impl IntoElement {
    let muted = is_muted(&owner);
    div()
        .id(gpui::SharedString::from(format!("snd-{owner}")))
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(crate::theme::black(0.45))
        .border_1()
        .border_color(crate::theme::white(0.35))
        .cursor_pointer()
        .hover(|s| s.bg(crate::theme::black(0.7)))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            toggle_sound(&owner, cx)
        })
        .child(crate::ui::icon(if muted { "mute" } else { "volume" }, size * 0.48))
}

pub struct VideoHandle(pub Entity<Video>);
impl Global for VideoHandle {}

pub fn get(cx: &App) -> Entity<Video> {
    cx.global::<VideoHandle>().0.clone()
}

pub fn init(cx: &mut App) {
    let v = cx.new(|cx: &mut Context<Video>| {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
        // the software surface is made only when there's no GPU path
        let surface = if cfg!(target_os = "windows") { None } else { couch::player::surface::Surface::new(640, 360).map_err(|e| log::warn!("ui video off: {e}")).ok() };
        if let Some(s) = &surface {
            let tx = tx.clone();
            s.on_frame(move || {
                let _ = tx.unbounded_send(());
            });
        }
        cx.spawn(async move |this, cx| {
            while rx.next().await.is_some() {
                // a burst of frames is one repaint
                while let Ok(Some(_)) = rx.try_next() {}
                if this
                    .update(cx, |v, cx| {
                        #[cfg(target_os = "windows")]
                        if v.since.is_none() && v.native.as_ref().map_or(false, |n| n.is_ready()) {
                            v.since = Some(Instant::now());
                        }
                        cx.notify()
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Video {
            surface,
            #[cfg(target_os = "windows")]
            native: None,
            owner: String::new(),
            image: None,
            serial: 0,
            since: None,
            old: vec![],
            ready_tx: Some(tx),
            paused: false,
        }
    });
    cx.set_global(VideoHandle(v));
}

impl Video {
    /// the GPU player, made the first time it's needed (the window must exist)
    #[cfg(target_os = "windows")]
    fn native(&mut self) -> Option<Arc<couch::player::clip::Clip>> {
        if self.native.is_none() {
            let host = crate::mini::clip_hwnd();
            if host == 0 {
                return None;
            }
            match couch::player::clip::Clip::new(host as i64) {
                Ok(c) => {
                    if let Some(tx) = self.ready_tx.clone() {
                        c.on_ready(move || {
                            let _ = tx.unbounded_send(());
                        });
                    }
                    self.native = Some(c);
                }
                Err(e) => log::warn!("gpu clips off: {e}"),
            }
        }
        self.native.clone()
    }

    /// Play `url` for `owner` (looping). Trailers have sound; the billboard
    /// and previews follow their sound switch (the speaker button on them).
    pub fn play(&mut self, owner: &str, url: &str, cx: &mut Context<Self>) {
        let muted = is_muted(owner);
        self.paused = false;
        self.owner = owner.to_string();
        self.since = None;
        if let Some(i) = self.image.take() {
            self.old.push(i);
        }
        #[cfg(target_os = "windows")]
        if let Some(n) = self.native() {
            n.play(url, muted);
            cx.notify();
            return;
        }
        if self.surface.is_none() {
            self.surface = couch::player::surface::Surface::new(640, 360).map_err(|e| log::warn!("ui video off: {e}")).ok();
            if let (Some(s), Some(tx)) = (&self.surface, self.ready_tx.clone()) {
                s.on_frame(move || {
                    let _ = tx.unbounded_send(());
                });
            }
        }
        let Some(s) = &self.surface else { return };
        s.set_muted(muted);
        // the billboard gets a sharper picture than a card
        if owner.starts_with("hero:") || owner.starts_with("trailer:") {
            s.set_size(1280, 720);
        } else {
            s.set_size(640, 360);
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
        #[cfg(target_os = "windows")]
        if let Some(n) = &self.native {
            n.stop();
        }
        if let Some(s) = &self.surface {
            s.stop();
        }
        if let Some(i) = self.image.take() {
            self.old.push(i);
        }
        cx.notify();
    }

    /// Stop whatever's showing if its owner starts with `prefix` (leaving a page).
    pub fn stop_prefix(&mut self, prefix: &str, cx: &mut Context<Self>) {
        if self.owner.starts_with(prefix) {
            let o = self.owner.clone();
            self.stop(&o, cx);
        }
    }

    /// Pause / resume, if `owner` is the one showing.
    pub fn set_paused(&mut self, owner: &str, paused: bool) {
        if self.owner != owner || self.paused == paused {
            return;
        }
        self.paused = paused;
        #[cfg(target_os = "windows")]
        if let Some(n) = &self.native {
            n.set_paused(paused);
        }
        if let Some(s) = &self.surface {
            s.set_paused(paused);
        }
    }

    pub fn set_muted(&self, muted: bool) {
        #[cfg(target_os = "windows")]
        if let Some(n) = &self.native {
            n.set_muted(muted);
        }
        if let Some(s) = &self.surface {
            s.set_muted(muted);
        }
    }

    /// What `owner`'s clip shows now (None while loading or not theirs).
    pub fn frame(&mut self, owner: &str, window: &mut Window) -> Option<Pic> {
        // free the frames that were replaced (GPU atlas)
        for i in self.old.drain(..) {
            let _ = window.drop_image(i);
        }
        if self.owner != owner {
            return None;
        }
        #[cfg(target_os = "windows")]
        if let Some(n) = &self.native {
            return n.is_ready().then_some(Pic::Native);
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
        self.image.clone().map(Pic::Image)
    }
}

// ------------------------------------------------- the see-through spot
// The element that shows a GPU clip leaves a hole in couch's background;
// the clip's window is moved under it after the frame is painted.

thread_local! {
    /// (the clip's whole area, the part of it on screen, corner radius)
    static HOLE: Cell<Option<(Bounds<Pixels>, Bounds<Pixels>, f32)>> = const { Cell::new(None) };
}

/// Start of a frame: no spot yet.
pub fn begin_frame() {
    HOLE.with(|h| h.set(None));
}

/// Where the GPU clip shows: fills its parent; `radius` rounds its corners.
pub fn spot(radius: f32) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let visible = bounds.intersect(&window.content_mask().bounds);
            if visible.size.width > px(1.) && visible.size.height > px(1.) {
                HOLE.with(|h| h.set(Some((bounds, visible, radius))));
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// couch's background, with the hole where a GPU clip shows; then the
/// clip's window goes under it (or away when nothing's showing).
pub fn paint_bg(color: gpui::Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, cx| {
            let hole = HOLE.with(|h| h.get());
            match hole {
                None => window.paint_quad(gpui::fill(bounds, color)),
                Some((full, r, radius)) => {
                    let (o, s) = (bounds.origin, bounds.size);
                    let quads = [
                        Bounds::new(o, gpui::size(s.width, r.top() - o.y)),
                        Bounds::new(gpui::point(o.x, r.bottom()), gpui::size(s.width, o.y + s.height - r.bottom())),
                        Bounds::new(gpui::point(o.x, r.top()), gpui::size(r.left() - o.x, r.size.height)),
                        Bounds::new(gpui::point(r.right(), r.top()), gpui::size(o.x + s.width - r.right(), r.size.height)),
                    ];
                    for q in quads {
                        if q.size.width > px(0.) && q.size.height > px(0.) {
                            window.paint_quad(gpui::fill(q, color));
                        }
                    }
                    // round the spot's corners: a ring in the background colour
                    if radius > 0. {
                        let d = px(radius);
                        window.paint_quad(gpui::quad(
                            full.dilate(d),
                            px(radius * 2.),
                            gpui::transparent_black(),
                            d,
                            color,
                            gpui::BorderStyle::Solid,
                        ));
                    }
                }
            }
            // the clip window follows (after this frame; it messages the window)
            let scale = window.scale_factor();
            // the window covers the whole clip (so it doesn't squash while
            // half off screen); its shape is just the part that shows
            let place = hole.map(|(full, r, rad)| {
                let p = |v: Pixels| (f32::from(v) * scale).round() as i32;
                crate::mini::ClipPlace {
                    x: p(full.origin.x),
                    y: p(full.origin.y),
                    w: p(full.size.width),
                    h: p(full.size.height),
                    vis: (p(r.origin.x - full.origin.x), p(r.origin.y - full.origin.y), p(r.size.width), p(r.size.height)),
                    r: (rad * scale).round() as i32,
                }
            });
            cx.defer(move |_| crate::mini::place_clip(place));
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// The clip on top of a picture, as children of the element that shows it:
/// software frames fade in over the art; a GPU clip leaves a spot and the
/// art fades out on top of it.
pub fn layers(art: Option<String>, pic: &Pic, a: f32, radius: f32) -> Vec<AnyElement> {
    let art_img = |op: f32| art.clone().map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).opacity(op).into_any_element());
    match pic {
        Pic::Image(f) => art_img(1.).into_iter().chain(std::iter::once(img(f.clone()).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover).opacity(a).into_any_element())).collect(),
        Pic::Native => std::iter::once(spot(radius).into_any_element()).chain(art_img(1. - a)).collect(),
    }
}

pub fn is_native(p: &Option<Pic>) -> bool {
    matches!(p, Some(Pic::Native))
}

#[allow(dead_code)]
fn _unused() -> gpui::Div {
    div()
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
        crate::timer::after(std::time::Duration::from_millis(delay)).await;
        let still = cx.update(|cx| cx.try_global::<Pending>().map_or(false, |p| p.0 == owner));
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
