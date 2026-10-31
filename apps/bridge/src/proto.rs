//! IPC names and payloads. Every `#[tauri::command]` in `apps/tauri/src-tauri/src/commands` takes and returns these
//! types, and [`crate::tauri::TauriBridge`] sends them. Changing a field name is a breaking change for both sides;
//! bump [`BRIDGE_VERSION`] when you do, and the UI refuses to start against a different shell version.

use nolabs_core::proto::api::{Engine, RenderItem, WordTiming};
use serde::{Deserialize, Serialize};

/// Version of this contract. The shell reports its own in [`AppInfo::bridge_version`].
pub const BRIDGE_VERSION: u32 = 1;

/// Tauri command names (`#[tauri::command] fn <name>`). Each takes one argument object named in the comment.
pub mod cmd {
    /// `()` → [`super::AppInfo`]
    pub const APP_INFO: &str = "app_info";
    /// `{ args: LoadArgs }` → `()`
    pub const PLAYER_LOAD: &str = "player_load";
    /// `{ command: PlayerCommand }` → `()`
    pub const PLAYER_CONTROL: &str = "player_control";
    /// `()` → [`super::Snapshot`]
    pub const PLAYER_SNAPSHOT: &str = "player_snapshot";
    /// `{ args: DownloadArgs }` → `()`
    pub const DOWNLOAD_START: &str = "download_start";
    /// `{ docId }` → `()`
    pub const DOWNLOAD_CANCEL: &str = "download_cancel";
    /// `{ docId }` → `()`
    pub const DOWNLOAD_REMOVE: &str = "download_remove";
    /// `()` → `Vec<DownloadInfo>`
    pub const DOWNLOAD_LIST: &str = "download_list";
    /// `()` → [`super::StorageUsage`]
    pub const STORAGE_USAGE: &str = "storage_usage";
    /// `()` → [`super::StorageUsage`]
    pub const CACHE_CLEAR: &str = "cache_clear";
    /// `{ session: NewSession }` → [`super::SessionView`]
    pub const AUTH_STORE: &str = "auth_store";
    /// `()` → `Option<SessionView>`
    pub const AUTH_SESSION: &str = "auth_session";
    /// `()` → [`super::SessionView`]
    pub const AUTH_REFRESH: &str = "auth_refresh";
    /// `()` → `()`
    pub const AUTH_SIGN_OUT: &str = "auth_sign_out";
    /// `{ url }` → `()`
    pub const AUTH_OPEN_BROWSER: &str = "auth_open_browser";
    /// `{ channel: UpdateChannel }` → `Option<UpdateInfo>`
    pub const UPDATE_CHECK: &str = "update_check";
    /// `()` → `()`
    pub const UPDATE_INSTALL: &str = "update_install";
    /// `()` → `Vec<OpenRequest>`
    pub const OPEN_TAKE_PENDING: &str = "open_take_pending";
    /// `{ path }` → raw bytes (an `ArrayBuffer` in the webview)
    pub const FILE_READ: &str = "file_read";

    /// Every app command, in the order `build.rs` registers them with the ACL.
    pub const ALL: &[&str] = &[
        APP_INFO,
        PLAYER_LOAD,
        PLAYER_CONTROL,
        PLAYER_SNAPSHOT,
        DOWNLOAD_START,
        DOWNLOAD_CANCEL,
        DOWNLOAD_REMOVE,
        DOWNLOAD_LIST,
        STORAGE_USAGE,
        CACHE_CLEAR,
        AUTH_STORE,
        AUTH_SESSION,
        AUTH_REFRESH,
        AUTH_SIGN_OUT,
        AUTH_OPEN_BROWSER,
        UPDATE_CHECK,
        UPDATE_INSTALL,
        OPEN_TAKE_PENDING,
        FILE_READ,
    ];
}

/// Tauri event names emitted by the shell (`app.emit(name, payload)`). Payloads in brackets.
pub mod event {
    /// [`super::PlayerEvent`], about 4 per second while playing and on every state change.
    pub const PLAYER: &str = "streams:player";
    /// [`super::DownloadInfo`] whenever a download moves.
    pub const DOWNLOAD: &str = "streams:download";
    /// [`super::UpdateEvent`]
    pub const UPDATE: &str = "streams:update";
    /// [`super::OpenRequest`]: a file, link or text opened with or shared to the app while it runs.
    pub const OPEN: &str = "streams:open";
    /// `{ url }`: a `nolabs://auth/...` deep link (OAuth / magic-link callback).
    pub const AUTH_CALLBACK: &str = "streams:auth-callback";

    pub const ALL: &[&str] = &[PLAYER, DOWNLOAD, UPDATE, OPEN, AUTH_CALLBACK];
}

