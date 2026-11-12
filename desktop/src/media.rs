//! couch and the rest of Windows while something plays: the media keys and
//! the volume overlay's play/pause/next (Windows' media controls, with the
//! title and its picture), and buttons under couch's taskbar preview
//! (back 10 s, play/pause, next episode). Everything goes through the same
//! rules as couch's own buttons (on someone's couch, the host decides).
use crate::playback;
use gpui::App;
use serde_json::json;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug)]
pub enum Press {
    PlayPause,
    Play,
    Pause,
    Next,
    Back10,
    Stop,
}

static TX: OnceLock<std::sync::Mutex<Sender<Press>>> = OnceLock::new();

fn send(p: Press) {
    if let Some(tx) = TX.get() {
        let _ = tx.lock().map(|t| t.send(p));
    }
}

/// what was last shown to Windows, so it's only told about changes
#[derive(Default, PartialEq, Clone)]
struct Shown {
    open: bool,
    paused: bool,
    title: String,
    sub: String,
    art: Option<String>,
}

pub fn start(cx: &mut App) {
    let (tx, rx): (Sender<Press>, Receiver<Press>) = channel();
    let _ = TX.set(std::sync::Mutex::new(tx));
    // presses (from Windows' threads) land here and become couch's own actions
    cx.spawn(async move |cx| loop {
        crate::timer::after(std::time::Duration::from_millis(120)).await;
        while let Ok(p) = rx.try_recv() {
            let _ = cx.update(|cx| act(p, cx));
        }
    })
    .detach();
    #[cfg(target_os = "windows")]
    {
        // a moment after the window exists (the taskbar button has to be there first)
        cx.spawn(async move |cx| {
            crate::timer::after(std::time::Duration::from_secs(3)).await;
            let _ = cx.update(|cx| cx.defer(|_| imp::init(crate::mini::main_hwnd())));
            let mut last = Shown::default();
            loop {
                crate::timer::after(std::time::Duration::from_millis(400)).await;
                let now = cx.update(|cx| {
                    let p = playback::get(cx).read(cx);
                    let m = p.meta.clone();
                    let parts: Vec<&str> = p.video_id.split(':').collect();
                    let sub = if parts.len() == 3 {
                        let ep = if p.episode_title.is_empty() { String::new() } else { format!(" · {}", p.episode_title) };
                        format!("S{} E{}{ep}", parts[1], parts[2])
                    } else {
                        "midnite couch".into()
                    };
                    Shown { open: p.open, paused: p.state.paused, title: m.as_ref().map(|m| m.name.clone()).unwrap_or_default(), sub, art: m.and_then(|m| m.background.or(m.poster)) }
                });
                if now != last {
                    let n = now.clone();
                    let _ = cx.update(|cx| cx.defer(move |_| imp::show(&n)));
                    last = now;
                }
            }
        })
        .detach();
    }
}

