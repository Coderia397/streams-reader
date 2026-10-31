//! Seek planning. A target inside the buffered ranges is a plain `currentTime` change; anything else tears the
//! MediaSource down and rebuilds it from the target segment, because a `sequence`-mode SourceBuffer can only grow at
//! its end.

use std::collections::BTreeMap;

use crate::buffer::is_buffered;
use crate::timeline::{Entry, Timeline, secs_to_ms};

/// `previous` restarts the current segment when more than this much of it has played.
pub const RESTART_THRESHOLD_MS: u32 = 3_000;
/// Skip length for the ±15 s buttons and the Media Session seek actions.
pub const SKIP_MS: i32 = 15_000;

/// Where a build should put the playhead, relative to a segment whose length may not be known yet. An offset may point
/// past either end of its segment; [`target_step`] carries it into the neighbour once the lengths are known, so a skip
/// across a short segment lands where it was aimed instead of stopping at that segment's edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offset {
    /// Milliseconds after the segment's start; negative points into the segments before it.
    FromStart(i64),
    /// Milliseconds before the segment's end; negative points into the segments after it. Used when seeking back past
    /// the start of the timeline: the earlier segment's length is unknown until fetched.
    FromEnd(i64),
}

impl Offset {
    pub const START: Offset = Offset::FromStart(0);

    /// The same target moved by `delta_ms`, for a skip pressed while the build is still loading.
    pub fn shifted(self, delta_ms: i32) -> Offset {
        let delta = i64::from(delta_ms);
        match self {
            Offset::FromStart(ms) => Offset::FromStart(ms.saturating_add(delta)),
            Offset::FromEnd(ms) => Offset::FromEnd(ms.saturating_sub(delta)),
        }
    }

    /// Where this offset falls relative to `entry`.
    pub fn land(self, entry: &Entry) -> Landing {
        let duration_ms = i64::from(secs_to_ms(entry.duration_s()));
        let from_start = match self {
            Offset::FromStart(ms) => ms,
            Offset::FromEnd(ms) => duration_ms.saturating_sub(ms),
        };
        if from_start < 0 {
            Landing::Before(from_start.unsigned_abs())
        } else if from_start > duration_ms {
            Landing::After((from_start - duration_ms).unsigned_abs())
        } else {
            // Measured from the anchor the offset names, so FromEnd is exact against the real end.
            let time_s = match self {
                Offset::FromStart(ms) => entry.start_s + ms as f64 / 1000.0,
                Offset::FromEnd(ms) => entry.end_s - ms as f64 / 1000.0,
            };
            Landing::At(time_s.clamp(entry.start_s, entry.end_s))
        }
    }
}

/// Where an [`Offset`] falls relative to one segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Landing {
    /// Inside the segment, at this media time.
    At(f64),
    /// This many milliseconds before the segment's start.
    Before(u64),
    /// This many milliseconds after the segment's end.
    After(u64),
}

/// Next move for a build's pending target `(segment, offset)`, given what the build has appended so far.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TargetStep {
    /// The target segment is not appended yet, and it is the one this build fetches next.
    Wait,
    /// Put the playhead here; the build is ready.
    Place(f64),
    /// The target is really in this other segment; evaluate again with it.
    Retarget(u32, Offset),
    /// This build cannot reach the target soon (it is before the build's first segment, or past the segment it
    /// fetches next): start a new build there.
    Rebuild(u32, Offset),
}

