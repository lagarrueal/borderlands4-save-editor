//! Embed the application icon (Windows resource compiled with windres/rc).
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let res = out.join("icon_res.o");
    let ok = std::process::Command::new("windres")
        .current_dir("assets")
        .args(["icon.rc", "-O", "coff", "-o"])
        .arg(&res)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        println!("cargo:rustc-link-arg-bins={}", res.display());
    } else {
        println!("cargo:warning=windres not found: building without an exe icon");
    }
}
