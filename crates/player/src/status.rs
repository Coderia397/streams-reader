//! The player's state machine and the small policies around it (snapshot throttle, playback rate range).

use crate::PlayerState;

/// Something that can change the state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// A (re)build started: a document was loaded, or a seek or jump left the buffer.
    Build,
    /// The first segment of a build is appended and the playhead is on its target.
    Ready,
    /// The element fired `playing`.
    Playing,
    /// The element fired `pause` (not the one that precedes `ended`).
    Paused,
    /// The element fired `waiting`: the playhead reached the end of the buffer.
    Waiting,
    /// Playback reached the end of the last segment.
    Ended,
    /// A fatal error, or a fetch failure while nothing is left to play.
    Failed(String),
    /// The document was unloaded.
    Unload,
}

/// Next state. `want_play` is whether the listener has asked to hear audio (play pressed and not paused since).
pub fn transition(state: &PlayerState, input: Input, want_play: bool) -> PlayerState {
    use PlayerState as S;
    match input {
        Input::Build => S::Loading,
        Input::Ready if want_play => S::Buffering,
        Input::Ready => S::Paused,
        Input::Playing => S::Playing,
        // A build replaces the element's source, which can pause it; that is not the listener pausing.
        Input::Paused => match state {
            S::Idle | S::Loading | S::Ended | S::Error(_) => state.clone(),
            S::Buffering | S::Playing | S::Paused => S::Paused,
        },
        Input::Waiting => match state {
            S::Playing | S::Buffering if want_play => S::Buffering,
            _ => state.clone(),
        },
        Input::Ended => S::Ended,
        Input::Failed(message) => S::Error(message),
        Input::Unload => S::Idle,
    }
}

/// Whether the listener hears, or is about to hear, audio: what `toggle` and the Media Session report.
pub fn is_active(state: &PlayerState, want_play: bool) -> bool {
    match state {
        PlayerState::Playing | PlayerState::Buffering => true,
        PlayerState::Loading => want_play,
        PlayerState::Idle | PlayerState::Paused | PlayerState::Ended | PlayerState::Error(_) => false,
    }
}

/// Playback rates the UI offers (notes/06 v2 #11).
pub const MIN_RATE: f64 = 0.5;
pub const MAX_RATE: f64 = 3.0;

pub fn clamp_rate(rate: f64) -> f64 {
    if rate.is_finite() { rate.clamp(MIN_RATE, MAX_RATE) } else { 1.0 }
}

/// Snapshot period while visible and playing: about 8 Hz, enough for word highlighting without flooding the UI.
pub const SNAPSHOT_INTERVAL_MS: f64 = 125.0;

/// Lets one event through per interval; `now_ms` is a monotonic clock (the `requestAnimationFrame` timestamp).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Throttle {
    interval_ms: f64,
    last_ms: Option<f64>,
}

impl Throttle {
    pub const fn new(interval_ms: f64) -> Self {
        Throttle { interval_ms, last_ms: None }
    }

    pub fn ready(&mut self, now_ms: f64) -> bool {
        match self.last_ms {
            // A clock that went backwards (a new page lifecycle) restarts the throttle.
            Some(last) if now_ms >= last && now_ms - last < self.interval_ms => false,
            _ => {
                self.last_ms = Some(now_ms);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PlayerState as S;

    #[test]
    fn build_then_ready_respects_intent() {
        assert_eq!(transition(&S::Idle, Input::Build, false), S::Loading);
        assert_eq!(transition(&S::Playing, Input::Build, true), S::Loading);
        assert_eq!(transition(&S::Loading, Input::Ready, false), S::Paused);
        assert_eq!(transition(&S::Loading, Input::Ready, true), S::Buffering);
        assert_eq!(transition(&S::Buffering, Input::Playing, true), S::Playing);
    }

    #[test]
    fn pause_during_a_build_is_ignored() {
        assert_eq!(transition(&S::Loading, Input::Paused, true), S::Loading);
        assert_eq!(transition(&S::Ended, Input::Paused, false), S::Ended);
        assert_eq!(transition(&S::Error("x".into()), Input::Paused, false), S::Error("x".into()));
        assert_eq!(transition(&S::Playing, Input::Paused, false), S::Paused);
        assert_eq!(transition(&S::Buffering, Input::Paused, false), S::Paused);
    }

    #[test]
    fn waiting_only_matters_while_trying_to_play() {
        assert_eq!(transition(&S::Playing, Input::Waiting, true), S::Buffering);
        assert_eq!(transition(&S::Paused, Input::Waiting, false), S::Paused);
        assert_eq!(transition(&S::Loading, Input::Waiting, true), S::Loading);
        assert_eq!(transition(&S::Playing, Input::Waiting, false), S::Playing);
    }

    #[test]
    fn terminal_inputs() {
        assert_eq!(transition(&S::Playing, Input::Ended, true), S::Ended);
        assert_eq!(transition(&S::Buffering, Input::Failed("gone".into()), true), S::Error("gone".into()));
        assert_eq!(transition(&S::Error("gone".into()), Input::Playing, true), S::Playing, "recovers when audio flows");
        assert_eq!(transition(&S::Playing, Input::Unload, true), S::Idle);
    }

    #[test]
    fn activity() {
        assert!(is_active(&S::Playing, false));
        assert!(is_active(&S::Buffering, true));
        assert!(is_active(&S::Loading, true));
        assert!(!is_active(&S::Loading, false));
        assert!(!is_active(&S::Paused, true));
        assert!(!is_active(&S::Ended, true));
    }

    #[test]
    fn rate_is_clamped() {
        assert_eq!(clamp_rate(1.25), 1.25);
        assert_eq!(clamp_rate(0.1), 0.5);
        assert_eq!(clamp_rate(9.0), 3.0);
        assert_eq!(clamp_rate(f64::NAN), 1.0);
        assert_eq!(clamp_rate(f64::INFINITY), 1.0);
    }

    #[test]
    fn throttle_lets_one_through_per_interval() {
        let mut t = Throttle::new(SNAPSHOT_INTERVAL_MS);
        assert!(t.ready(1_000.0));
        assert!(!t.ready(1_016.0));
        assert!(!t.ready(1_124.0));
        assert!(t.ready(1_125.0));
        assert!(!t.ready(1_200.0));
        assert!(t.ready(1_300.0));
        assert!(t.ready(10.0), "clock went backwards");
    }
}
