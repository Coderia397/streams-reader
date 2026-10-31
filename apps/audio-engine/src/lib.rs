//! Desktop audio engine for Streams Reader (macOS, Windows, Linux). Android uses Media3 instead
//! (`apps/android-plugin`). Design and the reasons for each choice: `notes/18-native-apps-architecture.md`.
//!
//! Threads:
//! ```text
//!  app (tokio) ──Command──▶ decode thread ──f32 frames + tags──▶ rtrb ring (≈400 ms) ──▶ cpal callback ──▶ device
//!       ▲                        │  demux (symphonia mkv) → Opus (libopus) → trim → stretch (Sonic) → gain
//!       └──────Event────────── clock (frames played ↔ segment/source ms)
//! ```
//! - Gapless: segments are decoded back to back into one continuous PCM stream; the output stream is never
//!   restarted between segments. Each file's encoder pre-skip is dropped by the decoder and its end padding is cut
//!   to the duration in the sidecar (see [`source`]).
//! - Rate: [`stretch`] changes speed without changing pitch. Rate 1.0 bypasses it. A rate change is heard after
//!   at most the ring length (≈400 ms), which is why the ring is short; look-ahead lives on disk, not in memory.
//! - Position: [`clock`] maps output frames back to (segment, source ms), so word timings from the Space apply
//!   unchanged at any rate.
//! - Nothing in the audio callback allocates, locks or logs.
//!
//! The engine knows files, not documents: the app's feeder (`apps/tauri/src-tauri/src/feed.rs`) renders, downloads
//! and appends ready segments with [`Engine::send`].

pub mod clock;
pub mod output;
pub mod queue;
pub mod source;
pub mod stretch;

use std::path::PathBuf;
use std::sync::mpsc;

/// Opus always decodes at 48 kHz; the engine runs at this rate internally.
pub const ENGINE_RATE: u32 = 48_000;
/// Decoded audio between the decode thread and the callback.
pub const RING_MS: u32 = 400;
/// Allowed playback rates (same range as `nolabs_player::{MIN_RATE, MAX_RATE}` on the web).
pub const MIN_RATE: f32 = 0.5;
pub const MAX_RATE: f32 = 3.0;
/// Sleep timer fade-out, as on the web player.
pub const SLEEP_FADE_MS: u32 = 10_000;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("no audio output device")]
    NoDevice,
    #[error("audio output: {0}")]
    Output(String),
    #[error("cannot decode {path}: {message}")]
    Decode { path: PathBuf, message: String },
    #[error("engine stopped")]
    Stopped,
}

/// A segment ready on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentFile {
    pub segment: u32,
    /// `<data>/audio/{engine}/{key}.webm`
    pub path: PathBuf,
    /// Exact audio length from the render sidecar; decoded samples beyond it (Opus end padding) are dropped.
    pub duration_ms: u32,
    /// Last segment of a section, for the "end of section" sleep timer.
    pub ends_section: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Start a new document: forget the queue, expect segments from `start_segment` on.
    Open {
        segment_count: u32,
        start_segment: u32,
        start_ms: u32,
    },
    /// A segment became available (in any order; the queue sorts and waits for gaps).
    Append(SegmentFile),
    Play,
    Pause,
    SeekTo {
        segment: u32,
        ms: u32,
    },
    SeekBy {
        delta_ms: i32,
    },
    SetRate(f32),
    /// `None` = off; `Some(ms)` = fade and pause after `ms`; end-of-section is handled by the app via `ends_section`.
    SetSleep(Option<u32>),
    SetVolume(f32),
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    State(EngineState),
    /// Sent ~4×/s while playing and on every seek.
    Position {
        segment: u32,
        segment_pos_ms: u32,
        buffered_ahead_ms: u32,
        at_ms: u64,
    },
    SegmentChanged(u32),
    /// The queue needs `segment` and does not have it: the feeder should hurry (render / download).
    Starved(u32),
    Ended,
    SleepFired,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    Idle,
    Buffering,
    Playing,
    Paused,
    Ended,
}

/// Handle to the engine threads. Cheap to clone the sender; dropping the last handle stops the engine.
pub struct Engine {
    commands: mpsc::Sender<Command>,
}

impl Engine {
    /// Open the default output device and start the decode thread. Events arrive on `events` (the app forwards
    /// them to the UI and to the OS media controls).
    ///
    /// TODO(phase 2): spawn `queue::run` on a named thread, build `output::Output` with the ring consumer, re-open
    /// the device on `cpal` stream errors (headphones unplugged → pause, like Android's "becoming noisy").
    pub fn start(events: mpsc::Sender<Event>) -> Result<Self, AudioError> {
        let (tx, rx) = mpsc::channel();
        queue::spawn(rx, events)?;
        Ok(Self { commands: tx })
    }

    pub fn send(&self, command: Command) -> Result<(), AudioError> {
        self.commands.send(command).map_err(|_| AudioError::Stopped)
    }
}
