//! Files and shares handed to the app by the OS (see `crate::open`). The UI imports them with the same
//! `nolabs-import` code as on the web, reading the bytes through `file_read`.

use streams_native_bridge::{BridgeError, ErrorCode, OpenRequest};
use tauri::State;
use tauri::ipc::Response;

use super::CmdResult;
use crate::state::AppState;

/// Largest file the importer accepts (EPUB/PDF); larger files are refused before reading.
const MAX_IMPORT_BYTES: u64 = 100 * 1024 * 1024;

#[tauri::command]
pub fn open_take_pending(state: State<'_, AppState>) -> CmdResult<Vec<OpenRequest>> {
    let mut q = state.pending_open.lock().map_err(|_| BridgeError::internal("open queue poisoned"))?;
    // TODO(phase 4, Android): also drain the Kotlin ShareIntake (`streams_media().take_shared()`).
    Ok(std::mem::take(&mut *q))
}

/// Raw bytes (an `ArrayBuffer` in the webview, no JSON/base64 overhead). Only files inside `imports/`.
#[tauri::command]
pub async fn file_read(state: State<'_, AppState>, path: String) -> Result<Response, BridgeError> {
    let p = std::path::PathBuf::from(&path);
    if !state.paths.is_import(&p) {
        return Err(BridgeError::new(ErrorCode::Invalid, "only imported files can be read"));
    }
    let meta = tokio::fs::metadata(&p).await.map_err(|e| BridgeError::new(ErrorCode::Storage, e.to_string()))?;
    if meta.len() > MAX_IMPORT_BYTES {
        return Err(BridgeError::new(ErrorCode::Invalid, "file is larger than 100 MB"));
    }
    let bytes = tokio::fs::read(&p).await.map_err(|e| BridgeError::new(ErrorCode::Storage, e.to_string()))?;
    Ok(Response::new(bytes))
}
