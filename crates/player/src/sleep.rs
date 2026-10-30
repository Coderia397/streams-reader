//! Sleep timer. A countdown runs on playing time only (it stops while paused or buffering, like a physical
//! audiobook player's timer) and fades the volume linearly over its last 10 s; end of section stops in the silence
//! that closes a section-ending segment.

use crate::Sleep;
use crate::timeline::Entry;

/// Length of the linear fade before a countdown pauses playback.
pub const FADE_MS: u32 = 10_000;
/// How far before the end of a section-ending segment to stop. The Space appends 1.2 s of silence after a section
/// (notes/09), and `timeupdate` fires about every 250 ms of wall time (0.75 s of media at 3x), so stopping 0.9 s early
/// lands inside that silence at every supported rate without cutting speech.
pub const SECTION_TAIL_S: f64 = 0.9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SleepTimer {
    mode: Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Off,
    Countdown {
        left_ms: u32,
    },
    EndOfSection,
}

/// Result of advancing a countdown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SleepStep {
    /// No countdown running; leave the volume alone.
    Idle,
    /// Keep playing at this volume (1.0 outside the fade).
    Volume(f64),
    /// Time is up: pause, restore the volume and report it. The timer is now off.
    Fire,
}

impl SleepTimer {
    pub fn set(&mut self, sleep: Sleep) {
        self.mode = match sleep {
            Sleep::Off => Mode::Off,
            Sleep::After { ms } => Mode::Countdown { left_ms: ms },
            Sleep::EndOfSection => Mode::EndOfSection,
        };
    }

    pub fn is_end_of_section(&self) -> bool {
        self.mode == Mode::EndOfSection
    }

    /// Countdown remaining, for the snapshot.
    pub fn left_ms(&self) -> Option<u32> {
        match self.mode {
            Mode::Countdown { left_ms } => Some(left_ms),
            _ => None,
        }
    }

    /// Volume the element should have right now (1.0 unless a countdown is inside its fade).
    pub fn volume(&self) -> f64 {
        self.left_ms().map_or(1.0, fade_volume)
    }

    /// Counts `elapsed_ms` of playing time off a countdown.
    pub fn advance(&mut self, elapsed_ms: f64) -> SleepStep {
        let Mode::Countdown { left_ms } = self.mode else {
            return SleepStep::Idle;
        };
        let elapsed = if elapsed_ms.is_finite() { elapsed_ms.clamp(0.0, f64::from(u32::MAX)) as u32 } else { 0 };
        let left_ms = left_ms.saturating_sub(elapsed);
        if left_ms == 0 {
            self.mode = Mode::Off;
            SleepStep::Fire
        } else {
            self.mode = Mode::Countdown { left_ms };
            SleepStep::Volume(fade_volume(left_ms))
        }
    }

    /// For end of section: the media time to stop at when playback moved from `prev_s` to `now_s` across the tail of
    /// a section-ending entry. Returns that entry's end, where the next section starts, and turns the timer off.
    pub fn section_stop(&mut self, entries: &[Entry], prev_s: f64, now_s: f64) -> Option<f64> {
        if !self.is_end_of_section() {
            return None;
        }
        let stop = crossed_section_end(entries, prev_s, now_s)?;
        self.mode = Mode::Off;
        Some(stop)
    }
}

/// Linear fade: full volume until [`FADE_MS`] remain, then down to silence at zero.
pub fn fade_volume(left_ms: u32) -> f64 {
    (f64::from(left_ms) / f64::from(FADE_MS)).clamp(0.0, 1.0)
}

