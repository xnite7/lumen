//! The film on a Mac (see mac/video.m): mpv renders through its render API
//! into an OpenGL layer in a view just under the app's own view.
use std::ffi::c_void;

extern "C" {
    fn couch_mac_video_attach(ui_view: *mut c_void);
    fn couch_mac_video_set_mpv(mpv: *mut c_void);
    fn couch_mac_video_place(x: f64, y: f64, w: f64, h: f64, radius: f64, visible: i32);
}

/// The window's own view (GPUI's NSView): the film's view goes under it.
pub fn attach(ui_view: isize) {
    unsafe { couch_mac_video_attach(ui_view as *mut c_void) }
}

/// mpv (made with vo=libmpv) is ready to be drawn.
pub(crate) fn set_mpv(mpv: *mut c_void) {
    unsafe { couch_mac_video_set_mpv(mpv) }
}

/// Where the film shows: `None` the whole window; `Some((x, y, w, h, radius))`
/// a box (the mini player), in points from the window's top left.
pub fn place(at: Option<(f32, f32, f32, f32, f32)>, visible: bool) {
    let (x, y, w, h, r) = at.unwrap_or((0., 0., 0., 0., 0.));
    unsafe { couch_mac_video_place(x as f64, y as f64, w as f64, h as f64, r as f64, visible as i32) }
}
