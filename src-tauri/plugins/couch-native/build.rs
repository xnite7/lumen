const COMMANDS: &[&str] = &["haptic", "share", "auth", "endauth", "play", "stop", "tabbar", "control", "register_listener", "remove_listener"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).ios_path("ios").build();
}
