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
    static HOOKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    /// couch's window won't go below 900×560 (its normal minimum); while it's
    /// the small picture, that minimum steps aside
    unsafe extern "system" fn sub(h: HWND, msg: u32, wp: windows::Win32::Foundation::WPARAM, lp: windows::Win32::Foundation::LPARAM, _id: usize, _data: usize) -> windows::Win32::Foundation::LRESULT {
        let r = unsafe { windows::Win32::UI::Shell::DefSubclassProc(h, msg, wp, lp) };
        if msg == windows::Win32::UI::WindowsAndMessaging::WM_GETMINMAXINFO && super::is_on() {
            let mi = lp.0 as *mut windows::Win32::UI::WindowsAndMessaging::MINMAXINFO;
            if !mi.is_null() {
                unsafe {
                    (*mi).ptMinTrackSize.x = 200;
                    (*mi).ptMinTrackSize.y = 112;
                }
            }
        }
        r
    }

    pub fn set(hwnd: isize, on: bool) {
        let h = HWND(hwnd as *mut _);
        unsafe {
            if !HOOKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                let _ = windows::Win32::UI::Shell::SetWindowSubclass(h, Some(sub), 0xC0C4, 0);
            }
            if on {
                let mut r = RECT::default();
                let _ = GetWindowRect(h, &mut r);
                let max = IsZoomed(h).as_bool();
                *RESTORE.lock() = Some((r, max));
                if max {
                    let _ = ShowWindow(h, SW_RESTORE);
                }
                // full screen (a borderless window covering the monitor) counts as big too
                let style = windows::Win32::UI::WindowsAndMessaging::GetWindowLongW(h, windows::Win32::UI::WindowsAndMessaging::GWL_STYLE) as u32;
                let _ = style;
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
        h => {
            cx.defer(move |_| imp::set(h, on));
            // the picture follows the window once it has settled at its new size
            // (Windows resizes in steps; one late fit missed it until a full-screen round trip)
            cx.spawn(async move |cx| {
                for ms in [40u64, 120, 300, 700] {
                    crate::timer::after(std::time::Duration::from_millis(ms)).await;
                    let _ = cx.update(|cx| {
                        cx.refresh_windows();
                        let s = cx.windows().first().and_then(|w| w.update(cx, |_, w, _| w.scale_factor()).ok()).unwrap_or(1.);
                        crate::mini::fit(s);
                    });
                }
            })
            .detach();
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = cx;
}

/// From the player: full screen steps down first (a full-screen window can't shrink into the corner).
pub fn toggle_in(window: &mut gpui::Window, cx: &mut gpui::App) {
    if !is_on() && window.is_fullscreen() {
        window.toggle_fullscreen();
        cx.spawn(async move |cx| {
            crate::timer::after(std::time::Duration::from_millis(280)).await;
            let _ = cx.update(|cx| toggle(cx));
        })
        .detach();
    } else {
        toggle(cx);
    }
}