/// Next move for the pending target of a build that started at segment `build_from`.
pub fn target_step(
    timeline: &Timeline,
    lengths: &Lengths,
    build_from: u32,
    segment: u32,
    offset: Offset,
    segment_count: u32,
) -> TargetStep {
    let to_i64 = |ms: u64| i64::try_from(ms).unwrap_or(i64::MAX);
    let Some(entry) = timeline.entry(segment) else {
        // Not in this build, but an earlier build may have measured it: a target that falls outside it moves on to
        // the neighbour now, so the build restarts (if it must) at the segment that holds the target, not one
        // segment at a time.
        if let Some((next, offset)) = carry(lengths, segment, offset, segment_count) {
            return TargetStep::Retarget(next, offset);
        }
        // A sequence-mode buffer cannot grow backwards, and fetching every segment up to a later target wastes the
        // listener's time: both restart the build at the target.
        return if segment < build_from || segment > next_fetch(timeline, build_from) {
            TargetStep::Rebuild(segment, offset)
        } else {
            TargetStep::Wait
        };
    };
    match offset.land(entry) {
        Landing::At(time_s) => TargetStep::Place(time_s),
        Landing::After(ms) => match next_target(segment, segment_count) {
            Some(next) => TargetStep::Retarget(next, Offset::FromStart(to_i64(ms))),
            // Past the end of the document: land on its end so the element finishes.
            None => TargetStep::Place(entry.end_s),
        },
        Landing::Before(ms) => match segment.checked_sub(1) {
            Some(previous) => TargetStep::Retarget(previous, Offset::FromEnd(to_i64(ms))),
            // Before the start of the document: clamp to it.
            None => TargetStep::Place(entry.start_s),
        },
    }
}

/// Segment the fetch loop of a build that started at `build_from` asks for next.
pub fn next_fetch(timeline: &Timeline, build_from: u32) -> u32 {
    timeline.last().map_or(build_from, |last| last.segment.saturating_add(1))
}

/// True when `segment` is the one the fetch loop of this build asks for next (in flight, or about to be): a command
/// that lands there keeps the build and waits for that append instead of restarting the fetch.
pub fn is_next_fetch(timeline: &Timeline, build_from: u32, segment: u32) -> bool {
    timeline.entry(segment).is_none() && segment == next_fetch(timeline, build_from)
}

/// Lengths in milliseconds of the segments appended by any build of the current document. A rebuild clears the
/// timeline but not this, so a skip back across segments played earlier resolves in one step.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lengths(BTreeMap<u32, u32>);

impl Lengths {
    pub fn record(&mut self, segment: u32, ms: u32) {
        self.0.insert(segment, ms);
    }

    pub fn get(&self, segment: u32) -> Option<u32> {
        self.0.get(&segment).copied()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Milliseconds from the start of `segment` to `offset`, when that does not depend on an unknown length.
    fn offset_from_start(&self, segment: u32, offset: Offset) -> Option<i64> {
        match (offset, self.get(segment)) {
            (Offset::FromStart(ms), _) => Some(ms),
            (Offset::FromEnd(ms), Some(len)) => Some(i64::from(len).saturating_sub(ms)),
            (Offset::FromEnd(_), None) => None,
        }
    }
}

/// One step of a target across a segment edge without fetching anything: the neighbour's target when `offset` is
/// known to point outside `segment` (before its start, or past its known length). None when it is inside, or when
/// that depends on a length not known yet. The ends of the document clamp.
pub fn carry(lengths: &Lengths, segment: u32, offset: Offset, segment_count: u32) -> Option<(u32, Offset)> {
    let from_start = lengths.offset_from_start(segment, offset)?;
    if from_start < 0 {
        // Before this segment's start whatever its length.
        return Some(match segment.checked_sub(1) {
            Some(previous) => (previous, Offset::FromEnd(from_start.saturating_neg())),
            None => (0, Offset::START),
        });
    }
    let len = i64::from(lengths.get(segment)?);
    if from_start <= len {
        return None;
    }
    Some(match next_target(segment, segment_count) {
        Some(next) => (next, Offset::FromStart(from_start - len)),
        // Past the end of the document: its end.
        None => (segment, Offset::FromEnd(0)),
    })
}

/// Carries a target across every edge the known lengths allow: the segment that holds it, or the first one whose
/// length is needed to tell.
pub fn resolve(lengths: &Lengths, segment: u32, offset: Offset, segment_count: u32) -> (u32, Offset) {
    let mut target = (segment, offset);
    // Each step moves one segment towards the target, or clamps at an end once.
    for _ in 0..segment_count.saturating_add(2) {
        match carry(lengths, target.0, target.1, segment_count) {
            Some(next) => target = next,
            None => break,
        }
    }
    target
}

/// Milliseconds into `segment` where `offset` lands, when that is known without fetching it: its start, or any point
/// inside a segment of known length.
pub fn exact_offset(lengths: &Lengths, segment: u32, offset: Offset) -> Option<u32> {
    if offset == Offset::START {
        return Some(0);
    }
    let len = lengths.get(segment)?;
    let from_start = lengths.offset_from_start(segment, offset)?;
    u32::try_from(from_start).ok().filter(|&ms| ms <= len)
}

/// A command pressed while a build's target still points at audio of unknown length (a skip carried past the edge of
/// a segment not fetched yet). It needs the exact (segment, offset) the target resolves to, so it is queued and applied
/// once that is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    Next,
    Previous,
    /// Skip by this many milliseconds.
    Shift(i32),
    /// Media Session seekto: this many milliseconds into the segment.
    Within(i64),
}

