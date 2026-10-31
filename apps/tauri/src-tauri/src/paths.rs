//! On-disk layout under the app data directory (Tauri `app_data_dir()`):
//!
//! | OS | Root |
//! |---|---|
//! | macOS | `~/Library/Application Support/watch.streams.reader/` |
//! | Windows | `%APPDATA%\watch.streams.reader\` |
//! | Linux | `~/.local/share/watch.streams.reader/` |
//! | Android | `/data/data/watch.streams.reader/files/` (app-private, removed on uninstall) |
//!
//! ```text
//! audio/{kokoro|ming}/{key}.webm        content-addressed clips (same keys as R2), shared by every document
//! audio/{kokoro|ming}/{key}.json        sidecar: duration_ms, words (from the render status)
//! downloads/{doc_id}.json               a download: voice, engine, segment keys, state (pins its clips)
//! covers/{sha256(url)}.img              cached artwork for the notification / Now Playing
//! imports/                              files opened with / shared to the app, deleted after import
//! updates/                              Android APK staging (desktop bundles are handled by the updater plugin)
//! state.json                            last document, segment, position, rate (resume after restart)
//! ```
//! Unpinned clips are evicted least recently played first above `cache_limit_bytes` (default 1 GiB).
//! The webview keeps its own IndexedDB (document bodies, prefs) exactly as on the web.

use std::path::{Path, PathBuf};

use nolabs_core::proto::api::Engine as Voice;
use tauri::{AppHandle, Manager, Runtime};

pub const DEFAULT_CACHE_LIMIT: u64 = 1 << 30;

pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn resolve<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Self> {
        Ok(Self { root: app.path().app_data_dir()? })
    }

    pub fn create_all(&self) -> std::io::Result<()> {
        for d in ["audio/kokoro", "audio/ming", "downloads", "covers", "imports", "updates"] {
            std::fs::create_dir_all(self.root.join(d))?;
        }
        Ok(())
    }

    pub fn audio(&self, engine: Voice, key: &str) -> PathBuf {
        self.root.join("audio").join(engine_dir(engine)).join(format!("{key}.webm"))
    }

    pub fn sidecar(&self, engine: Voice, key: &str) -> PathBuf {
        self.root.join("audio").join(engine_dir(engine)).join(format!("{key}.json"))
    }

    pub fn download(&self, doc_id: &str) -> PathBuf {
        self.root.join("downloads").join(format!("{doc_id}.json"))
    }

    pub fn imports(&self) -> PathBuf {
        self.root.join("imports")
    }

    pub fn updates(&self) -> PathBuf {
        self.root.join("updates")
    }

    /// True when `path` is inside the app's `imports/` folder: `file_read` serves nothing else.
    pub fn is_import(&self, path: &Path) -> bool {
        match (path.canonicalize(), self.imports().canonicalize()) {
            (Ok(p), Ok(dir)) => p.starts_with(dir),
            _ => false,
        }
    }
}

fn engine_dir(engine: Voice) -> &'static str {
    match engine {
        Voice::Kokoro => "kokoro",
        Voice::Ming => "ming",
    }
}