// ------------------------------------------------------------------------------------------------ app

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Web,
    Macos,
    Windows,
    Linux,
    Android,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// Playback runs natively (Rust engine on desktop, Media3 service on Android) and survives a hidden webview.
    pub native_player: bool,
    /// Whole documents can be rendered and stored on disk for offline listening.
    pub downloads: bool,
    /// The app can update itself.
    pub updater: bool,
    /// The refresh token lives in the OS keystore and the native side refreshes it.
    pub keystore: bool,
    /// Lock screen / notification / OS media keys.
    pub media_controls: bool,
    /// Files and shares can arrive from the OS.
    pub open_files: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub platform: Platform,
    /// App version (semver), as shown in Settings and compared by the updater.
    pub version: String,
    pub bridge_version: u32,
    pub channel: UpdateChannel,
    pub capabilities: Capabilities,
}

impl AppInfo {
    /// What a browser tab reports: no native capabilities.
    pub fn web(version: &str) -> Self {
        Self {
            platform: Platform::Web,
            version: version.to_owned(),
            bridge_version: BRIDGE_VERSION,
            channel: UpdateChannel::Stable,
            capabilities: Capabilities::default(),
        }
    }
}

// ------------------------------------------------------------------------------------------------ player

/// Lock screen / notification metadata for the whole document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// `https://` URL; the native side caches it on disk for the notification and Now Playing.
    pub artwork_url: Option<String>,
}

/// A segment the UI already knows to be rendered (from its own IndexedDB), so the native side can skip asking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownSegment {
    pub segment: u32,
    /// Content address of the audio (`audio/{engine}/{key}.webm`).
    pub key: String,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadArgs {
    pub doc_id: String,
    pub voice_id: String,
    pub engine: Engine,
    /// One item per segment, exactly what the web app would send to `/api/render`.
    pub items: Vec<RenderItem>,
    #[serde(default)]
    pub known: Vec<KnownSegment>,
    pub start_segment: u32,
    #[serde(default)]
    pub start_ms: u32,
    pub info: MediaInfo,
    pub rate: f64,
    #[serde(default)]
    pub sleep: Sleep,
    #[serde(default)]
    pub skip: Skip,
    pub autoplay: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "kind")]
pub enum Sleep {
    #[default]
    Off,
    After {
        ms: u32,
    },
    EndOfSection,
}

/// Seek-back / seek-forward step for the lock screen and headset buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skip {
    pub back_ms: u32,
    pub forward_ms: u32,
}

impl Default for Skip {
    fn default() -> Self {
        Self { back_ms: 15_000, forward_ms: 15_000 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "cmd")]
pub enum PlayerCommand {
    Play,
    Pause,
    Toggle,
    SeekBy { delta_ms: i32 },
    SeekTo { segment: u32, ms: u32 },
    Next,
    Previous,
    SetRate { rate: f64 },
    SetSleep { sleep: Sleep },
    SetSkip { skip: Skip },
    Unload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "kind")]
pub enum PlayerState {
    Idle,
    Loading,
    Buffering,
    Playing,
    Paused,
    Ended,
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub state: PlayerState,
    pub segment: u32,
    /// Position inside the segment in source time (before the rate change), so word timings apply directly.
    pub segment_pos_ms: u32,
    pub buffered_ahead_ms: u32,
    pub rate: f64,
    pub sleep_left_ms: Option<u32>,
    /// Native monotonic clock when sampled. The UI extrapolates the position between snapshots with
    /// `segment_pos_ms + (now - at_ms) * rate` while `state` is `Playing`, for smooth word highlighting.
    pub at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "type")]
pub enum PlayerEvent {
    Snapshot { snapshot: Snapshot },
    SegmentChanged { segment: u32 },
    /// Audio for a segment is on disk; carries what the UI needs for highlighting.
    SegmentReady { segment: u32, duration_ms: u32, words: Vec<WordTiming> },
    Ended,
    SleepFired,
    Error { message: String, retryable: bool },
}

// ------------------------------------------------------------------------------------------------ downloads

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadArgs {
    pub doc_id: String,
    pub voice_id: String,
    pub engine: Engine,
    pub items: Vec<RenderItem>,
    pub title: String,
    #[serde(default)]
    pub known: Vec<KnownSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "kind")]
