//! OS media controls on desktop with `souvlaki` 0.9.3: macOS Now Playing (MPNowPlayingInfoCenter + remote command
//! center), Windows System Media Transport Controls, Linux MPRIS (`use_zbus`: pure Rust, own thread, no libdbus).
//!
//! - Created on the main thread (`app.run_on_main_thread`): macOS needs the main run loop, which Tauri runs.
//! - Windows needs the main window's HWND (`WebviewWindow::hwnd()`), so creation waits until the window exists.
//! - `PlatformConfig { dbus_name: "watch.streams.reader", display_name: "Streams Reader", hwnd }`.
//! - `attach(handler)`: Play / Pause / Toggle / Next / Previous / Seek(direction) / SeekBy / SetPosition → the same
//!   `PlayerCommand`s the UI sends, so the engine has one input path.
//! - `set_metadata`: document title, author, album = product name, cover from `covers/` as a `file://` URL, duration
//!   = whole document (sum of segment durations), so the OS shows document progress, not a 20 s segment.
//! - `set_playback(Playing { progress })` on state changes and seeks only (MPRIS clients interpolate).
//!
//! TODO(phase 2): `MediaControls::start(app) -> Option<Self>` (None when the platform refuses, for example no
//! session bus on Linux), `update(&Snapshot, &MediaInfo)`.

pub const DBUS_NAME: &str = "watch.streams.reader";
pub const DISPLAY_NAME: &str = "Streams Reader";
