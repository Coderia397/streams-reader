//! Buffer window decisions: when to fetch the next segment, what to evict when the browser's SourceBuffer quota is
//! full, and how long to back off after a failed fetch.

/// Keep at least this much audio buffered ahead of the playhead (notes/09: ≥ 3 min), so a phone with the screen off
/// survives a slow render or a network gap.
pub const AHEAD_TARGET_S: f64 = 180.0;
/// On a quota error, keep this much already played audio behind the playhead so a short skip back stays instant.
pub const KEEP_BEHIND_S: f64 = 60.0;
/// Ranges closer than this are treated as one: sequence-mode appends can leave sub-frame holes between segments.
pub const GAP_TOLERANCE_S: f64 = 0.1;
/// Evicting less than this frees too little to make the retry worthwhile; wait for playback to move on instead.
const MIN_EVICT_S: f64 = 1.0;

/// What the fetch loop should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchStep {
    /// Ask the provider for this segment and append it.
    Fetch(u32),
    /// Enough is buffered; check again later.
    Wait,
    /// Every segment is appended; end the stream.
    Finish,
}

pub fn next_step(ahead_s: f64, next_segment: u32, segment_count: u32) -> FetchStep {
    if next_segment >= segment_count {
        FetchStep::Finish
    } else if ahead_s >= AHEAD_TARGET_S {
        FetchStep::Wait
    } else {
        FetchStep::Fetch(next_segment)
    }
}

/// What to do when an append fails with `QuotaExceededError`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Eviction {
    /// Remove this played range, then retry the append.
    Remove { start_s: f64, end_s: f64 },
    /// Nothing old enough to drop; retry after playback has moved on.
    WaitForPlayback,
}

/// Everything buffered more than [`KEEP_BEHIND_S`] behind `current_s` may go. `ranges` are the SourceBuffer's
/// buffered ranges, sorted.
pub fn eviction(ranges: &[(f64, f64)], current_s: f64) -> Eviction {
    let cutoff = current_s - KEEP_BEHIND_S;
    match ranges.first() {
        Some(&(start_s, _)) if cutoff - start_s >= MIN_EVICT_S => Eviction::Remove { start_s, end_s: cutoff },
        _ => Eviction::WaitForPlayback,
    }
}

/// True when `t` is inside a buffered range (with [`GAP_TOLERANCE_S`] slack at both edges).
pub fn is_buffered(ranges: &[(f64, f64)], t: f64) -> bool {
    ranges.iter().any(|&(s, e)| t >= s - GAP_TOLERANCE_S && t <= e + GAP_TOLERANCE_S)
}

/// Seconds playable from `t` without a stall: the end of the range holding `t`, following ranges that touch it.
pub fn ahead_of(ranges: &[(f64, f64)], t: f64) -> f64 {
    let mut end: Option<f64> = None;
    for &(s, e) in ranges {
        match end {
            None if t >= s - GAP_TOLERANCE_S && t <= e + GAP_TOLERANCE_S => end = Some(e),
            Some(prev) if s <= prev + GAP_TOLERANCE_S => end = Some(prev.max(e)),
            Some(_) => break,
            None => {}
        }
    }
    end.map_or(0.0, |e| (e - t).max(0.0))
}

/// Backoff before retrying a failed fetch: 2 s, 4 s, 8 s, 16 s, then 30 s.
pub fn retry_delay_ms(attempt: u32) -> u32 {
    2_000u32.saturating_mul(1u32 << attempt.min(4)).min(30_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_until_three_minutes_ahead() {
        assert_eq!(next_step(0.0, 0, 10), FetchStep::Fetch(0));
        assert_eq!(next_step(179.9, 3, 10), FetchStep::Fetch(3));
        assert_eq!(next_step(180.0, 3, 10), FetchStep::Wait);
        assert_eq!(next_step(500.0, 10, 10), FetchStep::Finish);
        assert_eq!(next_step(0.0, 11, 10), FetchStep::Finish);
        assert_eq!(next_step(0.0, 0, 0), FetchStep::Finish);
    }

    #[test]
    fn evicts_played_audio_older_than_a_minute() {
        assert_eq!(eviction(&[(0.0, 400.0)], 200.0), Eviction::Remove { start_s: 0.0, end_s: 140.0 });
        assert_eq!(eviction(&[(30.0, 50.0), (55.0, 400.0)], 200.0), Eviction::Remove { start_s: 30.0, end_s: 140.0 });
    }

    #[test]
    fn waits_when_nothing_old_enough() {
        assert_eq!(eviction(&[(0.0, 400.0)], 60.5), Eviction::WaitForPlayback);
        assert_eq!(eviction(&[(10.0, 400.0)], 30.0), Eviction::WaitForPlayback);
        assert_eq!(eviction(&[], 300.0), Eviction::WaitForPlayback);
    }

    #[test]
    fn buffered_check_has_slack_at_the_edges() {
        let ranges = [(0.0, 10.0), (20.0, 30.0)];
        assert!(is_buffered(&ranges, 5.0));
        assert!(is_buffered(&ranges, 10.05));
        assert!(is_buffered(&ranges, 19.95));
        assert!(!is_buffered(&ranges, 15.0));
        assert!(!is_buffered(&ranges, 31.0));
        assert!(!is_buffered(&[], 0.0));
    }

    #[test]
    fn ahead_follows_touching_ranges_only() {
        let ranges = [(0.0, 10.0), (10.05, 20.0), (25.0, 40.0)];
        assert!((ahead_of(&ranges, 4.0) - 16.0).abs() < 1e-9);
        assert!((ahead_of(&ranges, 30.0) - 10.0).abs() < 1e-9);
        assert_eq!(ahead_of(&ranges, 22.0), 0.0);
        assert_eq!(ahead_of(&ranges, 50.0), 0.0);
        assert_eq!(ahead_of(&[], 1.0), 0.0);
    }

    #[test]
    fn backoff_doubles_then_caps() {
        let delays: Vec<u32> = (0..7).map(retry_delay_ms).collect();
        assert_eq!(delays, [2_000, 4_000, 8_000, 16_000, 30_000, 30_000, 30_000]);
        assert_eq!(retry_delay_ms(u32::MAX), 30_000);
    }
}
