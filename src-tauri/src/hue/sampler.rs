//! Reads the colours of the picture for ambilight.
//!
//! On Windows this asks the desktop compositor to StretchBlt the window's
//! client area into a 64x36 bitmap -- the downscale happens in the GDI/DWM
//! path, so a 4K frame costs the same as a postage stamp. HDR/protected
//! surfaces still composite, so this sees what you see.

use crate::core::CoreRef;

pub const W: usize = 64;
pub const H: usize = 36;

pub struct Frame {
    /// RGB, 0..1, row-major.
    px: Vec<[f32; 3]>,
}

impl Frame {
    /// Colour for a light at Hue position (x: -1 left .. 1 right,
    /// z: -1 floor .. 1 ceiling).
    pub fn region_color(&self, x: f32, z: f32, sat_boost: f32) -> [f32; 3] {
        let cx = ((x.clamp(-1.0, 1.0) + 1.0) / 2.0 * (W as f32 - 1.0)) as i32;
        let cy = ((1.0 - (z.clamp(-1.0, 1.0) + 1.0) / 2.0) * (H as f32 - 1.0)) as i32;
        let (hw, hh) = ((W / 5) as i32, (H / 3) as i32);
        let mut acc = [0f32; 3];
        let mut wsum = 0f32;
        for yy in (cy - hh).max(0)..=(cy + hh).min(H as i32 - 1) {
            for xx in (cx - hw).max(0)..=(cx + hw).min(W as i32 - 1) {
                let p = self.px[yy as usize * W + xx as usize];
                let max = p[0].max(p[1]).max(p[2]);
                let min = p[0].min(p[1]).min(p[2]);
                // Saturated pixels say more about a scene than grey ones.
                let w = 0.15 + (max - min) * 2.0 + max * 0.3;
                for k in 0..3 {
                    acc[k] += p[k] * w;
                }
                wsum += w;
            }
        }
        if wsum <= 0.0 {
            return [0.0; 3];
        }
        let mut c = [acc[0] / wsum, acc[1] / wsum, acc[2] / wsum];
        let l = (c[0] + c[1] + c[2]) / 3.0;
        for v in c.iter_mut() {
            *v = (l + (*v - l) * sat_boost).clamp(0.0, 1.0);
        }
        c
    }
}

pub struct Sampler {
    #[allow(dead_code)]
    core: CoreRef,
    #[cfg(target_os = "windows")]
    hwnd: Option<isize>,
}

impl Sampler {
    pub fn new(core: CoreRef) -> Self {
        #[cfg(target_os = "windows")]
        {
            use tauri::Manager;
            let hwnd = core.app.get_webview_window("main").and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize);
            return Self { core, hwnd };
        }
        #[cfg(not(target_os = "windows"))]
        Self { core }
    }

    pub fn grab(&mut self) -> Option<Frame> {
        #[cfg(target_os = "windows")]
        {
            return win::grab(self.hwnd?);
        }
        #[cfg(not(target_os = "windows"))]
        None
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::{Frame, H, W};
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    pub fn grab(hwnd: isize) -> Option<Frame> {
        unsafe {
            let h = HWND(hwnd as *mut _);
            let mut rc = RECT::default();
            GetClientRect(h, &mut rc).ok()?;
            let mut origin = POINT { x: 0, y: 0 };
            let _ = ClientToScreen(h, &mut origin);
            let (cw, ch) = (rc.right - rc.left, rc.bottom - rc.top);
            if cw <= 0 || ch <= 0 {
                return None;
            }
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: W as i32,
                    biHeight: -(H as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(Some(mem), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            let old = SelectObject(mem, dib.into());
            SetStretchBltMode(mem, HALFTONE);
            let ok = StretchBlt(mem, 0, 0, W as i32, H as i32, Some(screen), origin.x, origin.y, cw, ch, SRCCOPY).as_bool();
            let mut px = Vec::with_capacity(W * H);
            if ok && !bits.is_null() {
                let raw = std::slice::from_raw_parts(bits as *const u8, W * H * 4);
                for i in 0..W * H {
                    px.push([raw[i * 4 + 2] as f32 / 255.0, raw[i * 4 + 1] as f32 / 255.0, raw[i * 4] as f32 / 255.0]);
                }
            }
            SelectObject(mem, old);
            let _ = DeleteObject(dib.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            if px.len() == W * H {
                Some(Frame { px })
            } else {
                None
            }
        }
    }
}
