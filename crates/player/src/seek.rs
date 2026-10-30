//! Seek planning. A target inside the buffered ranges is a plain `currentTime` change; anything else tears the
//! MediaSource down and rebuilds it from the target segment, because a `sequence`-mode SourceBuffer can only grow at
//! its end.

use crate::buffer::is_buffered;
use crate::timeline::{Entry, Timeline, secs_to_ms};

/// `previous` restarts the current segment when more than this much of it has played.
pub const RESTART_THRESHOLD_MS: u32 = 3_000;
/// Skip length for the ±15 s buttons and the Media Session seek actions.
pub const SKIP_MS: i32 = 15_000;

/// Where to land inside the first segment of a rebuild, resolved once that segment's real length is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offset {
    FromStart(u32),
    /// Used when seeking back past the start of the timeline: the earlier segment's length is unknown until fetched.
    FromEnd(u32),
}

impl Offset {
    pub const START: Offset = Offset::FromStart(0);

    /// The same target moved by `delta_ms`, for a skip pressed while the build is still loading. Clamped to the
    /// target segment; crossing into a neighbour would need its length, which is not known yet.
    pub fn shifted(self, delta_ms: i32) -> Offset {
        let shift = |ms: u32, delta: i64| (i64::from(ms) + delta).clamp(0, i64::from(u32::MAX)) as u32;
        match self {
            Offset::FromStart(ms) => Offset::FromStart(shift(ms, i64::from(delta_ms))),
            Offset::FromEnd(ms) => Offset::FromEnd(shift(ms, -i64::from(delta_ms))),
        }
    }

