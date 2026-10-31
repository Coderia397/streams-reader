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
        // Paused with play pressed is the element trying to play from the end of the buffered audio.
        Input::Waiting => match state {
            S::Playing | S::Buffering | S::Paused if want_play => S::Buffering,
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

/// Media Session `playbackState` for this state: what the lock screen's play/pause button shows.
pub fn session_state(state: &PlayerState, want_play: bool) -> &'static str {
    match state {
        PlayerState::Idle => "none",
        _ if is_active(state, want_play) => "playing",
        _ => "paused",
    }
}

/// How far the lock screen's position may run from the playhead before it is published again.
pub const POSITION_DRIFT_S: f64 = 0.1;

/// The last position given to the Media Session. While the session's playback state is "playing" the browser moves it
/// on at the playback rate from the moment of the call, whether or not the element's clock is moving.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionMark {
    /// Wall clock of the call, in milliseconds.
    pub wall_ms: f64,
    /// Media time published.
    pub media_s: f64,
    pub rate: f64,
}

impl PositionMark {
    /// Media time the lock screen shows at `now_ms`; `running` is whether the session's playback state is "playing".
    pub fn shown_s(&self, now_ms: f64, running: bool) -> f64 {
        if running { self.media_s + (now_ms - self.wall_ms).max(0.0) / 1000.0 * self.rate } else { self.media_s }
    }

    /// True when the lock screen is more than [`POSITION_DRIFT_S`] away from the playhead at `media_s`.
    pub fn drifted(&self, now_ms: f64, running: bool, media_s: f64) -> bool {
        (self.shown_s(now_ms, running) - media_s).abs() > POSITION_DRIFT_S
    }
}

/// The element's clock is running when it moved more than this between two probe rounds.
pub const CLOCK_MOVED_S: f64 = 0.01;
/// Consecutive rounds the clock must run with the published position before the probe stops: Chrome's clock can
/// move a few milliseconds when `playing` fires, then stand until the audio output starts.
pub const CLOCK_STEADY_ROUNDS: u32 = 3;

/// Watches the element's clock after a start or a seek while the session says "playing", where the lock screen
/// extrapolates the last published position at the playback rate whether or not audio comes out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockProbe {
    last_s: f64,
    steady: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeStep {
    /// Publish the position again; the lock screen is, or is about to be, ahead of (or behind) the clock.
    Republish,
    /// The clock runs with the published position; look again next round.
    Watch,
    /// Steady for [`CLOCK_STEADY_ROUNDS`]: the probe can stop.
    Done,
}

impl ClockProbe {
    pub fn new(media_s: f64) -> ClockProbe {
        ClockProbe { last_s: media_s, steady: 0 }
    }

    /// One round at wall time `now_ms`, with the clock at `media_s` and `mark` the last published position.
    pub fn step(&mut self, media_s: f64, mark: Option<PositionMark>, now_ms: f64) -> ProbeStep {
        let moving = media_s - self.last_s > CLOCK_MOVED_S;
        self.last_s = media_s;
        let agrees = mark.is_some_and(|mark| !mark.drifted(now_ms, true, media_s));
        if !(moving && agrees) {
            self.steady = 0;
            return ProbeStep::Republish;
        }
        self.steady += 1;
        if self.steady >= CLOCK_STEADY_ROUNDS { ProbeStep::Done } else { ProbeStep::Watch }
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
        assert_eq!(transition(&S::Paused, Input::Waiting, true), S::Buffering, "play pressed with nothing buffered");
        assert_eq!(transition(&S::Error("x".into()), Input::Waiting, true), S::Error("x".into()));
        assert_eq!(transition(&S::Ended, Input::Waiting, true), S::Ended);
    }

    #[test]
    fn lock_screen_position_is_republished_when_it_runs_away() {
        // Published at `playing`, 250 ms before the clock moved, at 2.5x.
        let mark = PositionMark { wall_ms: 1_000.0, media_s: 10.0, rate: 2.5 };
        assert_eq!(mark.shown_s(1_400.0, true), 11.0);
        assert_eq!(mark.shown_s(1_400.0, false), 10.0);
        assert_eq!(mark.shown_s(900.0, true), 10.0, "a clock that went backwards does not run it backwards");
        assert!(mark.drifted(1_400.0, true, 10.375), "playhead 0.625 s behind");
        assert!(!mark.drifted(1_400.0, true, 10.95));
        assert!(!mark.drifted(5_000.0, false, 10.05), "paused: not extrapolated");
        assert!(mark.drifted(5_000.0, false, 10.2));
    }

    #[test]
    fn the_clock_probe_holds_the_position_until_the_clock_runs_with_it() {
        let mark = |wall_ms: f64, media_s: f64| Some(PositionMark { wall_ms, media_s, rate: 2.0 });
        // `playing` at 1 000 ms published 0 s at 2x; the clock stands.
        let mut probe = ClockProbe::new(0.0);
        assert_eq!(probe.step(0.0, mark(1_000.0, 0.0), 1_050.0), ProbeStep::Republish);
        // A jitter of 30 ms that agrees with the fresh mark, then the clock stands again: not steady.
        assert_eq!(probe.step(0.03, mark(1_050.0, 0.0), 1_100.0), ProbeStep::Watch);
        assert_eq!(probe.step(0.033, mark(1_050.0, 0.0), 1_150.0), ProbeStep::Republish);
        // Audio starts at 2x: three rounds in step with the mark.
        let m = mark(1_150.0, 0.033);
        assert_eq!(probe.step(0.133, m, 1_200.0), ProbeStep::Watch);
        assert_eq!(probe.step(0.233, m, 1_250.0), ProbeStep::Watch);
        assert_eq!(probe.step(0.333, m, 1_300.0), ProbeStep::Done);
        // Moving, but slower than the lock screen assumes: publish again.
        let mut probe = ClockProbe::new(5.0);
        assert_eq!(probe.step(5.03, mark(1_000.0, 5.0), 1_100.0), ProbeStep::Republish);
        assert_eq!(ClockProbe::new(0.0).step(0.1, None, 0.0), ProbeStep::Republish, "nothing published");
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
    fn lock_screen_state_follows_intent_while_loading() {
        assert_eq!(session_state(&S::Loading, true), "playing");
        assert_eq!(session_state(&S::Loading, false), "paused");
        assert_eq!(session_state(&S::Playing, false), "playing");
        assert_eq!(session_state(&S::Error("x".into()), true), "paused");
        assert_eq!(session_state(&S::Idle, true), "none");
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
