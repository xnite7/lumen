//! The mini player: going back from the player keeps the film playing in a
//! corner of couch while you browse. mpv draws into a little host window of
//! our own; full size it sits under couch's UI, mini it moves into a small
//! rounded popup over the bottom-right corner.
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

static VIDEO: AtomicIsize = AtomicIsize::new(0);
/// the GPU clip player's window (billboard, previews, trailers)
static CLIP: AtomicIsize = AtomicIsize::new(0);
static MAIN: AtomicIsize = AtomicIsize::new(0);
static ON: AtomicBool = AtomicBool::new(false);
/// the popup was clicked: back to full size
pub static CLICKED: AtomicBool = AtomicBool::new(false);

/// logical size and margin of the mini picture
pub const W: f32 = 400.;
pub const H: f32 = 225.;
pub const M: f32 = 24.;

pub fn is_on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// mpv's parent window (the host), or 0 before it exists
pub fn video_hwnd() -> isize {
    VIDEO.load(Ordering::Relaxed)
}

/// where the clip window goes (client pixels) and what part of it shows
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ClipPlace {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// the visible part, relative to the window
    pub vis: (i32, i32, i32, i32),
    pub r: i32,
}

/// the UI clip player's window, or 0
pub fn clip_hwnd() -> isize {
    CLIP.load(Ordering::Relaxed)
}

