//! couch on a Mac: the menu bar (with ⌘ shortcuts), ⌘ for the text boxes,
//! and couch: links from the browser (Discord sign-in, shared titles).
#![cfg(target_os = "macos")]
use gpui::{actions, App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType};

actions!(couch_mac, [Quit, Hide, HideOthers, ShowAll, Minimize, Zoom, FullScreen, About, Guide, Search, Home, Settings, Back]);

fn window(cx: &mut App, f: impl FnOnce(&mut gpui::Window) + 'static) {
    if let Some(w) = cx.windows().first().copied() {
        let _ = w.update(cx, |_, window, _| f(window));
    }
}

pub fn init(cx: &mut App) {
    {
        use crate::ui::input::*;
        let c = Some("TextInput");
        cx.bind_keys([
            KeyBinding::new("cmd-a", SelectAll, c),
            KeyBinding::new("cmd-v", Paste, c),
            KeyBinding::new("cmd-c", Copy, c),
            KeyBinding::new("cmd-x", Cut, c),
            KeyBinding::new("cmd-left", Home, c),
            KeyBinding::new("cmd-right", End, c),
        ]);
    }
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("ctrl-cmd-f", FullScreen, None),
        KeyBinding::new("cmd-f", Search, None),
        KeyBinding::new("cmd-k", Search, None),
        KeyBinding::new("cmd-,", Settings, None),
        KeyBinding::new("cmd-[", Back, None),
        KeyBinding::new("shift-cmd-h", Home, None),
    ]);
    cx.on_action(|_: &Quit, cx| {
        // what's playing is saved first (where you got to)
        let player = couch::state::get().player.clone();
        let _ = couch::rt::block_on(async move { player.stop().await });
        cx.quit()
    });
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &Minimize, cx| window(cx, |w| w.minimize_window()));
    cx.on_action(|_: &Zoom, cx| window(cx, |w| w.zoom_window()));
    cx.on_action(|_: &FullScreen, cx| window(cx, |w| w.toggle_fullscreen()));
    cx.on_action(|_: &About, cx| cx.open_url("https://midnite.club"));
    cx.on_action(|_: &Guide, cx| cx.open_url("https://midnite.club/install"));
    cx.on_action(|_: &Search, cx| crate::Nav::go(crate::Route::Search, cx));
    cx.on_action(|_: &Home, cx| crate::Nav::go(crate::Route::Home, cx));
    cx.on_action(|_: &Settings, cx| crate::Nav::go(crate::Route::Settings, cx));
    cx.on_action(|_: &Back, cx| crate::Nav::back(cx));
    cx.set_menus([
        Menu::new("couch").items([
            MenuItem::action("About midnite couch", About),
            MenuItem::separator(),
            MenuItem::action("Settings…", Settings),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Hide couch", Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action("Quit couch", Quit),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Cut", crate::ui::input::Cut, OsAction::Cut),
            MenuItem::os_action("Copy", crate::ui::input::Copy, OsAction::Copy),
            MenuItem::os_action("Paste", crate::ui::input::Paste, OsAction::Paste),
            MenuItem::os_action("Select All", crate::ui::input::SelectAll, OsAction::SelectAll),
        ]),
        Menu::new("Go").items([
            MenuItem::action("Home", Home),
            MenuItem::action("Search", Search),
            MenuItem::action("Back", Back),
        ]),
        Menu::new("View").items([MenuItem::action("Enter Full Screen", FullScreen)]),
        Menu::new("Window").items([MenuItem::action("Minimize", Minimize), MenuItem::action("Zoom", Zoom)]),
        Menu::new("Help").items([MenuItem::action("How to install couch", Guide)]),
    ]);
}
