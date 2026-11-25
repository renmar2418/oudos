//! Build script generating Tauri v2 Windows resources and capability schemas.

fn main() {
    tauri_build::build();

    // Link the Tauri Common Controls v6 Windows manifest into `--lib` / integration test
    // harnesses as well as `--bin` targets so `comctl32.dll` v6 entrypoints resolve cleanly.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        if let Some(out_dir) = std::env::var_os("OUT_DIR") {
            let out_dir = std::path::PathBuf::from(out_dir);
            for candidate in ["libresource.a", "resource.lib"] {
                let path = out_dir.join(candidate);
                if path.exists() {
                    println!("cargo:rustc-link-arg-tests={}", path.display());
                    break;
                }
            }
        }
    }
}