/// couch's own window
pub fn main_hwnd() -> isize {
    MAIN.load(Ordering::Relaxed)
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{ClientToScreen, CreateSolidBrush, HBRUSH};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe extern "system" fn popup_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_LBUTTONUP => {
                CLICKED.store(true, Ordering::Relaxed);
                LRESULT(0)
            }
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            _ => unsafe { DefWindowProcW(h, msg, wp, lp) },
        }
    }

    unsafe extern "system" fn host_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(h, msg, wp, lp) }
    }

    static POPUP: AtomicIsize = AtomicIsize::new(0);

    fn hw(v: isize) -> HWND {
        HWND(v as *mut _)
    }

    /// make the host window inside couch's window (once)
    pub fn init(main: isize) -> isize {
        if VIDEO.load(Ordering::Relaxed) != 0 {
            return VIDEO.load(Ordering::Relaxed);
        }
        MAIN.store(main, Ordering::Relaxed);
        unsafe {
            let inst = GetModuleHandleW(None).unwrap_or_default();
            let black: HBRUSH = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0));
            let wc = WNDCLASSW { lpfnWndProc: Some(host_proc), hInstance: inst.into(), lpszClassName: w!("couch-video"), hbrBackground: black, ..Default::default() };
            RegisterClassW(&wc);
            let pc = WNDCLASSW { lpfnWndProc: Some(popup_proc), hInstance: inst.into(), lpszClassName: w!("couch-mini"), hbrBackground: black, hCursor: LoadCursorW(None, IDC_HAND).unwrap_or_default(), ..Default::default() };
            RegisterClassW(&pc);
            let mut r = RECT::default();
            let _ = GetClientRect(hw(main), &mut r);
            let v = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("couch-video"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_DISABLED | WS_CLIPCHILDREN,
                0,
                0,
                r.right.max(1),
                r.bottom.max(1),
                Some(hw(main)),
                None,
                Some(inst.into()),
                None,
            );
            let Ok(v) = v else { return 0 };
            let _ = SetWindowPos(v, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            VIDEO.store(v.0 as isize, Ordering::Relaxed);
            // the clip window: hidden until a spot asks for it
            if let Ok(c) = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("couch-video"),
                w!(""),
                WS_CHILD | WS_DISABLED | WS_CLIPCHILDREN,
                0,
                0,
                16,
                16,
                Some(hw(main)),
                None,
                Some(inst.into()),
                None,
            ) {
                CLIP.store(c.0 as isize, Ordering::Relaxed);
            }
            let p = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("couch-mini"),
                w!("couch"),
                WS_POPUP | WS_CLIPCHILDREN,
                0,
                0,
                400,
                225,
                Some(hw(main)),
                None,
                Some(inst.into()),
                None,
            );
            if let Ok(p) = p {
                POPUP.store(p.0 as isize, Ordering::Relaxed);
                // rounded corners (Windows 11)
                let pref: u32 = 2; // DWMWCP_ROUND
                let _ = windows::Win32::Graphics::Dwm::DwmSetWindowAttribute(p, windows::Win32::Graphics::Dwm::DWMWA_WINDOW_CORNER_PREFERENCE, &pref as *const u32 as *const _, 4);
            }
            v.0 as isize
        }
    }

    static LAST: parking_lot::Mutex<Option<ClipPlace>> = parking_lot::Mutex::new(None);

    /// Put the clip window under its see-through spot (client pixels), or hide it.
    pub fn place_clip(at: Option<ClipPlace>) {
        let c = CLIP.load(Ordering::Relaxed);
        if c == 0 {
            return;
        }
        let mut last = LAST.lock();
        if *last == at {
            return;
        }
        let prev = *last;
        *last = at;
        unsafe {
            match at {
                None => {
                    let _ = ShowWindow(hw(c), SW_HIDE);
                }
                Some(p) => {
                    use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn, RGN_AND};
                    let _ = SetWindowPos(hw(c), Some(HWND_TOP), p.x, p.y, p.w.max(1), p.h.max(1), SWP_NOACTIVATE | SWP_SHOWWINDOW);
                    // rounded corners, cut to the part that's on screen
                    let shape = if p.r > 0 { CreateRoundRectRgn(0, 0, p.w + 1, p.h + 1, p.r * 2, p.r * 2) } else { CreateRectRgn(0, 0, p.w, p.h) };
                    let (vx, vy, vw, vh) = p.vis;
                    let vis = CreateRectRgn(vx, vy, vx + vw, vy + vh);
                    let _ = CombineRgn(Some(shape), Some(shape), Some(vis), RGN_AND);
                    let _ = DeleteObject(vis.into());
                    // the window owns the region from here
                    let _ = SetWindowRgn(hw(c), Some(shape), true);
                }
            }
        }
        let moved = at.is_some() && prev.map_or(true, |q| (q.w, q.h) != (at.unwrap().w, at.unwrap().h));
        drop(last);
        // mpv's own window inside follows when the size changes
        if moved {
            couch::player::refit_video(Some(c));
        }
    }

    /// keep the host the size of couch's window (full size), or the popup (mini)
    pub fn fit(scale: f32) {
        let (v, main, pop) = (VIDEO.load(Ordering::Relaxed), MAIN.load(Ordering::Relaxed), POPUP.load(Ordering::Relaxed));
        if v == 0 {
            return;
        }
        unsafe {
            let mut r = RECT::default();
            let _ = GetClientRect(hw(main), &mut r);
            if ON.load(Ordering::Relaxed) && pop != 0 {
                let (w, h, m) = ((W * scale) as i32, (H * scale) as i32, (M * scale) as i32);
                let mut pt = POINT { x: r.right - w - m, y: r.bottom - h - m };
                let _ = ClientToScreen(hw(main), &mut pt);
                let _ = SetWindowPos(hw(pop), None, pt.x, pt.y, w, h, SWP_NOACTIVATE | SWP_NOZORDER);
                let _ = SetWindowPos(hw(v), None, 0, 0, w, h, SWP_NOACTIVATE | SWP_NOZORDER);
            } else {
                let _ = SetWindowPos(hw(v), Some(HWND_BOTTOM), 0, 0, r.right.max(1), r.bottom.max(1), SWP_NOACTIVATE);
            }
        }
        couch::player::refit_video(Some(v));
    }

    pub fn set(on: bool, scale: f32) {
        let (v, main, pop) = (VIDEO.load(Ordering::Relaxed), MAIN.load(Ordering::Relaxed), POPUP.load(Ordering::Relaxed));
        if v == 0 || pop == 0 || ON.swap(on, Ordering::Relaxed) == on {
            return;
        }
        unsafe {
            if on {
                let _ = SetParent(hw(v), Some(hw(pop)));
                fit(scale);
                let _ = ShowWindow(hw(pop), SW_SHOWNOACTIVATE);
            } else {
                let _ = ShowWindow(hw(pop), SW_HIDE);
                let _ = SetParent(hw(v), Some(hw(main)));
                fit(scale);
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub use imp::{fit, init, place_clip, set};

#[cfg(not(target_os = "windows"))]
pub fn init(main: isize) -> isize {
    MAIN.store(main, Ordering::Relaxed);
    0
}
#[cfg(not(target_os = "windows"))]
pub fn fit(_scale: f32) {}
#[cfg(not(target_os = "windows"))]
pub fn place_clip(_at: Option<ClipPlace>) {}
#[cfg(not(target_os = "windows"))]
pub fn set(on: bool, _scale: f32) {
    ON.store(on, Ordering::Relaxed);
}
