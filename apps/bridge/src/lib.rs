//! The contract between the Streams Reader UI and the native shells. Design: `notes/18-native-apps-architecture.md`.
//!
//! The Leptos app (`crates/web`) runs in a browser tab or inside a Tauri webview. It talks to the platform through
//! one object, a [`NativeBridge`]:
//! - in a browser, `crates/web` provides `WebBridge`, which reports no native capabilities and keeps today's paths
//!   (MediaSource player, IndexedDB, Cache API);
//! - inside Tauri, [`tauri::TauriBridge`] (wasm32 only) forwards every call to the Rust core of the app through
//!   `window.__TAURI__.core.invoke` and turns Tauri events into [`BridgeEvent`]s.
//!
//! The UI decides once at start-up: [`tauri::TauriBridge::connect`] succeeds only inside the app. After that every
//! feature asks [`Capabilities`] instead of checking the platform, so a capability can move (for example downloads on
//! the web later) without touching the callers.
//!
//! [`proto`] holds the command and event names and every payload. Both sides compile against the same types; the
//! JSON field names are part of the contract (camelCase, because Tauri maps command arguments by camelCase name).

pub mod proto;

#[cfg(target_arch = "wasm32")]
pub mod tauri;

use std::future::Future;
use std::pin::Pin;

pub use proto::*;

/// A future returned by the bridge. Not `Send`: the UI runs on one thread (wasm32).
pub type BridgeFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, BridgeError>> + 'a>>;

fn unsupported<'a, T: 'a>(what: &'static str) -> BridgeFuture<'a, T> {
    Box::pin(async move { Err(BridgeError::unsupported(what)) })
}

/// Everything the UI can ask of the platform. Default methods answer [`ErrorCode::Unsupported`], so an
/// implementation only writes what its platform has; callers check [`AppInfo::capabilities`] first.
pub trait NativeBridge {
    /// Platform, versions and capabilities, read once at start-up.
    fn info(&self) -> &AppInfo;

    // ---- player (capability `native_player`) ----

    /// Open a document for playback. Replaces whatever was loaded. The native side owns rendering ahead,
    /// downloading audio and the playlist from here on; the UI only shows state.
    fn player_load(&self, _args: LoadArgs) -> BridgeFuture<'_, ()> {
        unsupported("player_load")
    }
    /// Play, pause, seek, rate, sleep timer, unload.
    fn player_control(&self, _command: PlayerCommand) -> BridgeFuture<'_, ()> {
        unsupported("player_control")
    }
    /// Current state, for a view that mounts while audio is already playing (for example after the webview was
    /// recreated on Android).
    fn player_snapshot(&self) -> BridgeFuture<'_, Snapshot> {
        unsupported("player_snapshot")
    }

    // ---- offline downloads (capability `downloads`) ----

    fn download_start(&self, _args: DownloadArgs) -> BridgeFuture<'_, ()> {
        unsupported("download_start")
    }
    fn download_cancel(&self, _doc_id: String) -> BridgeFuture<'_, ()> {
        unsupported("download_cancel")
    }
    fn download_remove(&self, _doc_id: String) -> BridgeFuture<'_, ()> {
        unsupported("download_remove")
    }
    fn download_list(&self) -> BridgeFuture<'_, Vec<DownloadInfo>> {
        unsupported("download_list")
    }
    fn storage_usage(&self) -> BridgeFuture<'_, StorageUsage> {
        unsupported("storage_usage")
    }
    /// Drop cached audio that no download pins.
    fn cache_clear(&self) -> BridgeFuture<'_, StorageUsage> {
        unsupported("cache_clear")
    }

    // ---- session (capability `keystore`) ----

    /// Hand a fresh sign-in to the native side. From then on the native side is the only holder of the refresh
    /// token (OS keystore) and the only one that refreshes, so two refreshers never race on a rotating token.
    fn auth_store(&self, _session: NewSession) -> BridgeFuture<'_, SessionView> {
        unsupported("auth_store")
    }
    /// The current session with a valid access token (refreshed natively when it is within a minute of expiry),
    /// or `None` when signed out.
    fn auth_session(&self) -> BridgeFuture<'_, Option<SessionView>> {
        unsupported("auth_session")
    }
    /// Refresh now (after a 401 from the API).
    fn auth_refresh(&self) -> BridgeFuture<'_, SessionView> {
        unsupported("auth_refresh")
    }
    fn auth_sign_out(&self) -> BridgeFuture<'_, ()> {
        unsupported("auth_sign_out")
    }
    /// Open an OAuth authorise URL in the system browser; the result comes back as [`BridgeEvent::AuthCallback`].
    fn auth_open_browser(&self, _url: String) -> BridgeFuture<'_, ()> {
        unsupported("auth_open_browser")
    }

    // ---- updates (capability `updater`) ----

    fn update_check(&self, _channel: UpdateChannel) -> BridgeFuture<'_, Option<UpdateInfo>> {
        unsupported("update_check")
    }
    /// Download, verify and install the update found by the last check. Desktop restarts the app when done;
    /// Android hands the APK to the system installer.
    fn update_install(&self) -> BridgeFuture<'_, ()> {
        unsupported("update_install")
    }

    // ---- files and shares (capability `open_files`) ----

    /// Files, links and text the app was opened with before the UI subscribed.
    fn open_take_pending(&self) -> BridgeFuture<'_, Vec<OpenRequest>> {
        unsupported("open_take_pending")
    }
    /// Bytes of a file handed over in an [`OpenRequest::File`].
    fn file_read(&self, _path: String) -> BridgeFuture<'_, Vec<u8>> {
        unsupported("file_read")
    }

    // ---- events ----

    /// Receive every [`BridgeEvent`] until the returned [`Subscription`] is dropped.
    fn subscribe(&self, handler: Box<dyn Fn(BridgeEvent)>) -> Subscription;
}

