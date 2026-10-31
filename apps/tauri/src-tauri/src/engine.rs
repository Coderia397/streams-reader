//! One playback facade over two platform engines, so commands and the feeder never branch on the OS.
//!
//! | | Desktop | Android |
//! |---|---|---|
//! | Engine | `streams_audio::Engine` (decode thread + cpal) | Media3 `PlaybackService` via `tauri-plugin-streams-media` |
//! | Media controls | `media_controls` (souvlaki) | MediaSession (notification, lock screen, Bluetooth) |
//! | Rate | Sonic (`libsonic`) | ExoPlayer `PlaybackParameters` (Sonic inside) |
//! | Events → UI | engine `Event` → `streams:player` | Kotlin `Channel` → `streams:player` |
//!
//! TODO(phase 1, Android): `start` creates a `tauri::ipc::Channel` whose handler re-emits each Kotlin
//! `BridgeEvent` envelope as `streams:player` / `streams:update`, and passes it with `set_event_handler`.
//! TODO(phase 2, desktop): `start` spawns `streams_audio::Engine`, a forwarder thread from its `Event`s to
//! `streams:player` + souvlaki, and maps `PlayerCommand` → `streams_audio::Command`.

use streams_native_bridge::{BridgeError, LoadArgs, PlayerCommand, PlayerState, Snapshot};
use tauri::AppHandle;

pub struct Engine {
    _app: AppHandle,
}

impl Engine {
    pub fn start(app: &AppHandle) -> Result<Self, BridgeError> {
        Ok(Self { _app: app.clone() })
    }

    /// New document: the feeder starts rendering/downloading from `args.start_segment`, the platform engine opens
    /// an empty playlist and starts as soon as the first segment is on disk.
    pub fn load(&self, _args: LoadArgs) -> Result<(), BridgeError> {
        Err(BridgeError::unsupported("native player (phase 1)"))
    }

    pub fn control(&self, _command: PlayerCommand) -> Result<(), BridgeError> {
        Err(BridgeError::unsupported("native player (phase 1)"))
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: PlayerState::Idle,
            segment: 0,
            segment_pos_ms: 0,
            buffered_ahead_ms: 0,
            rate: 1.0,
            sleep_left_ms: None,
            at_ms: 0,
        }
    }
}