    /// The media time this offset names inside `entry`, clamped to the entry.
    pub fn resolve(self, entry: &Entry) -> f64 {
        match self {
            Offset::FromStart(ms) => (entry.start_s + f64::from(ms) / 1000.0).min(entry.end_s),
            Offset::FromEnd(ms) => (entry.end_s - f64::from(ms) / 1000.0).max(entry.start_s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeekPlan {
    /// Set `currentTime`.
    InBuffer { time_s: f64 },
    /// Tear down and rebuild from this segment.
    Rebuild { segment: u32, offset: Offset },
    /// Nothing to do (no document loaded, or the target does not exist).
    Ignore,
}

/// Plan a relative seek of `delta_ms` from `current_s`.
pub fn plan_seek_by(
    timeline: &Timeline,
    ranges: &[(f64, f64)],
    current_s: f64,
    delta_ms: i32,
    segment_count: u32,
) -> SeekPlan {
    plan_seek_to(timeline, ranges, current_s + f64::from(delta_ms) / 1000.0, segment_count)
}

/// Plan an absolute seek to media time `target_s`.
pub fn plan_seek_to(timeline: &Timeline, ranges: &[(f64, f64)], target_s: f64, segment_count: u32) -> SeekPlan {
    let (Some(first), Some(last)) = (timeline.first(), timeline.last()) else {
        return SeekPlan::Ignore;
    };
    if target_s < first.start_s {
        return if first.segment == 0 {
            // Already at the beginning of the document: clamp to it.
            start_of(first, ranges)
        } else {
            SeekPlan::Rebuild {
                segment: first.segment - 1,
                offset: Offset::FromEnd(secs_to_ms(first.start_s - target_s)),
            }
        };
    }
    if target_s >= last.end_s {
        let next = last.segment.saturating_add(1);
        return if next >= segment_count {
            // Past the end of the document: land on its end so the element finishes.
            if is_buffered(ranges, last.end_s) {
                SeekPlan::InBuffer { time_s: last.end_s }
            } else {
                SeekPlan::Rebuild { segment: last.segment, offset: Offset::FromEnd(0) }
            }
        } else {
            SeekPlan::Rebuild { segment: next, offset: Offset::FromStart(secs_to_ms(target_s - last.end_s)) }
        };
    }
    if is_buffered(ranges, target_s) {
        return SeekPlan::InBuffer { time_s: target_s };
    }
    // Inside the timeline but evicted.
    match timeline.locate(target_s) {
        Some(pos) => SeekPlan::Rebuild { segment: pos.segment, offset: Offset::FromStart(pos.offset_ms) },
        None => SeekPlan::Ignore,
    }
}

/// Plan a jump to the start of `segment`: in place when that start is still buffered, otherwise a rebuild.
pub fn plan_jump(timeline: &Timeline, ranges: &[(f64, f64)], segment: u32, segment_count: u32) -> SeekPlan {
    if segment >= segment_count {
        return SeekPlan::Ignore;
    }
    match timeline.entry(segment) {
        Some(entry) => start_of(entry, ranges),
        None => SeekPlan::Rebuild { segment, offset: Offset::START },
    }
}

/// Segment `previous` should go to: the start of the current one once [`RESTART_THRESHOLD_MS`] of it has played,
/// otherwise the one before (the first segment restarts).
pub fn previous_target(segment: u32, offset_ms: u32) -> u32 {
    if offset_ms > RESTART_THRESHOLD_MS { segment } else { segment.saturating_sub(1) }
}

/// Segment `next` should go to, if any.
pub fn next_target(segment: u32, segment_count: u32) -> Option<u32> {
    segment.checked_add(1).filter(|&s| s < segment_count)
}

/// Skip length for a Media Session seekbackward/seekforward that may carry its own offset in seconds.
pub fn skip_ms(offset_s: f64) -> i32 {
    if offset_s.is_finite() && offset_s > 0.0 {
        (offset_s * 1000.0).round().min(f64::from(i32::MAX)) as i32
    } else {
        SKIP_MS
    }
}

fn start_of(entry: &Entry, ranges: &[(f64, f64)]) -> SeekPlan {
    if is_buffered(ranges, entry.start_s) {
        SeekPlan::InBuffer { time_s: entry.start_s }
    } else {
        SeekPlan::Rebuild { segment: entry.segment, offset: Offset::START }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Segments 3..=5, 10 s each, from media time 0; the first 12 s were evicted.
    fn fixture() -> (Timeline, Vec<(f64, f64)>) {
        let mut t = Timeline::default();
        t.push(3, 0.0, 10.0, vec![], false);
        t.push(4, 10.0, 20.0, vec![], false);
        t.push(5, 20.0, 30.0, vec![], false);
        (t, vec![(12.0, 30.0)])
    }

    #[test]
    fn inside_the_buffer_is_a_plain_seek() {
        let (t, r) = fixture();
        assert_eq!(plan_seek_by(&t, &r, 15.0, 10_000, 10), SeekPlan::InBuffer { time_s: 25.0 });
        assert_eq!(plan_seek_by(&t, &r, 28.0, -15_000, 10), SeekPlan::InBuffer { time_s: 13.0 });
    }

    #[test]
    fn evicted_range_rebuilds_at_that_segment() {
        let (t, r) = fixture();
        assert_eq!(
            plan_seek_by(&t, &r, 20.0, -15_000, 10),
            SeekPlan::Rebuild { segment: 3, offset: Offset::FromStart(5_000) }
        );
        assert_eq!(plan_seek_to(&t, &r, 11.0, 10), SeekPlan::Rebuild { segment: 4, offset: Offset::FromStart(1_000) });
    }

    #[test]
    fn before_the_timeline_rebuilds_one_segment_back_from_its_end() {
        let (t, r) = fixture();
        assert_eq!(
            plan_seek_by(&t, &r, 2.0, -15_000, 10),
            SeekPlan::Rebuild { segment: 2, offset: Offset::FromEnd(13_000) }
        );
    }

    #[test]
    fn before_the_first_segment_of_the_document_clamps_to_zero() {
        let mut t = Timeline::default();
        t.push(0, 0.0, 10.0, vec![], false);
        assert_eq!(plan_seek_by(&t, &[(0.0, 10.0)], 3.0, -15_000, 5), SeekPlan::InBuffer { time_s: 0.0 });
        assert_eq!(
            plan_seek_by(&t, &[(5.0, 10.0)], 6.0, -15_000, 5),
            SeekPlan::Rebuild { segment: 0, offset: Offset::START }
        );
    }

    #[test]
    fn past_the_timeline_rebuilds_at_the_next_segment() {
        let (t, r) = fixture();
        assert_eq!(
            plan_seek_by(&t, &r, 25.0, 15_000, 10),
            SeekPlan::Rebuild { segment: 6, offset: Offset::FromStart(10_000) }
        );
    }

    #[test]
    fn past_the_end_of_the_document_lands_on_its_end() {
        let (t, r) = fixture();
        assert_eq!(plan_seek_by(&t, &r, 25.0, 15_000, 6), SeekPlan::InBuffer { time_s: 30.0 });
        assert_eq!(
            plan_seek_to(&t, &[(0.0, 5.0)], 40.0, 6),
            SeekPlan::Rebuild { segment: 5, offset: Offset::FromEnd(0) }
        );
    }

    #[test]
    fn nothing_loaded_is_ignored() {
        assert_eq!(plan_seek_by(&Timeline::default(), &[], 0.0, 15_000, 10), SeekPlan::Ignore);
    }

    #[test]
    fn jump_reuses_the_buffer_when_possible() {
        let (t, r) = fixture();
        assert_eq!(plan_jump(&t, &r, 5, 10), SeekPlan::InBuffer { time_s: 20.0 });
        assert_eq!(plan_jump(&t, &r, 3, 10), SeekPlan::Rebuild { segment: 3, offset: Offset::START });
        assert_eq!(plan_jump(&t, &r, 8, 10), SeekPlan::Rebuild { segment: 8, offset: Offset::START });
        assert_eq!(plan_jump(&t, &r, 10, 10), SeekPlan::Ignore);
    }

    #[test]
    fn previous_and_next_targets() {
        assert_eq!(previous_target(4, 3_001), 4);
        assert_eq!(previous_target(4, 3_000), 3);
        assert_eq!(previous_target(0, 500), 0);
        assert_eq!(next_target(4, 10), Some(5));
        assert_eq!(next_target(9, 10), None);
        assert_eq!(next_target(u32::MAX, u32::MAX), None);
    }

    #[test]
    fn media_session_skip_uses_the_offered_offset() {
        assert_eq!(skip_ms(f64::NAN), SKIP_MS);
        assert_eq!(skip_ms(0.0), SKIP_MS);
        assert_eq!(skip_ms(-5.0), SKIP_MS);
        assert_eq!(skip_ms(10.0), 10_000);
        assert_eq!(skip_ms(1e12), i32::MAX);
    }

    #[test]
    fn pending_offsets_shift_and_clamp() {
        assert_eq!(Offset::FromStart(2_000).shifted(15_000), Offset::FromStart(17_000));
        assert_eq!(Offset::FromStart(2_000).shifted(-15_000), Offset::FromStart(0));
        assert_eq!(Offset::FromEnd(20_000).shifted(-15_000), Offset::FromEnd(35_000));
        assert_eq!(Offset::FromEnd(5_000).shifted(15_000), Offset::FromEnd(0));
        assert_eq!(Offset::FromStart(u32::MAX).shifted(i32::MAX), Offset::FromStart(u32::MAX));
    }

    #[test]
    fn offsets_resolve_inside_the_entry() {
        let e = Entry { segment: 1, start_s: 10.0, end_s: 20.0, words: vec![], ends_section: false };
        assert_eq!(Offset::FromStart(2_500).resolve(&e), 12.5);
        assert_eq!(Offset::FromStart(60_000).resolve(&e), 20.0);
        assert_eq!(Offset::FromEnd(4_000).resolve(&e), 16.0);
        assert_eq!(Offset::FromEnd(60_000).resolve(&e), 10.0);
    }
}
