//! A game controller (Xbox, or anything Windows sees as one) as a remote:
//! d-pad / left stick to move, A to pick, B to go back, Y for info, Start to
//! play / pause (and, outside TV mode, to switch it on), the bumpers to skip.
//! Polled about 30 times a second; held directions repeat like a remote.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Pad {
    /// buttons down last time (bit flags, as XInput gives them)
    last: u16,
    /// the direction being held, since when, and when it last repeated
    held: Option<(&'static str, Instant, Instant)>,
    /// a controller was there last time (when not, look less often)
    there: bool,
    next_look: Option<Instant>,
}

const FIRST: Duration = Duration::from_millis(380);
const AGAIN: Duration = Duration::from_millis(110);

impl Pad {
    /// what was pressed since last time, as key names
    pub fn poll(&mut self) -> Vec<&'static str> {
        let Some((buttons, lx, ly)) = self.read() else { return vec![] };
        let mut out = vec![];
        let now = Instant::now();
        // the stick counts as the d-pad
        let dir = |b: u16| -> Option<&'static str> {
            let dead = 16000i32;
            if b & 0x0001 != 0 || ly as i32 > dead {
                Some("up")
            } else if b & 0x0002 != 0 || (ly as i32) < -dead {
                Some("down")
            } else if b & 0x0004 != 0 || (lx as i32) < -dead {
                Some("left")
            } else if b & 0x0008 != 0 || lx as i32 > dead {
                Some("right")
            } else {
                None
            }
        };
        match (dir(buttons), self.held) {
            (Some(d), Some((h, since, last))) if d == h => {
                if now - since >= FIRST && now - last >= AGAIN {
                    out.push(d);
                    self.held = Some((d, since, now));
                }
            }
            (Some(d), _) => {
                out.push(d);
                self.held = Some((d, now, now));
            }
            (None, _) => self.held = None,
        }
        let pressed = buttons & !self.last;
        self.last = buttons;
        for (bit, key) in [(0x1000u16, "enter"), (0x2000, "escape"), (0x8000, "info"), (0x4000, "play"), (0x0010, "play"), (0x0020, "escape"), (0x0100, "prev"), (0x0200, "next")] {
            if pressed & bit != 0 && !out.contains(&key) {
                out.push(key);
            }
        }
        out
    }

    #[cfg(target_os = "windows")]
    fn read(&mut self) -> Option<(u16, i16, i16)> {
        use windows::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_STATE};
        // nobody plugged in: look once every two seconds
        if !self.there {
            if let Some(t) = self.next_look {
                if Instant::now() < t {
                    return None;
                }
            }
            self.next_look = Some(Instant::now() + Duration::from_secs(2));
        }
        for i in 0..4u32 {
            let mut st = XINPUT_STATE::default();
            if unsafe { XInputGetState(i, &mut st) } == 0 {
                self.there = true;
                let g = st.Gamepad;
                return Some((g.wButtons.0, g.sThumbLX, g.sThumbLY));
            }
        }
        self.there = false;
        None
    }

    #[cfg(not(target_os = "windows"))]
    fn read(&mut self) -> Option<(u16, i16, i16)> {
        None
    }
}
