fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    build_windows_icon();
}

#[cfg(target_os = "windows")]
fn build_windows_icon() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    if let Err(e) = res.compile() {
        eprintln!("cargo:warning=failed to embed Windows icon: {e}");
    }
}

#[cfg(not(target_os = "windows"))]
fn build_windows_icon() {}