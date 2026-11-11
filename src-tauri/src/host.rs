//! What the engine needs from whatever window it lives in: send an event to
//! the UI, open a link, find the native window (mpv draws into it), move the
//! window around, show a system notification. The Tauri app and the GPUI app
//! each provide one.
use serde_json::Value;

pub trait Host: Send + Sync + 'static {
    /// tell the UI something happened ("library-changed", "player-state"...)
    fn emit(&self, event: &str, payload: Value);
    /// open a link in the user's browser
    fn open_url(&self, url: &str) -> Result<(), String>;
    /// the main window's native handle (HWND on Windows), for mpv to draw into
    fn main_window(&self) -> Option<isize>;
    /// minimize / maximize / close / fullscreen / pip ...
    fn window(&self, action: &str, arg: Value) -> Result<(), String>;
    /// a system notification
    fn notify(&self, title: &str, body: &str);
    /// where the window sits on its monitor, in physical pixels (eye tracking)
    fn geometry(&self) -> Option<Geometry> {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    pub monitor_x: f64,
    pub monitor_y: f64,
    pub monitor_w: f64,
    pub monitor_h: f64,
    pub inner_x: f64,
    pub inner_y: f64,
    pub inner_w: f64,
    pub inner_h: f64,
    pub scale: f64,
}

/// A host that does nothing (tests, headless tools).
pub struct NoHost;
impl Host for NoHost {
    fn emit(&self, _: &str, _: Value) {}
    fn open_url(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn main_window(&self) -> Option<isize> {
        None
    }
    fn window(&self, _: &str, _: Value) -> Result<(), String> {
        Ok(())
    }
    fn notify(&self, _: &str, _: &str) {}
}
