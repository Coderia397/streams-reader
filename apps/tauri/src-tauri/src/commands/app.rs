//! `app_info`: the first call the UI makes (`TauriBridge::connect`). The UI refuses to run against a shell with a
//! different `bridge_version` and shows "update the app" instead.

use streams_native_bridge::{AppInfo, BRIDGE_VERSION, Capabilities, Platform};
use tauri::State;

use super::CmdResult;
use crate::state::AppState;

fn platform() -> Platform {
    if cfg!(target_os = "android") {
        Platform::Android
    } else if cfg!(target_os = "macos") {
        Platform::Macos
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> CmdResult<AppInfo> {
    let channel = state.channel.lock().map(|c| *c).unwrap_or_default();
    // Capabilities switch on as each phase lands (notes/18, phased plan); the UI keeps its web paths until then.
    let capabilities = Capabilities {
        native_player: false,
        downloads: false,
        updater: false,
        keystore: false,
        media_controls: false,
        open_files: true,
    };
    Ok(AppInfo {
        platform: platform(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        bridge_version: BRIDGE_VERSION,
        channel,
        capabilities,
    })
}
