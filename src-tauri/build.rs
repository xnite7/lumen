fn main() {
    // libmpv on Windows: CI (or you) puts mpv.lib + libmpv-2.dll in
    // src-tauri/mpv/ -- see scripts/fetch-mpv.ps1.
    let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target == "windows" {
        let dir = std::env::var("MPV_LIB_DIR").unwrap_or_else(|_| {
            format!("{}/mpv", std::env::var("CARGO_MANIFEST_DIR").unwrap())
        });
        println!("cargo:rustc-link-search=native={dir}");
        println!("cargo:rerun-if-env-changed=MPV_LIB_DIR");
    }
    tauri_build::build()
}
