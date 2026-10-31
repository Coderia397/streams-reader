// Registers every app command with Tauri's ACL so capabilities/default.json must grant each one explicitly
// (`allow-<command-with-hyphens>`). The list is the bridge contract's, so a command cannot exist without a grant.
fn main() {
    let attrs = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(streams_native_bridge::cmd::ALL));
    if let Err(e) = tauri_build::try_build(attrs) {
        println!("cargo:warning=tauri-build failed: {e:#}");
        std::process::exit(1);
    }
}
