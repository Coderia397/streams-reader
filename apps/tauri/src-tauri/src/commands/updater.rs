//! Self-update. Channel manifests live at `https://listen.streams.watch/updates/` (R2 bucket behind the Worker):
//!
//! - Desktop: `tauri-plugin-updater` 2.14 with endpoint `updates/desktop/{channel}/latest.json`, minisign public key
//!   in tauri.conf.json, `requireSignedVersion: true` (the signature binds the version, so a replayed old artefact
//!   cannot be passed off as new). Looks up `{os}-{arch}-{installer}` (e.g. `windows-x86_64-nsis`,
//!   `linux-x86_64-deb`) before `{os}-{arch}`. Install = download, verify, install, `app.restart()`.
//! - Android: `updates/android/{channel}/latest.json` = `{ version, versionCode, url, sha256, size, signature,
//!   notes, pubDate, minSdk }`. The core downloads the APK into `updates/`, checks size + SHA-256 + minisign
//!   signature (same public key, `minisign-verify`), then calls the Kotlin `installApk` (PackageInstaller).
//!
//! Progress and results go out as `streams:update` (`UpdateEvent`). The UI checks on start (at most every 6 h) and
//! when the Worker answers 409 `upgrade_required` (then `UpdateInfo.required = true`).
//!
//! TODO(phase 3): implement both paths; channel stored in `state.json`.

use streams_native_bridge::{BridgeError, UpdateChannel, UpdateInfo};
use tauri::State;

use super::CmdResult;
use crate::state::{AppState, SITE_URL};

/// Manifest URL for a platform family and channel.
pub fn manifest_url(family: &str, channel: UpdateChannel) -> String {
    format!("{SITE_URL}/updates/{family}/{}/latest.json", channel.as_str())
}

#[tauri::command]
pub async fn update_check(state: State<'_, AppState>, channel: UpdateChannel) -> CmdResult<Option<UpdateInfo>> {
    if let Ok(mut c) = state.channel.lock() {
        *c = channel;
    }
    Err(BridgeError::unsupported("updater (phase 3)"))
}

#[tauri::command]
pub async fn update_install(_state: State<'_, AppState>) -> CmdResult<()> {
    Err(BridgeError::unsupported("updater (phase 3)"))
}
