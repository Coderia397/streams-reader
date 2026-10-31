//! Gapless audiobook player for the browser. See notes/09-build-spec.md for the contract with the web app.
//!
//! Segments are appended in order to one `sequence`-mode SourceBuffer on a hidden `<audio>` element, so the browser
//! plays the document as one continuous stream and keeps playing with the screen off. A [`timeline`] of what was
//! appended maps the element's clock back to (segment, offset, word). Seeks inside the buffer move the playhead;
//! anything else rebuilds the MediaSource from the target segment. Browsers without WebM/Opus in MediaSource fall back
//! to one blob URL per segment on the same element.
//!
//! The decisions (timeline maths, buffer window, seek planning, sleep timer, state machine) are pure modules that
//! build and test on the host; `web` holds the DOM driver and builds for wasm32 only.

// The pure modules are used by the wasm32 driver; on the host only their tests use them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod buffer;
mod seat;
mod seek;
mod sleep;
mod status;
mod timeline;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

use std::rc::Rc;

use futures::future::LocalBoxFuture;
use nolabs_core::proto::api::WordTiming;

/// Playback rate range [`Player::set_rate`] accepts; the UI should offer the same range.
pub use status::{MAX_RATE, MIN_RATE};

#[cfg(not(target_arch = "wasm32"))]
use native::Driver;
#[cfg(target_arch = "wasm32")]
use web::Driver;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlayerError {
    #[error("audio is not supported in this browser: {0}")]
    Unsupported(String),
    /// Retryable: the player backs off and asks the provider again.
    #[error("could not load audio: {0}")]
    Load(String),
    #[error("{0}")]
    Other(String),
}

/// One segment's audio, as the provider hands it over.
pub struct Track {
    pub segment: u32,
    /// A complete WebM/Opus file.
    pub bytes: js_sys::Uint8Array,
    pub duration_ms: u32,
    pub words: Vec<WordTiming>,
    /// The segment closes a section (heading or scene break follows); "sleep at end of section" stops after it.
    pub ends_section: bool,
}

/// Fetches one segment; the future resolves when its audio is ready. Called in order, one at a time.
pub type Provider = Rc<dyn Fn(u32) -> LocalBoxFuture<'static, Result<Track, PlayerError>>>;

/// What the lock screen and notification show.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sleep {
    Off,
    /// Pause after this much playing time, fading out over the last 10 s.
    After {
        ms: u32,
    },
    /// Pause when the current section's last segment finishes.
    EndOfSection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerState {
    Idle,
    Loading,
    Buffering,
    Playing,
    Paused,
    Ended,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub state: PlayerState,
    pub segment: u32,
    pub segment_pos_ms: u32,
    /// Index into the segment's word timings.
    pub word: Option<usize>,
    pub buffered_ahead_ms: u32,
    pub rate: f64,
    pub sleep_left_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    /// About 8 Hz while visible and playing, within a frame of each change of the spoken word, and on every state
    /// change, seek, rate or sleep change.
    Snapshot(Snapshot),
    /// The playhead moved into another segment (also emitted at the start of a rebuild for its target segment).
    SegmentChanged(u32),
    /// Playback finished the last segment.
    Ended,
    SleepFired,
    Error(String),
}

/// The player. Events are delivered synchronously to `on_event`, never while the player holds internal state, so the
/// callback may call back into the player.
pub struct Player {
    driver: Driver,
}

impl Player {
    pub fn new(provider: Provider, on_event: Rc<dyn Fn(PlayerEvent)>) -> Result<Player, PlayerError> {
        Ok(Player { driver: Driver::new(provider, on_event)? })
    }
    /// Opens a document of `segment_count` segments at `start_segment`, paused. Call [`Player::play`] to start.
    pub fn load(&self, info: MediaInfo, segment_count: u32, start_segment: u32) {
        self.driver.load(info, segment_count, start_segment);
    }
    pub fn play(&self) {
        self.driver.play();
    }
    pub fn pause(&self) {
        self.driver.pause();
    }
    pub fn toggle(&self) {
        self.driver.toggle();
    }
    pub fn seek_by(&self, delta_ms: i32) {
        self.driver.seek_by(delta_ms);
    }
    /// Plays from the start of `segment` (in place when it is still buffered).
    pub fn jump_to(&self, segment: u32) {
        self.driver.jump_to(segment);
    }
    pub fn next(&self) {
        self.driver.next();
    }
    /// Restarts the current segment after its first 3 s, otherwise goes to the previous one.
    pub fn previous(&self) {
        self.driver.previous();
    }
    /// Clamped to 0.5–3.0; pitch is preserved.
    pub fn set_rate(&self, rate: f64) {
        self.driver.set_rate(rate);
    }
    pub fn set_sleep(&self, sleep: Sleep) {
        self.driver.set_sleep(sleep);
    }
    /// Skip lengths for the lock screen, notification and headset seek buttons when they do not name their own (the
    /// app's skip setting). 0 restores the default of 15 s.
    pub fn set_skip(&self, back_ms: u32, forward_ms: u32) {
        self.driver.set_skip(back_ms, forward_ms);
    }
    pub fn snapshot(&self) -> Snapshot {
        self.driver.snapshot()
    }
    /// Stops playback and releases the document's audio; the player can load another one.
    pub fn unload(&self) {
        self.driver.unload();
    }
}
