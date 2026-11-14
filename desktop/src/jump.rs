//! Taskbar extras: right-click couch's icon for what you're watching and
//! what's on your list (the jump list), and a second couch (from the jump
//! list, a shortcut, a link) hands its page to the one already open.
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

const PORT: u16 = 53692;

/// What a launch asked for: `--open detail:series:tt123` (or nothing: just show couch).
pub fn requested() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == "--open")
        .and_then(|i| args.get(i + 1).cloned())
        // a couch: link (installed couch is started with the link itself)
        .or_else(|| args.iter().skip(1).find(|a| a.starts_with("couch:")).cloned())
        .or_else(|| std::env::var("COUCH_START").ok())
}

/// Wait (up to `max`) until no other couch is answering.
pub fn wait_gone(max: Duration) {
    let until = std::time::Instant::now() + max;
    while std::time::Instant::now() < until {
        if TcpStream::connect_timeout(&([127, 0, 0, 1], PORT).into(), Duration::from_millis(200)).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Already open? Hand it the page and return true (this launch then quits).
pub fn forward(spec: Option<&str>) -> bool {
    let Ok(mut s) = TcpStream::connect_timeout(&([127, 0, 0, 1], PORT).into(), Duration::from_millis(300)) else { return false };
    let _ = writeln!(s, "{}", spec.unwrap_or("show"));
    true
}

/// Listen for later launches; their pages arrive on the returned channel.
pub fn listen() -> Receiver<String> {
    let (tx, rx) = channel();
    if let Ok(l) = TcpListener::bind(("127.0.0.1", PORT)) {
        std::thread::Builder::new()
            .name("couch-launches".into())
            .spawn(move || {
                for s in l.incoming().flatten() {
                    let mut line = String::new();
                    if BufReader::new(s).read_line(&mut line).is_ok() {
                        let _ = tx.send(line.trim().to_string());
                    }
                }
            })
            .ok();
    }
    rx
}

/// One jump list entry: the label and the page it opens.
pub struct Item {
    pub title: String,
    pub open: String,
}

/// Rebuild the jump list: "continue watching", "on your list", and quick tasks.
#[cfg(target_os = "windows")]
pub fn set(groups: Vec<(String, Vec<Item>)>) {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, ShellLink};
    // the installed couch is started through its alias (its own folder can't be run from directly)
    let exe = if crate::packaged::is_packaged() {
        match dirs::data_local_dir() {
            Some(d) => d.join("Microsoft\\WindowsApps\\couch.exe"),
            None => return,
        }
    } else {
        let Ok(exe) = std::env::current_exe() else { return };
        exe
    };
    // COM on a thread of its own (the UI thread's COM setup is GPUI's business)
    let _ = std::thread::spawn(move || unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link = |title: &str, open: &str| -> Option<IShellLinkW> {
            let l: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
            l.SetPath(&HSTRING::from(exe.as_os_str())).ok()?;
            l.SetArguments(&HSTRING::from(format!("--open \"{open}\""))).ok()?;
            l.SetIconLocation(&HSTRING::from(exe.as_os_str()), 0).ok()?;
            l.SetDescription(&HSTRING::from(title)).ok()?;
            let store: IPropertyStore = l.cast().ok()?;
            store.SetValue(&PKEY_Title, &PROPVARIANT::from(title)).ok()?;
            store.Commit().ok()?;
            Some(l)
        };
        let Ok(list) = CoCreateInstance::<_, ICustomDestinationList>(&DestinationList, None, CLSCTX_INPROC_SERVER) else { return };
        let mut slots = 0u32;
        let Ok(_removed) = list.BeginList::<IObjectArray>(&mut slots) else { return };
        let mut room = slots.max(6) as usize;
        for (name, items) in groups {
            let Ok(coll) = CoCreateInstance::<_, IObjectCollection>(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER) else { continue };
            let mut n = 0;
            for it in items.iter().take(room) {
                if let Some(l) = link(&it.title, &it.open) {
                    if coll.AddObject(&l).is_ok() {
                        n += 1;
                    }
                }
            }
            if n == 0 {
                continue;
            }
            room = room.saturating_sub(n);
            if let Ok(arr) = coll.cast::<IObjectArray>() {
                if name.is_empty() {
                    let _ = list.AddUserTasks(&arr);
                } else {
                    let _ = list.AppendCategory(&HSTRING::from(name), &arr);
                }
            }
        }
        let _ = list.CommitList();
    });
}

#[cfg(not(target_os = "windows"))]
pub fn set(_groups: Vec<(String, Vec<Item>)>) {}
