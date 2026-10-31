//! Rust face of the Android layer (`android/`, Kotlin). The app's core calls these methods; each one runs a
//! `@Command` on `watch.streams.reader.media.MediaPlugin` through Tauri's mobile plugin bridge.
//!
//! On desktop the plugin registers but every method answers `Unsupported`; the desktop app uses
//! `streams-audio` + souvlaki instead (see `apps/tauri/src-tauri/src/engine`).

use serde::{Deserialize, Serialize};
use streams_native_bridge::{BridgeError, ErrorCode, MediaInfo, Skip, Sleep};
use tauri::plugin::{Builder, PluginApi, TauriPlugin};
use tauri::{Manager, Runtime};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "watch.streams.reader.media";

/// One playable segment handed to ExoPlayer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub segment: u32,
    /// `file://` URI of `<files>/audio/{engine}/{key}.webm`.
    pub uri: String,
    pub duration_ms: u32,
    pub ends_section: bool,
}

/// Arguments of the Kotlin `open` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenArgs {
    pub doc_id: String,
    pub segment_count: u32,
    /// Estimated per-segment durations (ms) for the document-level progress bar until real ones arrive.
    pub estimated_ms: Vec<u32>,
    pub start_segment: u32,
    pub start_ms: u32,
    pub info: MediaInfo,
    /// Local file path of the cached cover, or None.
    pub artwork_path: Option<String>,
    pub rate: f64,
    pub sleep: Sleep,
    pub skip: Skip,
    pub autoplay: bool,
}

/// The Android layer. Clone is cheap.
pub struct StreamsMedia<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    _app: tauri::AppHandle<R>,
}

#[cfg(target_os = "android")]
fn bridge_err(e: tauri::plugin::mobile::PluginInvokeError) -> BridgeError {
    BridgeError::new(ErrorCode::Internal, e.to_string())
}

impl<R: Runtime> StreamsMedia<R> {
    #[cfg(target_os = "android")]
    fn run<T: serde::de::DeserializeOwned>(&self, cmd: &str, args: impl Serialize) -> Result<T, BridgeError> {
        self.handle.run_mobile_plugin(cmd, args).map_err(bridge_err)
    }

    #[cfg(not(target_os = "android"))]
    fn run<T: serde::de::DeserializeOwned>(&self, cmd: &str, _args: impl Serialize) -> Result<T, BridgeError> {
        Err(BridgeError::new(ErrorCode::Unsupported, format!("{cmd}: Android only")))
    }

    /// Give Kotlin a channel for player events (`PlayerEvent` JSON, see `MediaPlugin.setEventHandler`).
    pub fn set_event_handler(&self, channel: tauri::ipc::Channel<serde_json::Value>) -> Result<(), BridgeError> {
        self.run("setEventHandler", serde_json::json!({ "handler": channel }))
    }
    /// Start the MediaSessionService (foreground while playing) with a new document.
    pub fn open(&self, args: OpenArgs) -> Result<(), BridgeError> {
        self.run("open", args)
    }
    /// Segments now on disk, appended to ExoPlayer's playlist in order.
    pub fn append(&self, items: Vec<QueueItem>) -> Result<(), BridgeError> {
        self.run("append", serde_json::json!({ "items": items }))
    }
    /// Transport: the same `PlayerCommand` JSON the UI sends.
    pub fn control(&self, command: &streams_native_bridge::PlayerCommand) -> Result<(), BridgeError> {
        self.run("control", serde_json::json!({ "command": command }))
    }
    pub fn snapshot(&self) -> Result<streams_native_bridge::Snapshot, BridgeError> {
        self.run("snapshot", serde_json::json!({}))
    }

    /// Secrets in an app-private file encrypted with an AES-GCM key held by the Android Keystore.
    pub fn secret_get(&self, key: &str) -> Result<Option<String>, BridgeError> {
        #[derive(Deserialize)]
        struct Out {
            value: Option<String>,
        }
        self.run::<Out>("secretGet", serde_json::json!({ "key": key })).map(|o| o.value)
    }
    pub fn secret_set(&self, key: &str, value: &str) -> Result<(), BridgeError> {
        self.run("secretSet", serde_json::json!({ "key": key, "value": value }))
    }
    pub fn secret_delete(&self, key: &str) -> Result<(), BridgeError> {
        self.run("secretDelete", serde_json::json!({ "key": key }))
    }

    /// Install a downloaded, already verified APK through `PackageInstaller` (self-update).
    pub fn install_apk(&self, path: &str) -> Result<(), BridgeError> {
        self.run("installApk", serde_json::json!({ "path": path }))
    }
    /// Shares and VIEW intents received before the UI asked (copied into `imports/`).
    pub fn take_shared(&self) -> Result<Vec<streams_native_bridge::OpenRequest>, BridgeError> {
        #[derive(Deserialize)]
        struct Out {
            items: Vec<streams_native_bridge::OpenRequest>,
        }
        self.run::<Out>("takeShared", serde_json::json!({})).map(|o| o.items)
    }
}

/// Access from anywhere with an `AppHandle`.
pub trait StreamsMediaExt<R: Runtime> {
    fn streams_media(&self) -> &StreamsMedia<R>;
}

impl<R: Runtime, T: Manager<R>> StreamsMediaExt<R> for T {
    fn streams_media(&self) -> &StreamsMedia<R> {
        self.state::<StreamsMedia<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("streams-media")
        .setup(|app, api: PluginApi<R, ()>| {
            #[cfg(target_os = "android")]
            let media = {
                let _ = app;
                StreamsMedia { handle: api.register_android_plugin(PLUGIN_IDENTIFIER, "MediaPlugin")? }
            };
            #[cfg(not(target_os = "android"))]
            let media = {
                let _ = api;
                StreamsMedia { _app: app.clone() }
            };
            app.manage(media);
            Ok(())
        })
        .build()
}
