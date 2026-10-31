//! Offline downloads. A download renders every segment of (document, voice) on the server, fetches every clip into
//! `audio/`, and writes `downloads/{doc_id}.json`, which pins those clips against cache eviction. Progress goes out
//! as `streams:download` (`DownloadInfo`). Downloads survive app restarts (resumed from the JSON on start) and pause
//! without network or session.
//!
//! TODO(phase 2): implement with the feeder (`crate::feed`), one task per download, at most one running at a time.

use streams_native_bridge::{BridgeError, DownloadArgs, DownloadInfo, StorageUsage};
use tauri::State;

use super::CmdResult;
use crate::paths::DEFAULT_CACHE_LIMIT;
use crate::state::AppState;

#[tauri::command]
pub fn download_start(_state: State<'_, AppState>, args: DownloadArgs) -> CmdResult<()> {
    let _ = args;
    Err(BridgeError::unsupported("downloads (phase 2)"))
}

#[tauri::command]
pub fn download_cancel(_state: State<'_, AppState>, doc_id: String) -> CmdResult<()> {
    let _ = doc_id;
    Err(BridgeError::unsupported("downloads (phase 2)"))
}

#[tauri::command]
pub fn download_remove(_state: State<'_, AppState>, doc_id: String) -> CmdResult<()> {
    let _ = doc_id;
    Err(BridgeError::unsupported("downloads (phase 2)"))
}

#[tauri::command]
pub fn download_list(_state: State<'_, AppState>) -> CmdResult<Vec<DownloadInfo>> {
    Ok(Vec::new())
}

#[tauri::command]
pub fn storage_usage(_state: State<'_, AppState>) -> CmdResult<StorageUsage> {
    // TODO(phase 2): walk audio/ once at start, then keep a running total.
    Ok(StorageUsage { audio_bytes: 0, pinned_bytes: 0, cache_limit_bytes: DEFAULT_CACHE_LIMIT })
}

#[tauri::command]
pub fn cache_clear(state: State<'_, AppState>) -> CmdResult<StorageUsage> {
    storage_usage(state)
}