/// Keeps an event subscription alive; dropping it unsubscribes.
pub struct Subscription {
    on_drop: Option<Box<dyn FnOnce()>>,
}

impl Subscription {
    pub fn new(on_drop: impl FnOnce() + 'static) -> Self {
        Self { on_drop: Some(Box::new(on_drop)) }
    }
    /// A subscription for a platform that never sends events.
    pub fn none() -> Self {
        Self { on_drop: None }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(f) = self.on_drop.take() {
            f();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bare(AppInfo);
    impl NativeBridge for Bare {
        fn info(&self) -> &AppInfo {
            &self.0
        }
        fn subscribe(&self, _handler: Box<dyn Fn(BridgeEvent)>) -> Subscription {
            Subscription::none()
        }
    }

    #[test]
    fn defaults_answer_unsupported() {
        let b = Bare(AppInfo::web("0.1.0"));
        let fut = b.player_control(PlayerCommand::Play);
        let out = futures_lite_block_on(fut);
        assert!(matches!(out, Err(BridgeError { code: ErrorCode::Unsupported, .. })));
        assert!(!b.info().capabilities.native_player);
    }

    /// Polls a future that is ready on first poll (the default methods are).
    fn futures_lite_block_on<T>(mut fut: BridgeFuture<'_, T>) -> Result<T, BridgeError> {
        use std::task::{Context, Poll, Waker};
        let mut cx = Context::from_waker(Waker::noop());
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => v,
            Poll::Pending => Err(BridgeError::internal("pending")),
        }
    }

    #[test]
    fn subscription_runs_its_drop_hook_once() {
        use std::cell::Cell;
        use std::rc::Rc;
        let hits = Rc::new(Cell::new(0));
        let h = hits.clone();
        drop(Subscription::new(move || h.set(h.get() + 1)));
        assert_eq!(hits.get(), 1);
    }
}