/// End time of the first section-ending entry whose stop point (`end - SECTION_TAIL_S`, never before its start) lies
/// in `(prev_s, now_s]`. Callers reset `prev_s` after a seek so a jump never counts as a crossing.
pub fn crossed_section_end(entries: &[Entry], prev_s: f64, now_s: f64) -> Option<f64> {
    entries.iter().filter(|e| e.ends_section).find_map(|e| {
        let stop_at = (e.end_s - SECTION_TAIL_S).max(e.start_s);
        (prev_s < stop_at && stop_at <= now_s).then_some(e.end_s)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(segment: u32, start_s: f64, end_s: f64, ends_section: bool) -> Entry {
        Entry { segment, start_s, end_s, words: vec![], ends_section }
    }

    #[test]
    fn fade_is_linear_over_the_last_ten_seconds() {
        assert_eq!(fade_volume(60_000), 1.0);
        assert_eq!(fade_volume(10_000), 1.0);
        assert_eq!(fade_volume(7_500), 0.75);
        assert_eq!(fade_volume(1_000), 0.1);
        assert_eq!(fade_volume(0), 0.0);
    }

    #[test]
    fn countdown_fades_then_fires_once() {
        let mut t = SleepTimer::default();
        t.set(Sleep::After { ms: 15_000 });
        assert_eq!(t.left_ms(), Some(15_000));
        assert_eq!(t.advance(4_000.0), SleepStep::Volume(1.0));
        assert_eq!(t.advance(6_000.0), SleepStep::Volume(0.5));
        assert_eq!(t.volume(), 0.5);
        assert_eq!(t.left_ms(), Some(5_000));
        assert_eq!(t.advance(5_250.0), SleepStep::Fire);
        assert_eq!(t.left_ms(), None);
        assert_eq!(t.advance(1_000.0), SleepStep::Idle);
        assert_eq!(t.volume(), 1.0);
    }

    #[test]
    fn bad_elapsed_values_do_not_move_the_countdown() {
        let mut t = SleepTimer::default();
        t.set(Sleep::After { ms: 20_000 });
        assert_eq!(t.advance(f64::NAN), SleepStep::Volume(1.0));
        assert_eq!(t.advance(-500.0), SleepStep::Volume(1.0));
        assert_eq!(t.left_ms(), Some(20_000));
        assert_eq!(t.advance(f64::INFINITY), SleepStep::Volume(1.0));
    }

    #[test]
    fn off_and_end_of_section_do_not_count_down() {
        let mut t = SleepTimer::default();
        assert_eq!(t.advance(1_000.0), SleepStep::Idle);
        t.set(Sleep::EndOfSection);
        assert_eq!(t.advance(1_000.0), SleepStep::Idle);
        assert_eq!(t.left_ms(), None);
        assert!(t.is_end_of_section());
        t.set(Sleep::Off);
        assert!(!t.is_end_of_section());
    }

    #[test]
    fn section_stop_fires_inside_the_closing_silence() {
        let entries = [entry(0, 0.0, 10.0, false), entry(1, 10.0, 20.0, true), entry(2, 20.0, 30.0, false)];
        let mut t = SleepTimer::default();
        t.set(Sleep::EndOfSection);
        assert_eq!(t.section_stop(&entries, 9.5, 18.9), None, "segment 0 does not end a section; 1 not reached");
        assert_eq!(t.section_stop(&entries, 18.9, 19.15), Some(20.0));
        assert!(!t.is_end_of_section(), "fires once");
        assert_eq!(t.section_stop(&entries, 18.9, 19.15), None);
    }

    #[test]
    fn section_stop_catches_up_when_the_boundary_was_skipped() {
        let entries = [entry(1, 10.0, 20.0, true), entry(2, 20.0, 30.0, false)];
        assert_eq!(crossed_section_end(&entries, 18.5, 20.3), Some(20.0));
        assert_eq!(crossed_section_end(&entries, 19.2, 20.3), None, "already past the stop point");
    }

    #[test]
    fn short_section_segments_stop_no_earlier_than_their_start() {
        let entries = [entry(4, 10.0, 10.5, true)];
        assert_eq!(crossed_section_end(&entries, 9.9, 10.0), Some(10.5));
        assert_eq!(crossed_section_end(&entries, 9.0, 9.95), None);
    }
}
