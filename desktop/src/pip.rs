//! Picture in picture: the whole window shrinks into a small, always-on-top
//! corner of the screen and keeps playing; again (or ⤢) brings it back.
use std::sync::atomic::{AtomicBool, Ordering};

static ON: AtomicBool = AtomicBool::new(false);

pub fn is_on() -> bool {
    ON.load(Ordering::Relaxed)
}

#[cfg(target_os = "windows")]
mod imp {
    use parking_lot::Mutex;
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW, IsZoomed, ShowWindow, SW_RESTORE, SW_MAXIMIZE};

    static RESTORE: Mutex<Option<(RECT, bool)>> = Mutex::new(None);

    pub fn set(hwnd: isize, on: bool) {
        let h = HWND(hwnd as *mut _);
        unsafe {
            if on {
                let mut r = RECT::default();
                let _ = GetWindowRect(h, &mut r);
                let max = IsZoomed(h).as_bool();
                *RESTORE.lock() = Some((r, max));
                if max {
                    let _ = ShowWindow(h, SW_RESTORE);
                }
                let mon = MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST);
                let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
                let _ = GetMonitorInfoW(mon, &mut mi);
                let wa = mi.rcWork;
                // 480×270 logical, scaled for the screen
                let scale = windows::Win32::UI::HiDpi::GetDpiForWindow(h) as f32 / 96.;
                let (w, hgt) = ((480. * scale) as i32, (270. * scale) as i32);
                let m = (24. * scale) as i32;
                let _ = SetWindowPos(h, Some(HWND_TOPMOST), wa.right - w - m, wa.bottom - hgt - m, w, hgt, SWP_SHOWWINDOW | SWP_NOACTIVATE);
            } else if let Some((r, max)) = RESTORE.lock().take() {
                let _ = SetWindowPos(h, Some(HWND_NOTOPMOST), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_SHOWWINDOW);
                if max {
                    let _ = ShowWindow(h, SW_MAXIMIZE);
                }
            }
        }
    }
}

pub fn toggle(cx: &mut gpui::App) {
    let on = !is_on();
    ON.store(on, Ordering::Relaxed);
    #[cfg(target_os = "windows")]
    // after this frame: moving the window sends it messages right away,
    // which can't land while couch is in the middle of an update
    match crate::mini::main_hwnd() {
        0 => {}
        h => cx.defer(move |_| imp::set(h, on)),
    }
    #[cfg(not(target_os = "windows"))]
    let _ = cx;
}
