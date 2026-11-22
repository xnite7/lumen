use crate::addons::Addons;
use crate::core::CoreRef;
use crate::hue::Hue;
use crate::player::Player;
use once_cell::sync::OnceCell;
use std::sync::Arc;

/// Everything the app runs on, set once at startup and reachable from
/// anywhere (background tasks, both UIs).
#[derive(Clone)]
pub struct AppState {
    pub core: CoreRef,
    pub addons: Arc<Addons>,
    pub player: Arc<Player>,
    pub hue: Arc<Hue>,
    #[cfg(desktop)]
    pub tobii: Arc<crate::tobii::Tobii>,
}

static STATE: OnceCell<AppState> = OnceCell::new();

pub fn set(s: AppState) {
    let _ = STATE.set(s);
}

pub fn get() -> &'static AppState {
    STATE.get().expect("couch state not ready")
}

pub fn try_get() -> Option<&'static AppState> {
    STATE.get()
}

pub fn addons() -> Arc<Addons> {
    get().addons.clone()
}

pub fn player() -> Arc<Player> {
    get().player.clone()
}

pub fn hue() -> Arc<Hue> {
    get().hue.clone()
}
