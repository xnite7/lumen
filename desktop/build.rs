// Bake everything under assets/ into the binary (see src/assets.rs).
use std::fmt::Write;
fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            walk(&p, base, out);
        } else {
            out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
        }
    }
}
fn main() {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets");
    let mut files = vec![];
    walk(&root, &root, &mut files);
    let mut s = String::from("[\n");
    for f in &files {
        let abs = root.join(f).to_string_lossy().replace('\\', "/");
        writeln!(s, "    (\"{f}\", include_bytes!(\"{abs}\") as &[u8]),").unwrap();
    }
    s.push(']');
    std::fs::write(std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("assets.rs"), s).unwrap();
    println!("cargo:rerun-if-changed=assets");
    // libmpv on Windows lives next to the engine (src-tauri/mpv)
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let dir = std::env::var("MPV_LIB_DIR")
            .unwrap_or_else(|_| format!("{}/../src-tauri/mpv", std::env::var("CARGO_MANIFEST_DIR").unwrap()));
        println!("cargo:rustc-link-search=native={dir}");
        println!("cargo:rerun-if-env-changed=MPV_LIB_DIR");
    }
}
