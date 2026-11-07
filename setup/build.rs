// The couch installer (NSIS, built first) rides inside this exe. Set
// COUCH_PAYLOAD to its path; without it the setup builds with nothing to
// install (fine for checking the code).
fn main() {
    println!("cargo:rerun-if-env-changed=COUCH_PAYLOAD");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("payload.exe");
    match std::env::var("COUCH_PAYLOAD") {
        Ok(p) if !p.is_empty() => {
            println!("cargo:rerun-if-changed={p}");
            std::fs::copy(&p, &out).expect("COUCH_PAYLOAD not found");
        }
        _ => std::fs::write(&out, b"").unwrap(),
    }
    tauri_build::build()
}
