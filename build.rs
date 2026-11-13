fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    build_windows_icon();
    embed_patches();
}

fn embed_patches() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let patches_dir = std::path::PathBuf::from(&manifest_dir).join("patches");

    println!("cargo:rerun-if-changed={}", patches_dir.display());

    let mut file_names: Vec<String> = match std::fs::read_dir(&patches_dir) {
        Ok(entries) => entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("ips") {
                    path.file_name()?.to_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect(),
        Err(e) => panic!(
            "build.rs: could not read patches directory {patches_dir:?}: {e}. \
             Every .ips patch must live at <crate root>/patches/ to be compiled in."
        ),
    };
    file_names.sort();

    for file_name in &file_names {
        println!("cargo:rerun-if-changed={}", patches_dir.join(file_name).display());
    }

    let mut generated = String::new();
    generated.push_str("pub static EMBEDDED_PATCHES: &[(&str, &[u8])] = &[\n");
    for file_name in &file_names {
        let abs_path = patches_dir.join(file_name);
        generated.push_str(&format!("    ({file_name:?}, include_bytes!({abs_path:?})),\n"));
    }
    generated.push_str("];\n");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest_path = std::path::PathBuf::from(out_dir).join("embedded_patches.rs");
    std::fs::write(&dest_path, generated)
        .unwrap_or_else(|e| panic!("build.rs: failed to write {dest_path:?}: {e}"));
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