fn act(p: Press, cx: &mut App) {
    let pb = playback::get(cx);
    if !pb.read(cx).open {
        return;
    }
    let (paused, pos) = (pb.read(cx).state.paused, pb.read(cx).state.pos);
    match p {
        Press::PlayPause | Press::Play | Press::Pause => {
            let want_pause = match p {
                Press::Play => false,
                Press::Pause => true,
                _ => !paused,
            };
            if want_pause == paused {
                return;
            }
            if crate::views::player::may(if want_pause { "pause" } else { "play" }, None, cx) {
                pb.read(cx).cmd("pause", json!({ "value": want_pause }));
            }
        }
        Press::Back10 => {
            if crate::views::player::may("seek", Some((pos - 10.).max(0.)), cx) {
                pb.read(cx).cmd("seek", json!({ "to": -10, "relative": true }));
            }
        }
        Press::Next => {
            if pb.read(cx).next_video().is_some() && crate::views::player::may("episode", None, cx) {
                pb.update(cx, |p, cx| p.play_next(cx));
            }
        }
        Press::Stop => {
            if crate::views::player::may("pause", None, cx) {
                pb.read(cx).cmd("pause", json!({ "value": true }));
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{send, Press, Shown};
    use parking_lot::Mutex;
    use windows::core::HSTRING;
    use windows::Foundation::{TypedEventHandler, Uri};
    use windows::Media::{MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls, SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs};
    use windows::Storage::Streams::RandomAccessStreamReference;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
    use windows::Win32::UI::Shell::{DefSubclassProc, ITaskbarList3, SetWindowSubclass, TaskbarList, THBF_ENABLED, THBN_CLICKED, THB_FLAGS, THB_ICON, THB_TOOLTIP, THUMBBUTTON};
    use windows::Win32::UI::WindowsAndMessaging::{CreateIcon, HICON, WM_COMMAND};

    struct Win {
        smtc: Option<SystemMediaTransportControls>,
        bar: Option<ITaskbarList3>,
        hwnd: isize,
        icons: Vec<HICON>,
    }
    // (only touched on couch's main thread, through cx.defer)
    unsafe impl Send for Win {}
    static WIN: Mutex<Option<Win>> = Mutex::new(None);

    const B_BACK: u32 = 41;
    const B_PLAY: u32 = 42;
    const B_NEXT: u32 = 43;

    unsafe extern "system" fn sub(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
        if msg == WM_COMMAND && ((wp.0 >> 16) & 0xffff) as u32 == THBN_CLICKED {
            match (wp.0 & 0xffff) as u32 {
                B_BACK => send(Press::Back10),
                B_PLAY => send(Press::PlayPause),
                B_NEXT => send(Press::Next),
                _ => {}
            }
            return LRESULT(0);
        }
        unsafe { DefSubclassProc(h, msg, wp, lp) }
    }

    /// a 32×32 icon drawn here: white shapes, soft edges
    fn icon(shape: &dyn Fn(f32, f32) -> bool) -> HICON {
        const N: usize = 32;
        const S: usize = 4; // supersampling
        let mut bgra = vec![0u8; N * N * 4];
        for y in 0..N {
            for x in 0..N {
                let mut hit = 0;
                for sy in 0..S {
                    for sx in 0..S {
                        let fx = (x as f32 + (sx as f32 + 0.5) / S as f32) / N as f32;
                        let fy = (y as f32 + (sy as f32 + 0.5) / S as f32) / N as f32;
                        if shape(fx, fy) {
                            hit += 1;
                        }
                    }
                }
                let a = (hit * 255 / (S * S)) as u8;
                let i = (y * N + x) * 4;
                // premultiplied white
                bgra[i] = a;
                bgra[i + 1] = a;
                bgra[i + 2] = a;
                bgra[i + 3] = a;
            }
        }
        let mask = vec![0u8; N * N / 8];
        unsafe { CreateIcon(None, N as i32, N as i32, 1, 32, mask.as_ptr(), bgra.as_ptr()).unwrap_or_default() }
    }

    fn tri(fx: f32, fy: f32, x0: f32, x1: f32, top: f32, bot: f32, right: bool) -> bool {
        // a triangle pointing right (or left) between x0..x1
        if fx < x0 || fx > x1 || fy < top || fy > bot {
            return false;
        }
        let mid = (top + bot) / 2.;
        let half = (bot - top) / 2.;
        let t = if right { (fx - x0) / (x1 - x0) } else { (x1 - fx) / (x1 - x0) };
        (fy - mid).abs() <= half * (1. - t)
    }

    fn icons() -> Vec<HICON> {
        let play = icon(&|x, y| tri(x, y, 0.32, 0.78, 0.22, 0.78, true));
        let pause = icon(&|x, y| (y > 0.24 && y < 0.76) && ((x > 0.3 && x < 0.43) || (x > 0.57 && x < 0.7)));
        let back = icon(&|x, y| tri(x, y, 0.16, 0.5, 0.26, 0.74, false) || tri(x, y, 0.48, 0.82, 0.26, 0.74, false));
        let next = icon(&|x, y| tri(x, y, 0.2, 0.62, 0.24, 0.76, true) || (x > 0.64 && x < 0.75 && y > 0.24 && y < 0.76));
        vec![play, pause, back, next]
    }

    fn tip(s: &str) -> [u16; 260] {
        let mut t = [0u16; 260];
        for (i, c) in s.encode_utf16().take(259).enumerate() {
            t[i] = c;
        }
        t
    }

    fn buttons(icons: &[HICON], paused: bool, on: bool) -> [THUMBBUTTON; 3] {
        let flags = if on { THBF_ENABLED } else { windows::Win32::UI::Shell::THBF_DISABLED };
        let b = |id: u32, ic: HICON, t: &str| THUMBBUTTON { dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS, iId: id, hIcon: ic, szTip: tip(t), dwFlags: flags, ..Default::default() };
        [
            b(B_BACK, icons[2], "back 10 seconds"),
            if paused { b(B_PLAY, icons[0], "play") } else { b(B_PLAY, icons[1], "pause") },
            b(B_NEXT, icons[3], "next episode"),
        ]
    }

    pub fn init(hwnd: isize) {
        if hwnd == 0 || WIN.lock().is_some() {
            return;
        }
        let h = HWND(hwnd as *mut _);
        let icons = icons();
        // the taskbar preview's buttons
        let bar: Option<ITaskbarList3> = unsafe {
            CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok().filter(|b: &ITaskbarList3| b.HrInit().is_ok())
        };
        if let Some(b) = &bar {
            unsafe {
                let _ = SetWindowSubclass(h, Some(sub), 0xC0C5, 0);
                let _ = b.ThumbBarAddButtons(h, &buttons(&icons, true, false));
            }
        }
        // Windows' media controls (keys, the volume overlay, the lock screen)
        let smtc = (|| -> windows::core::Result<SystemMediaTransportControls> {
            let interop = windows::core::factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
            let s: SystemMediaTransportControls = unsafe { interop.GetForWindow(h)? };
            s.SetIsPlayEnabled(true)?;
            s.SetIsPauseEnabled(true)?;
            s.SetIsNextEnabled(true)?;
            s.SetIsStopEnabled(true)?;
            s.SetIsEnabled(false)?;
            s.ButtonPressed(&TypedEventHandler::<SystemMediaTransportControls, SystemMediaTransportControlsButtonPressedEventArgs>::new(|_, a| {
                if let Some(a) = a.as_ref() {
                    match a.Button()? {
                        SystemMediaTransportControlsButton::Play => send(Press::Play),
                        SystemMediaTransportControlsButton::Pause => send(Press::Pause),
                        SystemMediaTransportControlsButton::Next => send(Press::Next),
                        SystemMediaTransportControlsButton::Stop => send(Press::Stop),
                        _ => {}
                    }
                }
                Ok(())
            }))?;
            Ok(s)
        })()
        .ok();
        *WIN.lock() = Some(Win { smtc, bar, hwnd, icons });
    }

    pub fn show(n: &Shown) {
        let g = WIN.lock();
        let Some(w) = g.as_ref() else { return };
        if let Some(b) = &w.bar {
            unsafe {
                let _ = b.ThumbBarUpdateButtons(HWND(w.hwnd as *mut _), &buttons(&w.icons, n.paused || !n.open, n.open));
            }
        }
        let Some(s) = &w.smtc else { return };
        let _ = (|| -> windows::core::Result<()> {
            if !n.open {
                s.SetPlaybackStatus(MediaPlaybackStatus::Closed)?;
                s.SetIsEnabled(false)?;
                return Ok(());
            }
            s.SetIsEnabled(true)?;
            s.SetPlaybackStatus(if n.paused { MediaPlaybackStatus::Paused } else { MediaPlaybackStatus::Playing })?;
            let du = s.DisplayUpdater()?;
            du.SetType(MediaPlaybackType::Video)?;
            let vp = du.VideoProperties()?;
            vp.SetTitle(&HSTRING::from(n.title.as_str()))?;
            vp.SetSubtitle(&HSTRING::from(n.sub.as_str()))?;
            if let Some(a) = n.art.as_deref().filter(|a| a.starts_with("http")) {
                let u = crate::api::sized(a, "w780");
                if let Ok(uri) = Uri::CreateUri(&HSTRING::from(u.as_str())) {
                    if let Ok(r) = RandomAccessStreamReference::CreateFromUri(&uri) {
                        let _ = du.SetThumbnail(&r);
                    }
                }
            }
            du.Update()?;
            Ok(())
        })();
    }
}
