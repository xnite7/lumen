//! Native iPhone features. Every command is implemented in Swift
//! (ios/Sources) and called from the page as `plugin:lumen-native|<cmd>`;
//! on other platforms the plugin registers nothing and the page's wrappers
//! (src/lib/native.ts) don't call it.

use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_lumen_native);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("lumen-native")
        .setup(|_app, _api| {
            #[cfg(target_os = "ios")]
            _api.register_ios_plugin(init_plugin_lumen_native)?;
            Ok(())
        })
        .build()
}
