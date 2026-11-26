//! The Mac's Now Playing and staying awake (see mac/system.m).
use std::ffi::CString;

extern "C" {
    fn couch_mac_np_commands(f: extern "C" fn(i32));
    fn couch_mac_np_set(open: i32, paused: i32, title: *const std::ffi::c_char, sub: *const std::ffi::c_char, pos: f64, duration: f64, has_next: i32);
    fn couch_mac_np_art(bytes: *const u8, len: usize);
    fn couch_mac_stay_awake(on: i32);
}

/// Presses from the media keys / Now Playing: 0 toggle, 1 play, 2 pause,
/// 3 next, 4 back 10 s, 5 stop, 6 forward 10 s.
pub fn on_press(f: extern "C" fn(i32)) {
    unsafe { couch_mac_np_commands(f) }
}

pub fn now_playing(open: bool, paused: bool, title: &str, sub: &str, pos: f64, duration: f64, has_next: bool) {
    let (t, s) = (CString::new(title.replace('\0', "")).unwrap_or_default(), CString::new(sub.replace('\0', "")).unwrap_or_default());
    unsafe { couch_mac_np_set(open as i32, paused as i32, t.as_ptr(), s.as_ptr(), pos, duration, has_next as i32) }
}

pub fn artwork(bytes: &[u8]) {
    unsafe { couch_mac_np_art(bytes.as_ptr(), bytes.len()) }
}

pub fn stay_awake(on: bool) {
    unsafe { couch_mac_stay_awake(on as i32) }
}