impl Then {
    /// The target this command gives from the exact position `(segment, offset_ms)`.
    pub fn apply(self, segment: u32, offset_ms: u32, segment_count: u32) -> (u32, Offset) {
        let here = (segment, Offset::FromStart(i64::from(offset_ms)));
        match self {
            // `next` on the last segment does nothing.
            Then::Next => next_target(segment, segment_count).map_or(here, |next| (next, Offset::START)),
            Then::Previous => (previous_target(segment, offset_ms), Offset::START),
            Then::Shift(delta_ms) => (segment, here.1.shifted(delta_ms)),
            Then::Within(ms) => (segment, Offset::FromStart(ms.max(0))),
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
                offset: Offset::FromEnd(i64::from(secs_to_ms(first.start_s - target_s))),
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
            SeekPlan::Rebuild { segment: next, offset: Offset::FromStart(i64::from(secs_to_ms(target_s - last.end_s))) }
        };
    }
    if is_buffered(ranges, target_s) {
        return SeekPlan::InBuffer { time_s: target_s };
    }
    // Inside the timeline but evicted.
    match timeline.locate(target_s) {
        Some(pos) => SeekPlan::Rebuild { segment: pos.segment, offset: Offset::FromStart(i64::from(pos.offset_ms)) },
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

/// Skip length for a Media Session seekbackward/seekforward that may carry its own offset in seconds; `default_ms` is
/// the listener's setting.
pub fn skip_ms(offset_s: f64, default_ms: i32) -> i32 {
    if offset_s.is_finite() && offset_s > 0.0 {
        (offset_s * 1000.0).round().min(f64::from(i32::MAX)) as i32
    } else {
        default_ms
    }
}

/// Skip length to store for a setting of `ms`; 0 means the default.
pub fn skip_setting(ms: u32) -> i32 {
    if ms == 0 { SKIP_MS } else { i32::try_from(ms).unwrap_or(i32::MAX) }
}

/// True when `time_s` is the end of the whole document (the end of its last segment, appended in this build).
pub fn is_document_end(timeline: &Timeline, time_s: f64, segment_count: u32) -> bool {
    timeline.last().is_some_and(|last| next_target(last.segment, segment_count).is_none() && time_s >= last.end_s)
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

    const NONE: Lengths = Lengths(BTreeMap::new());

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
        assert_eq!(skip_ms(f64::NAN, SKIP_MS), SKIP_MS);
        assert_eq!(skip_ms(0.0, SKIP_MS), SKIP_MS);
        assert_eq!(skip_ms(-5.0, 30_000), 30_000);
        assert_eq!(skip_ms(f64::NAN, 30_000), 30_000);
        assert_eq!(skip_ms(10.0, 30_000), 10_000);
        assert_eq!(skip_ms(1e12, SKIP_MS), i32::MAX);
        assert_eq!(skip_setting(0), SKIP_MS);
        assert_eq!(skip_setting(5_000), 5_000);
        assert_eq!(skip_setting(u32::MAX), i32::MAX);
    }

    #[test]
    fn pending_offsets_shift_without_clamping() {
        assert_eq!(Offset::FromStart(2_000).shifted(15_000), Offset::FromStart(17_000));
        assert_eq!(Offset::FromStart(2_000).shifted(-15_000), Offset::FromStart(-13_000));
        assert_eq!(Offset::FromEnd(20_000).shifted(-15_000), Offset::FromEnd(35_000));
        assert_eq!(Offset::FromEnd(5_000).shifted(15_000), Offset::FromEnd(-10_000));
        assert_eq!(Offset::FromStart(i64::MAX).shifted(i32::MAX), Offset::FromStart(i64::MAX));
    }

    #[test]
    fn offsets_land_inside_or_beside_the_entry() {
        let e = Entry { segment: 1, start_s: 10.0, end_s: 20.0, words: vec![], ends_section: false };
        assert_eq!(Offset::FromStart(2_500).land(&e), Landing::At(12.5));
        assert_eq!(Offset::FromStart(10_000).land(&e), Landing::At(20.0));
        assert_eq!(Offset::FromStart(60_000).land(&e), Landing::After(50_000));
        assert_eq!(Offset::FromStart(-1_500).land(&e), Landing::Before(1_500));
        assert_eq!(Offset::FromEnd(4_000).land(&e), Landing::At(16.0));
        assert_eq!(Offset::FromEnd(60_000).land(&e), Landing::Before(50_000));
        assert_eq!(Offset::FromEnd(-3_000).land(&e), Landing::After(3_000));
    }

    /// Segments 4 (2.7 s) and 5 (17.3 s) of a build that started at 4.
    fn short_first() -> Timeline {
        let mut t = Timeline::default();
        t.push(4, 0.0, 2.741, vec![], false);
        t.push(5, 2.741, 20.102, vec![], false);
        t
    }

    #[test]
    fn a_skip_past_a_short_segment_carries_into_the_next() {
        let mut t = Timeline::default();
        t.push(4, 0.0, 2.741, vec![], false);
        // +15 s pressed while segment 4 loaded: lands 12.259 s into 5, once 5 is in.
        assert_eq!(
            target_step(&t, &NONE, 4, 4, Offset::FromStart(15_000), 10),
            TargetStep::Retarget(5, Offset::FromStart(12_259))
        );
        assert_eq!(target_step(&t, &NONE, 4, 5, Offset::FromStart(12_259), 10), TargetStep::Wait);
        let t = short_first();
        match target_step(&t, &NONE, 4, 5, Offset::FromStart(12_259), 10) {
            TargetStep::Place(time_s) => assert!((time_s - 15.0).abs() < 1e-9, "{time_s}"),
            other => assert_eq!(other, TargetStep::Place(15.0)),
        }
    }

    #[test]
    fn a_skip_back_past_the_build_start_restarts_earlier() {
        let t = short_first();
        // 14 s before the end of 4 (2.7 s long) is 11.259 s before its start: in segment 3, from its end.
        assert_eq!(
            target_step(&t, &NONE, 4, 4, Offset::FromEnd(14_000), 10),
            TargetStep::Retarget(3, Offset::FromEnd(11_259))
        );
        assert_eq!(
            target_step(&t, &NONE, 4, 3, Offset::FromEnd(11_259), 10),
            TargetStep::Rebuild(3, Offset::FromEnd(11_259))
        );
        // Known to be earlier before anything was fetched: the segment before, then a rebuild there.
        let empty = Timeline::default();
        assert_eq!(
            target_step(&empty, &NONE, 7, 7, Offset::FromStart(-13_000), 10),
            TargetStep::Retarget(6, Offset::FromEnd(13_000))
        );
        assert_eq!(
            target_step(&empty, &NONE, 7, 6, Offset::FromEnd(13_000), 10),
            TargetStep::Rebuild(6, Offset::FromEnd(13_000))
        );
        assert_eq!(
            target_step(&empty, &NONE, 0, 0, Offset::FromStart(-13_000), 10),
            TargetStep::Retarget(0, Offset::START)
        );
        assert_eq!(target_step(&empty, &NONE, 7, 7, Offset::FromStart(500), 10), TargetStep::Wait);
    }

    #[test]
    fn a_skip_back_into_an_appended_segment_does_not_wait() {
        // Build from 0: segment 0 (2.7 s) is in, 1 is still rendering. The target 1@-12.3 s is in 0.
        let mut t = Timeline::default();
        t.push(0, 0.0, 2.741, vec![], false);
        assert_eq!(
            target_step(&t, &NONE, 0, 1, Offset::FromStart(-2_000), 10),
            TargetStep::Retarget(0, Offset::FromEnd(2_000))
        );
        match target_step(&t, &NONE, 0, 0, Offset::FromEnd(2_000), 10) {
            TargetStep::Place(time_s) => assert!((time_s - 0.741).abs() < 1e-9, "{time_s}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(target_step(&t, &NONE, 0, 0, Offset::FromEnd(12_300), 10), TargetStep::Place(0.0));
    }

    #[test]
    fn a_target_beyond_the_next_fetch_rebuilds_there() {
        let t = short_first();
        assert_eq!(target_step(&t, &NONE, 4, 6, Offset::START, 10), TargetStep::Wait);
        assert_eq!(target_step(&t, &NONE, 4, 7, Offset::START, 10), TargetStep::Rebuild(7, Offset::START));
        assert_eq!(
            target_step(&Timeline::default(), &NONE, 4, 5, Offset::START, 10),
            TargetStep::Rebuild(5, Offset::START)
        );
        assert_eq!(
            target_step(&Timeline::default(), &NONE, 4, 3, Offset::START, 10),
            TargetStep::Rebuild(3, Offset::START)
        );
    }

    #[test]
    fn the_document_ends_clamp() {
        let t = short_first();
        assert_eq!(target_step(&t, &NONE, 4, 5, Offset::FromStart(99_000), 6), TargetStep::Place(20.102));
        let mut first = Timeline::default();
        first.push(0, 0.0, 2.741, vec![], false);
        assert_eq!(target_step(&first, &NONE, 0, 0, Offset::FromEnd(9_000), 6), TargetStep::Place(0.0));
    }

    #[test]
    fn queued_commands_apply_to_the_exact_position() {
        // 9@5.5 s: previous restarts 9, next goes to 10.
        assert_eq!(Then::Previous.apply(9, 5_500, 26), (9, Offset::START));
        assert_eq!(Then::Previous.apply(9, 2_000, 26), (8, Offset::START));
        assert_eq!(Then::Next.apply(17, 11_800, 26), (18, Offset::START));
        assert_eq!(Then::Next.apply(25, 1_000, 26), (25, Offset::FromStart(1_000)), "next on the last segment");
        assert_eq!(Then::Shift(-15_000).apply(9, 5_500, 26), (9, Offset::FromStart(-9_500)));
        assert_eq!(Then::Within(8_000).apply(5, 0, 26), (5, Offset::FromStart(8_000)));
        assert_eq!(Then::Within(-1).apply(5, 300, 26), (5, Offset::START));
    }

    #[test]
    fn document_end_is_the_end_of_the_last_segment() {
        let t = short_first();
        assert!(is_document_end(&t, 20.102, 6));
        assert!(!is_document_end(&t, 20.0, 6));
        assert!(!is_document_end(&t, 20.102, 7), "more segments follow");
        assert!(!is_document_end(&Timeline::default(), 0.0, 6));
    }

    /// Lengths of 14 (20 s), 15 (18.1 s) and 16 (3.26 s), as a build from 14 measured them.
    fn measured() -> Lengths {
        let mut l = Lengths::default();
        l.record(14, 20_000);
        l.record(15, 18_100);
        l.record(16, 3_260);
        l
    }

    #[test]
    fn known_lengths_carry_a_skip_back_to_the_segment_that_holds_it() {
        let l = measured();
        // -15 s from 1.3 s into 17: 13.7 s before the end of 16 (3.26 s) is 10.44 s before the end of 15.
        assert_eq!(resolve(&l, 16, Offset::FromEnd(13_700), 26), (15, Offset::FromEnd(10_440)));
        // -34 s from 1 s into 17: through 16 and 15 into 14.
        assert_eq!(resolve(&l, 16, Offset::FromEnd(33_000), 26), (14, Offset::FromEnd(11_640)));
        // ...and on into 13, whose length is unknown: the build starts there and measures it.
        assert_eq!(resolve(&l, 16, Offset::FromEnd(64_000), 26), (13, Offset::FromEnd(22_640)));
        // Forward through known lengths, and past the document's end.
        assert_eq!(resolve(&l, 14, Offset::FromStart(40_000), 26), (16, Offset::FromStart(1_900)));
        assert_eq!(resolve(&l, 15, Offset::FromStart(30_000), 26), (17, Offset::FromStart(8_640)), "17 unknown");
        assert_eq!(resolve(&l, 14, Offset::FromStart(99_000), 17), (16, Offset::FromEnd(0)));
        // Inside, or depending on an unknown length: unchanged.
        assert_eq!(resolve(&l, 15, Offset::FromEnd(2_000), 26), (15, Offset::FromEnd(2_000)));
        assert_eq!(resolve(&l, 20, Offset::FromEnd(2_000), 26), (20, Offset::FromEnd(2_000)));
        assert_eq!(resolve(&Lengths::default(), 3, Offset::FromStart(-1), 26), (2, Offset::FromEnd(1)));
        assert_eq!(resolve(&l, 0, Offset::FromStart(-5_000), 26), (0, Offset::START));
    }

    #[test]
    fn a_target_is_exact_once_its_segment_length_is_known() {
        let l = measured();
        assert_eq!(exact_offset(&l, 20, Offset::START), Some(0));
        assert_eq!(exact_offset(&l, 20, Offset::FromStart(500)), None);
        assert_eq!(exact_offset(&l, 15, Offset::FromEnd(10_440)), Some(7_660));
        assert_eq!(exact_offset(&l, 15, Offset::FromStart(18_100)), Some(18_100));
        assert_eq!(exact_offset(&l, 15, Offset::FromStart(18_101)), None);
        assert_eq!(exact_offset(&l, 15, Offset::FromEnd(20_000)), None);
    }

    #[test]
    fn a_skip_back_before_the_build_rebuilds_once_where_the_lengths_say() {
        // Build from 17 (12 s in); 14..=16 measured by an earlier build.
        let l = measured();
        let mut t = Timeline::default();
        t.push(17, 0.0, 12.0, vec![], false);
        assert_eq!(
            target_step(&t, &l, 17, 16, Offset::FromEnd(13_700), 26),
            TargetStep::Retarget(15, Offset::FromEnd(10_440))
        );
        assert_eq!(
            target_step(&t, &l, 17, 15, Offset::FromEnd(10_440), 26),
            TargetStep::Rebuild(15, Offset::FromEnd(10_440))
        );
    }

    #[test]
    fn the_segment_in_flight_keeps_the_build() {
        let t = short_first();
        assert!(is_next_fetch(&t, 4, 6));
        assert!(!is_next_fetch(&t, 4, 5), "appended");
        assert!(!is_next_fetch(&t, 4, 7), "beyond the next fetch");
        assert!(!is_next_fetch(&t, 4, 3));
        assert!(is_next_fetch(&Timeline::default(), 9, 9), "nothing appended yet: the first fetch");
        assert!(!is_next_fetch(&Timeline::default(), 9, 10));
        // A skip past the buffered end plans a rebuild at the next segment, which is the one in flight.
        assert_eq!(
            plan_seek_to(&t, &[(0.0, 20.102)], 25.0, 10),
            SeekPlan::Rebuild { segment: 6, offset: Offset::FromStart(4_898) }
        );
    }
}
