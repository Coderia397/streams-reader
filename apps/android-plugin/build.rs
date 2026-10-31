// The Kotlin side is called from the app's Rust core (`run_mobile_plugin`), never from the webview, so the plugin
// registers no webview commands and needs no permissions.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).android_path("android").build();
}
