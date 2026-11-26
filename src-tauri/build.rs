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
    // the Mac's film view (mpv's render API in an OpenGL layer under the UI)
    if target == "macos" {
        let mut b = cc::Build::new();
        b.file("mac/video.m").file("mac/system.m").flag("-fobjc-arc").flag("-Wno-deprecated-declarations");
        // mpv's headers (Homebrew's, or MPV_INCLUDE_DIR)
        for d in [std::env::var("MPV_INCLUDE_DIR").ok(), Some("/opt/homebrew/include".into()), Some("/usr/local/include".into())].into_iter().flatten() {
            b.include(d);
        }
        b.compile("couchmacvideo");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=QuartzCore");
        println!("cargo:rustc-link-lib=framework=OpenGL");
        println!("cargo:rustc-link-lib=framework=MediaPlayer");
        println!("cargo:rustc-link-lib=framework=IOKit");
        for d in ["/opt/homebrew/lib", "/usr/local/lib"] {
            println!("cargo:rustc-link-search=native={d}");
        }
        println!("cargo:rerun-if-changed=mac/video.m");
        println!("cargo:rerun-if-changed=mac/system.m");
        println!("cargo:rerun-if-env-changed=MPV_INCLUDE_DIR");
    }
    #[cfg(feature = "tauri-app")]
    tauri_build::build();
    // without Tauri, set the same desktop/mobile switches it would
    #[cfg(not(feature = "tauri-app"))]
    {
        println!("cargo::rustc-check-cfg=cfg(desktop)");
        println!("cargo::rustc-check-cfg=cfg(mobile)");
        if target == "ios" || target == "android" {
            println!("cargo:rustc-cfg=mobile");
        } else {
            println!("cargo:rustc-cfg=desktop");
        }
    }
}