pub enum DownloadState {
    Queued,
    /// Waiting for the Worker / Space to render.
    Rendering,
    /// Fetching rendered audio.
    Fetching,
    /// Waiting for a network or a sign-in.
    Paused { reason: String },
    Done,
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadInfo {
    pub doc_id: String,
    pub voice_id: String,
    pub title: String,
    pub state: DownloadState,
    pub segments_done: u32,
    pub segments_total: u32,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    /// All audio on disk.
    pub audio_bytes: u64,
    /// Part of `audio_bytes` pinned by downloads (never evicted).
    pub pinned_bytes: u64,
    /// Unpinned audio is evicted, least recently played first, above this size.
    pub cache_limit_bytes: u64,
}

// ------------------------------------------------------------------------------------------------ session

/// A sign-in completed by the UI (email code, magic link or OAuth with PKCE against Supabase Auth).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSession {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_ms: i64,
    pub user_id: String,
    pub email: Option<String>,
}

/// What the UI gets back: never the refresh token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub access_token: String,
    pub expires_at_ms: i64,
    pub user_id: String,
    pub email: Option<String>,
}

// ------------------------------------------------------------------------------------------------ updates

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

impl UpdateChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
    /// RFC 3339.
    pub pub_date: Option<String>,
    pub size_bytes: Option<u64>,
    /// The Worker answered 409 `upgrade_required`: the UI blocks until this is installed.
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "kind")]
pub enum UpdateEvent {
    Progress { downloaded: u64, total: Option<u64> },
    /// Desktop: installed, the app restarts next.
    Installed,
    /// Android: the system needs the user (first time "install unknown apps", or a confirmation dialog).
    NeedsUserAction { message: String },
    Failed { message: String },
}

// ------------------------------------------------------------------------------------------------ open / share

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "kind")]
pub enum OpenRequest {
    /// A file opened with the app or shared to it, copied into the app's `imports/` folder.
    File { path: String, name: String, mime: Option<String>, size: u64 },
    /// A link shared to the app (Android share sheet) or a `nolabs://open?url=` deep link.
    Url { url: String },
    /// Plain text shared to the app.
    Text { text: String, subject: Option<String> },
}

// ------------------------------------------------------------------------------------------------ events and errors

/// Every event the UI can receive, decoded from the Tauri event it came in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "source")]
pub enum BridgeEvent {
    Player { event: PlayerEvent },
    Download { info: DownloadInfo },
    Update { event: UpdateEvent },
    Open { request: OpenRequest },
    AuthCallback { url: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unsupported,
    NotSignedIn,
    Network,
    Storage,
    Busy,
    Invalid,
    Internal,
}

/// The error every command rejects with (serialised as `{ code, message }`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct BridgeError {
    pub code: ErrorCode,
    pub message: String,
}

impl BridgeError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
    pub fn unsupported(what: &str) -> Self {
        Self::new(ErrorCode::Unsupported, format!("{what} is not available on this platform"))
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn command_names_are_unique_snake_case() {
        let mut seen = std::collections::HashSet::new();
        for c in cmd::ALL {
            assert!(seen.insert(*c), "duplicate {c}");
            assert!(c.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_'), "{c}");
        }
    }

    #[test]
    fn event_names_use_only_characters_tauri_accepts() {
        // Tauri 2 event names: alphanumeric, '-', '/', ':', '_'.
        for e in event::ALL {
            assert!(e.chars().all(|ch| ch.is_ascii_alphanumeric() || "-/:_".contains(ch)), "{e}");
        }
    }

    #[test]
    fn player_command_wire_shape() {
        let v = serde_json::to_value(PlayerCommand::SeekBy { delta_ms: -15_000 }).ok();
        assert_eq!(v, Some(json!({ "cmd": "seekBy", "deltaMs": -15000 })));
        let v = serde_json::to_value(PlayerCommand::SetSleep { sleep: Sleep::After { ms: 600_000 } }).ok();
        assert_eq!(v, Some(json!({ "cmd": "setSleep", "sleep": { "kind": "after", "ms": 600000 } })));
    }

    #[test]
    fn error_wire_shape() {
        let v = serde_json::to_value(BridgeError::new(ErrorCode::NotSignedIn, "sign in")).ok();
        assert_eq!(v, Some(json!({ "code": "not_signed_in", "message": "sign in" })));
    }

    #[test]
    fn load_args_accept_minimal_json() {
        let v = json!({
            "docId": "d", "voiceId": "v", "engine": "kokoro", "items": [{ "text": "Hello." }],
            "startSegment": 0, "info": { "title": "t", "artist": "a", "album": "b", "artworkUrl": null },
            "rate": 1.0, "autoplay": true
        });
        let args: Result<LoadArgs, _> = serde_json::from_value(v);
        let args = args.map_err(|e| e.to_string());
        assert!(args.is_ok(), "{args:?}");
        if let Ok(a) = args {
            assert_eq!(a.sleep, Sleep::Off);
            assert_eq!(a.skip, Skip::default());
        }
    }
}
