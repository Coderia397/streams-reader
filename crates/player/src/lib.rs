//! Gapless audiobook player for the browser. See notes/09-build-spec.md for the contract with the web app.

use std::rc::Rc;

use futures::future::LocalBoxFuture;
use nolabs_core::proto::api::WordTiming;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlayerError {
    #[error("audio is not supported in this browser: {0}")]
    Unsupported(String),
    #[error("could not load audio: {0}")]
    Load(String),
    #[error("{0}")]
    Other(String),
}

pub struct Track {
    pub segment: u32,
    pub bytes: js_sys::Uint8Array,
    pub duration_ms: u32,
    pub words: Vec<WordTiming>,
    pub ends_section: bool,
}

pub type Provider = Rc<dyn Fn(u32) -> LocalBoxFuture<'static, Result<Track, PlayerError>>>;

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
    After { ms: u32 },
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
    pub word: Option<usize>,
    pub buffered_ahead_ms: u32,
    pub rate: f64,
    pub sleep_left_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    Snapshot(Snapshot),
    SegmentChanged(u32),
    Ended,
    SleepFired,
    Error(String),
}

pub struct Player {
    _provider: Provider,
    _on_event: Rc<dyn Fn(PlayerEvent)>,
}

impl Player {
    pub fn new(provider: Provider, on_event: Rc<dyn Fn(PlayerEvent)>) -> Result<Player, PlayerError> {
        Ok(Player { _provider: provider, _on_event: on_event })
    }
    pub fn load(&self, info: MediaInfo, segment_count: u32, start_segment: u32) {
        let _ = (info, segment_count, start_segment);
        todo!("implemented by the player agent")
    }
    pub fn play(&self) {
        todo!("implemented by the player agent")
    }
    pub fn pause(&self) {
        todo!("implemented by the player agent")
    }
    pub fn toggle(&self) {
        todo!("implemented by the player agent")
    }
    pub fn seek_by(&self, delta_ms: i32) {
        let _ = delta_ms;
        todo!("implemented by the player agent")
    }
    pub fn jump_to(&self, segment: u32) {
        let _ = segment;
        todo!("implemented by the player agent")
    }
    pub fn next(&self) {
        todo!("implemented by the player agent")
    }
    pub fn previous(&self) {
        todo!("implemented by the player agent")
    }
    pub fn set_rate(&self, rate: f64) {
        let _ = rate;
        todo!("implemented by the player agent")
    }
    pub fn set_sleep(&self, sleep: Sleep) {
        let _ = sleep;
        todo!("implemented by the player agent")
    }
    pub fn snapshot(&self) -> Snapshot {
        todo!("implemented by the player agent")
    }
    pub fn unload(&self) {
        todo!("implemented by the player agent")
    }
}
