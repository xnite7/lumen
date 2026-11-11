//! The Tauri app as couch's host: events go to the webview, links open with
//! the opener plugin, the window is the "main" webview window.
use crate::error::{AppError, AppResult};
use crate::host::{Geometry, Host};
use once_cell::sync::OnceCell;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

static APP: OnceCell<AppHandle> = OnceCell::new();

pub fn app() -> AppHandle {
    APP.get().expect("tauri app not ready").clone()
}

pub struct TauriHost;

impl TauriHost {
    pub fn new(app: AppHandle) -> Self {
        let _ = APP.set(app);
        TauriHost
    }
}

impl Host for TauriHost {
    fn emit(&self, event: &str, payload: Value) {
        let _ = app().emit(event, payload);
    }
    fn open_url(&self, url: &str) -> Result<(), String> {
        use tauri_plugin_opener::OpenerExt;
        app().opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
    }
    fn main_window(&self) -> Option<isize> {
        let w = app().get_webview_window("main")?;
        #[cfg(target_os = "windows")]
        {
            return w.hwnd().ok().map(|h| h.0 as isize);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = w;
            None
        }
    }
    fn window(&self, action: &str, arg: Value) -> Result<(), String> {
        if action == "pip" {
            return pip(arg.as_bool().unwrap_or(false)).map_err(|e| e.to_string());
        }
        window_action(action).map_err(|e| e.to_string())
    }
    fn notify(&self, title: &str, body: &str) {
        #[cfg(desktop)]
        {
            use tauri_plugin_notification::NotificationExt;
            let _ = app().notification().builder().title(title).body(body).show();
        }
        #[cfg(mobile)]
        let _ = (title, body);
    }
    fn geometry(&self) -> Option<Geometry> {
        let w = app().get_webview_window("main")?;
        let mon = w.current_monitor().ok()??;
        let scale = w.scale_factor().ok()?;
        let (mp, ms) = (mon.position(), mon.size());
        let ip = w.inner_position().ok()?;
        let is = w.inner_size().ok()?;
        Some(Geometry {
            monitor_x: mp.x as f64,
            monitor_y: mp.y as f64,
            monitor_w: ms.width as f64,
            monitor_h: ms.height as f64,
            inner_x: ip.x as f64,
            inner_y: ip.y as f64,
            inner_w: is.width as f64,
            inner_h: is.height as f64,
            scale,
        })
    }
}

fn window_action(action: &str) -> AppResult<()> {
    #[cfg(desktop)]
    {
        let w = app().get_webview_window("main").ok_or_else(|| AppError::msg("no window"))?;
        match action {
            "minimize" => w.minimize()?,
            "maximize" => {
                if w.is_maximized()? {
                    w.unmaximize()?
                } else {
                    w.maximize()?
                }
            }
            "close" => w.close()?,
            "fullscreen" => w.set_fullscreen(!w.is_fullscreen()?)?,
            "exit-fullscreen" => w.set_fullscreen(false)?,
            "drag" => w.start_dragging()?,
            _ => {}
        }
    }
    #[cfg(mobile)]
    let _ = action;
    Ok(())
}

/// Picture-in-picture for a native player is just a small, always-on-top,
/// borderless window in the corner -- mpv keeps rendering into it.
fn pip(on: bool) -> AppResult<()> {
    #[cfg(desktop)]
    {
        use tauri::{LogicalPosition, LogicalSize, Position, Size};
        let w = app().get_webview_window("main").ok_or_else(|| AppError::msg("no window"))?;
        let core = crate::state::get().core.clone();
        if on {
            if w.is_fullscreen()? {
                w.set_fullscreen(false)?;
            }
            let scale = w.scale_factor()?;
            let pos = w.outer_position()?.to_logical::<f64>(scale);
            let size = w.inner_size()?.to_logical::<f64>(scale);
            core.db.set("pip_restore", &json!({ "x": pos.x, "y": pos.y, "w": size.width, "h": size.height, "max": w.is_maximized()? }))?;
            if w.is_maximized()? {
                w.unmaximize()?;
            }
            let (pw, ph) = (480.0, 270.0);
            let mon = w.current_monitor()?.ok_or_else(|| AppError::msg("no monitor"))?;
            let ms = mon.size().to_logical::<f64>(mon.scale_factor());
            let mp = mon.position().to_logical::<f64>(mon.scale_factor());
            w.set_always_on_top(true)?;
            w.set_size(Size::Logical(LogicalSize::new(pw, ph)))?;
            w.set_position(Position::Logical(LogicalPosition::new(mp.x + ms.width - pw - 28.0, mp.y + ms.height - ph - 72.0)))?;
        } else {
            w.set_always_on_top(false)?;
            if let Some(r) = core.db.get::<Value>("pip_restore") {
                w.set_size(Size::Logical(LogicalSize::new(r["w"].as_f64().unwrap_or(1280.0), r["h"].as_f64().unwrap_or(800.0))))?;
                w.set_position(Position::Logical(LogicalPosition::new(r["x"].as_f64().unwrap_or(80.0), r["y"].as_f64().unwrap_or(80.0))))?;
                if r["max"].as_bool().unwrap_or(false) {
                    w.maximize()?;
                }
            }
        }
    }
    #[cfg(mobile)]
    let _ = on;
    Ok(())
}
