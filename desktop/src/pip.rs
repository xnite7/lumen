//! Picture in picture: the whole window shrinks into a small, always-on-top
//! corner of the screen and keeps playing; again (or ⤢) brings it back.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]
use std::sync::atomic::{AtomicBool, Ordering};

static ON: AtomicBool = AtomicBool::new(false);
/// the film's shape (width / height), for resizing the little window
static ASPECT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// how wide the little window may get (logical px)
const MAX_W: f32 = 960.;

pub fn set_aspect(a: f32) {
    if a.is_finite() && a > 0.3 && a < 4. {
        ASPECT.store(a.to_bits(), Ordering::Relaxed);
    }
}
pub fn aspect() -> f32 {
    match f32::from_bits(ASPECT.load(Ordering::Relaxed)) {
        a if a > 0.3 => a,
        _ => 16. / 9.,
    }
}

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
                let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(h) } as f32 / 96.;
                unsafe {
                    (*mi).ptMinTrackSize.x = (260. * scale) as i32;
                    (*mi).ptMinTrackSize.y = (146. * scale) as i32;
                    // and never bigger than this: it stays a little picture
                    (*mi).ptMaxTrackSize.x = (super::MAX_W * scale) as i32;
                    (*mi).ptMaxTrackSize.y = (super::MAX_W * scale) as i32;
                }
            }
        }
        // dragging an edge: the film's shape holds (like holding Shift)
        if msg == windows::Win32::UI::WindowsAndMessaging::WM_SIZING && super::is_on() {
            let rc = lp.0 as *mut RECT;
            if !rc.is_null() {
                let a = super::aspect();
                let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(h) } as f32 / 96.;
                unsafe {
                    let r = &mut *rc;
                    let (w, hh) = ((r.right - r.left) as f32, (r.bottom - r.top) as f32);
                    // which edge moved: top/bottom drive the width, the rest drive the height
                    const WMSZ_TOP: usize = 3;
                    const WMSZ_BOTTOM: usize = 6;
                    let by_height = wp.0 == WMSZ_TOP || wp.0 == WMSZ_BOTTOM;
                    let max_w = super::MAX_W * scale;
                    let (mut nw, mut nh) = if by_height { (hh * a, hh) } else { (w, w / a) };
                    if nw > max_w {
                        nw = max_w;
                        nh = max_w / a;
                    }
                    let (nw, nh) = (nw.round() as i32, nh.round() as i32);
                    // keep the corner opposite the one being dragged in place
                    const WMSZ_LEFT: usize = 1;
                    const WMSZ_TOPLEFT: usize = 4;
                    const WMSZ_TOPRIGHT: usize = 5;
                    const WMSZ_BOTTOMLEFT: usize = 7;
                    match wp.0 {
                        WMSZ_LEFT | WMSZ_TOPLEFT | WMSZ_BOTTOMLEFT => r.left = r.right - nw,
                        _ => r.right = r.left + nw,
                    }
                    match wp.0 {
                        WMSZ_TOP | WMSZ_TOPLEFT | WMSZ_TOPRIGHT => r.top = r.bottom - nh,
                        _ => r.bottom = r.top + nh,
                    }
                }
                return windows::Win32::Foundation::LRESULT(1);
            }
        }
        r
    }

    /// What clicking another window and back did (that fixed it): a lost and
    /// regained focus, then GPUI's own "draw the whole window again" message.
    pub fn kick(hwnd: isize) {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_ACTIVATE, WM_USER};
        let h = HWND(hwnd as *mut _);
        unsafe {
            let _ = PostMessageW(Some(h), WM_ACTIVATE, WPARAM(0), LPARAM(0));
            let _ = PostMessageW(Some(h), WM_ACTIVATE, WPARAM(1), LPARAM(0));
            // (gpui's WM_GPUI_FORCE_UPDATE_WINDOW)
            let _ = PostMessageW(Some(h), WM_USER + 5, WPARAM(0), LPARAM(0));
        }
    }

    /// Move the window from `a` to `b` over a quarter second, easing out
    /// (then maximize, if asked). On its own thread: each step is a real move.
    fn glide(hwnd: isize, a: RECT, b: RECT, max_after: bool) {
        std::thread::spawn(move || {
            let h = HWND(hwnd as *mut _);
            const STEPS: i32 = 16;
            for i in 1..=STEPS {
                let x = i as f32 / STEPS as f32;
                let e = 1. - (1. - x).powi(3);
                let l = |p: i32, q: i32| p + ((q - p) as f32 * e).round() as i32;
                let (left, top, right, bottom) = (l(a.left, b.left), l(a.top, b.top), l(a.right, b.right), l(a.bottom, b.bottom));
                unsafe {
                    let _ = SetWindowPos(h, None, left, top, right - left, bottom - top, SWP_NOACTIVATE | windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER);
                }
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            if max_after {
                unsafe {
                    let _ = ShowWindow(h, SW_MAXIMIZE);
                }
            }
        });
    }

    /// one pixel wider and back: a real resize, end to end
    pub fn wiggle(hwnd: isize) {
        use windows::Win32::UI::WindowsAndMessaging::{SWP_NOMOVE, SWP_NOZORDER};
        let h = HWND(hwnd as *mut _);
        unsafe {
            if IsZoomed(h).as_bool() {
                return;
            }
            let mut r = RECT::default();
            let _ = GetWindowRect(h, &mut r);
            let (w, hh) = (r.right - r.left, r.bottom - r.top);
            let _ = SetWindowPos(h, None, 0, 0, w + 1, hh, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
            let _ = SetWindowPos(h, None, 0, 0, w, hh, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }

    /// While a game (or anything) is full screen in front, the little picture
    /// lets the mouse straight through to it: no stray clicks or hovers on
    /// couch while you play. Back to normal as soon as that's no longer so.
    static PASS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    static GUARD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    fn fullscreen_other_in_front(ours: HWND) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId};
        unsafe {
            let fg = GetForegroundWindow();
            if fg.is_invalid() || fg == ours {
                return false;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(fg, Some(&mut pid));
            if pid == std::process::id() {
                return false;
            }
            // the desktop and the taskbar aren't games
            let mut cls = [0u16; 64];
            let n = GetClassNameW(fg, &mut cls) as usize;
            let cls = String::from_utf16_lossy(&cls[..n.min(64)]);
            if matches!(cls.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
                return false;
            }
            let mut r = RECT::default();
            if GetWindowRect(fg, &mut r).is_err() {
                return false;
            }
            let mon = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            let _ = GetMonitorInfoW(mon, &mut mi);
            let m = mi.rcMonitor;
            r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
        }
    }

    fn pass_through(h: HWND, on: bool) {
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, GWL_EXSTYLE, LWA_ALPHA, WS_EX_LAYERED, WS_EX_TRANSPARENT};
        if PASS.swap(on, std::sync::atomic::Ordering::Relaxed) == on {
            return;
        }
        unsafe {
            let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
            if on {
                let _ = SetWindowLongW(h, GWL_EXSTYLE, (ex | WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0) as i32);
                let _ = SetLayeredWindowAttributes(h, windows::Win32::Foundation::COLORREF(0), 255, LWA_ALPHA);
            } else {
                let _ = SetWindowLongW(h, GWL_EXSTYLE, (ex & !(WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0)) as i32);
            }
        }
    }

    /// watch what's in front while the little picture is up
    fn guard(hwnd: isize) {
        if GUARD.swap(true, std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        std::thread::spawn(move || {
            let h = HWND(hwnd as *mut _);
            while super::is_on() {
                pass_through(h, fullscreen_other_in_front(h));
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            pass_through(h, false);
            GUARD.store(false, std::sync::atomic::Ordering::Relaxed);
        });
    }

    /// is the mouse passing straight through right now (a game in front)?
    pub fn passing() -> bool {
        PASS.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set(hwnd: isize, on: bool) {
        let h = HWND(hwnd as *mut _);
        if on {
            guard(hwnd);
        }
        unsafe {
            if !HOOKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                let _ = windows::Win32::UI::Shell::SetWindowSubclass(h, Some(sub), 0xC0C4, 0);
            }
            if on {
                let mut r = RECT::default();
                let _ = GetWindowRect(h, &mut r);
                let max = IsZoomed(h).as_bool();
                if max {
                    // restored size to come back to; the glide starts from the full screen
                    let _ = ShowWindow(h, SW_RESTORE);
                    let mut back = RECT::default();
                    let _ = GetWindowRect(h, &mut back);
                    *RESTORE.lock() = Some((back, true));
                } else {
                    *RESTORE.lock() = Some((r, false));
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
                let (w, hgt) = ((480. * scale) as i32, (480. / super::aspect() * scale) as i32);
                let m = (24. * scale) as i32;
                // it shrinks into the corner instead of jumping there
                let _ = SetWindowPos(h, Some(HWND_TOPMOST), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_SHOWWINDOW | SWP_NOACTIVATE);
                glide(hwnd, r, RECT { left: wa.right - w - m, top: wa.bottom - hgt - m, right: wa.right - m, bottom: wa.bottom - m }, false);
            } else if let Some((r, max)) = RESTORE.lock().take() {
                let mut from = RECT::default();
                let _ = GetWindowRect(h, &mut from);
                let _ = SetWindowPos(h, Some(HWND_NOTOPMOST), from.left, from.top, from.right - from.left, from.bottom - from.top, SWP_SHOWWINDOW);
                // and grows back out of it
                glide(hwnd, from, r, max);
            }
        }
    }
}

/// Is the pointer over couch's window? (PiP shows its controls only then)
pub fn cursor_inside() -> bool {
    // a game in front: the pointer belongs to it
    #[cfg(target_os = "windows")]
    if imp::passing() {
        return false;
    }
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::Foundation::{HWND, POINT, RECT};
        use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect};
        let h = crate::mini::main_hwnd();
        if h == 0 {
            return true;
        }
        let (mut p, mut r) = (POINT::default(), RECT::default());
        if GetCursorPos(&mut p).is_err() || GetWindowRect(HWND(h as *mut _), &mut r).is_err() {
            return true;
        }
        return p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom;
    }
    #[allow(unreachable_code)]
    true
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
                for (k, ms) in [120u64, 200, 380, 750].into_iter().enumerate() {
                    crate::timer::after(std::time::Duration::from_millis(ms)).await;
                    let _ = cx.update(|cx| {
                        // once it has settled: a 1px wiggle makes everything (couch's
                        // drawing, mpv's window) take the new size, like a focus change did
                        if k == 2 {
                            imp::wiggle(h);
                        }
                        // and what a click on another window did: couch draws itself afresh
                        if k >= 1 {
                            imp::kick(h);
                        }
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
