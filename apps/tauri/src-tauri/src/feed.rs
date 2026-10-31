//! The feeder: keeps audio on disk ahead of the listener, without the webview.
//!
//! Why native: on Android the webview's timers are throttled or stopped while the screen is off, so the web app's
//! `RenderSession` cannot be trusted to request renders during background playback (assumed; acceptance check in
//! phase 1). The feeder does the same work in the core process:
//!
//! 1. `POST {SITE_URL}/api/render` with the `RenderItem`s of the window around the play position (same request as
//!    `crates/web/src/provider/session.rs`, `x-nolabs-api: 2`, `Authorization: Bearer <access token>`).
//! 2. Poll render status with backoff for queued items (same rules: min 800 ms between polls of one window).
//! 3. For each ready item: `GET /api/audio/{key}?t=<audio token>` into `audio/{engine}/{key}.webm` (write to
//!    `.part`, fsync, rename), sidecar with duration and words, then hand it to the engine (`Append`) and emit
//!    `SegmentReady` to the UI.
//! 4. Keep ≥ 3 min of contiguous audio ahead (spec in notes/09) during playback; a download keeps going to the end.
//!
//! Tokens: the access token comes from [`crate::commands::auth`] (refreshed natively from the keystore token), so
//! the feeder keeps working with the UI gone. The window/backoff bookkeeping should be shared with the web app by
//! moving `crates/web/src/provider/table.rs` (pure) into `nolabs-core` (open item for the web owner).
//!
//! TODO(phase 1): implement as a tokio task per loaded document + one per download, cancelled on unload.

/// Minutes of audio kept ready ahead of the play position.
pub const AHEAD_MS: u32 = 3 * 60 * 1000;
/// Parallel clip downloads.
pub const PARALLEL_FETCHES: usize = 3;
