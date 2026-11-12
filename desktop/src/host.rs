//! couch's engine talks to the window through this: events land on a
//! channel the UI drains, links open in the browser, and mpv finds the
//! window's native handle here.
use couch::host::{Geometry, Host};
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use parking_lot::Mutex;
use serde_json::Value;
use std::sync::atomic::{AtomicIsize, Ordering};

pub enum Msg {
    Event(String, Value),
    OpenUrl(String),
    Window(String, Value),
    Notify(String, String),
}

pub struct GpuiHost {
    tx: UnboundedSender<Msg>,
    pub hwnd: AtomicIsize,
    pub geometry: Mutex<Option<Geometry>>,
}

impl GpuiHost {
    pub fn new() -> (Self, UnboundedReceiver<Msg>) {
        let (tx, rx) = unbounded();
        (Self { tx, hwnd: AtomicIsize::new(0), geometry: Mutex::new(None) }, rx)
    }
}

/// Anyone in the UI can listen for an engine event by name.
static SUBS: Mutex<Vec<(String, UnboundedSender<Value>)>> = Mutex::new(Vec::new());

pub fn listen(event: &str) -> UnboundedReceiver<Value> {
    let (tx, rx) = unbounded();
    SUBS.lock().push((event.to_string(), tx));
    rx
}

impl Host for GpuiHost {
    fn emit(&self, event: &str, payload: Value) {
        SUBS.lock().retain(|(name, tx)| name != event || tx.unbounded_send(payload.clone()).is_ok());
        let _ = self.tx.unbounded_send(Msg::Event(event.to_string(), payload));
    }
    fn open_url(&self, url: &str) -> Result<(), String> {
        self.tx.unbounded_send(Msg::OpenUrl(url.to_string())).map_err(|e| e.to_string())
    }
    fn main_window(&self) -> Option<isize> {
        // mpv draws into the mini player's host window once it exists
        let v = crate::mini::video_hwnd();
        if v != 0 {
            return Some(v);
        }
        match self.hwnd.load(Ordering::Relaxed) {
            0 => None,
            h => Some(h),
        }
    }
    fn window(&self, action: &str, arg: Value) -> Result<(), String> {
        self.tx.unbounded_send(Msg::Window(action.to_string(), arg)).map_err(|e| e.to_string())
    }
    fn notify(&self, title: &str, body: &str) {
        let _ = self.tx.unbounded_send(Msg::Notify(title.to_string(), body.to_string()));
    }
    fn geometry(&self) -> Option<Geometry> {
        *self.geometry.lock()
    }
}
