use crate::addons::Addons;
use crate::core::CoreRef;
use crate::hue::Hue;
use crate::player::Player;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub struct AppState {
    pub core: CoreRef,
    pub addons: Arc<Addons>,
    pub player: Arc<Player>,
    pub hue: Arc<Hue>,
    #[cfg(desktop)]
    pub tobii: Arc<crate::tobii::Tobii>,
}

pub fn get(app: &AppHandle) -> tauri::State<'_, AppState> {
    app.state::<AppState>()
}

pub fn addons(app: &AppHandle) -> Arc<Addons> {
    get(app).addons.clone()
}

pub fn player(app: &AppHandle) -> Arc<Player> {
    get(app).player.clone()
}

pub fn hue(app: &AppHandle) -> Arc<Hue> {
    get(app).hue.clone()
}
