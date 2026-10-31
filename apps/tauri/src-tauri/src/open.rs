//! Files and links arriving from the OS: desktop argv (first launch and, through single-instance, later launches),
//! macOS `RunEvent::Opened`, Android share/VIEW intents (collected by the Kotlin `ShareIntake`).
//!
//! Each file is copied into `imports/` (so `file_read` only ever serves that folder), turned into an
//! [`OpenRequest`], queued for `open_take_pending` and emitted as `streams:open` for a UI that is already running.
//! Supported: `.epub .pdf .txt .md` (bundle.fileAssociations in tauri.conf.json), and `nolabs://open?url=...`.

use streams_native_bridge::{OpenRequest, event};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::state::AppState;

const EXTENSIONS: &[&str] = &["epub", "pdf", "txt", "md"];

/// Paths among `argv` (skipping the executable) with a supported extension.
pub fn supported_paths(argv: &[String]) -> Vec<std::path::PathBuf> {
    argv.iter()
        .skip(1)
        .map(std::path::PathBuf::from)
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        })
        .collect()
}

pub fn from_argv<R: Runtime>(app: &AppHandle<R>, argv: &[String]) {
    for path in supported_paths(argv) {
        // TODO(phase 4): copy into imports/ with a size limit, fill name/mime/size.
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let req = OpenRequest::File { path: path.to_string_lossy().into_owned(), name, mime: None, size: 0 };
        deliver(app, req);
    }
}

#[cfg(target_os = "macos")]
pub fn from_urls<R: Runtime>(app: &AppHandle<R>, urls: &[tauri::Url]) {
    let argv: Vec<String> = std::iter::once(String::new())
        .chain(urls.iter().filter_map(|u| u.to_file_path().ok()).map(|p| p.to_string_lossy().into_owned()))
        .collect();
    from_argv(app, &argv);
}

fn deliver<R: Runtime>(app: &AppHandle<R>, req: OpenRequest) {
    if let Some(state) = app.try_state::<AppState>()
        && let Ok(mut q) = state.pending_open.lock()
    {
        q.push(req.clone());
    }
    let _ = app.emit(event::OPEN, req);
}

#[cfg(test)]
mod tests {
    #[test]
    fn keeps_only_supported_documents() {
        let argv = ["/app", "/a/book.EPUB", "/a/x.exe", "/a/notes.md", "--flag"].map(String::from);
        let got = super::supported_paths(&argv);
        assert_eq!(got.len(), 2);
    }
}
