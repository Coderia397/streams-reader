//! The media timeline of one build: every segment appended since the last (re)build, in order, with its exact start
//! and end on the media element's clock. With a `sequence`-mode SourceBuffer the browser lays segments end to end, so
//! the timeline is contiguous; the fallback path builds the same shape from the reported durations.

use nolabs_core::proto::api::WordTiming;

/// One appended segment.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub segment: u32,
    /// Start on the media clock, in seconds.
    pub start_s: f64,
    /// End on the media clock, in seconds (equals the next entry's start).
    pub end_s: f64,
    pub words: Vec<WordTiming>,
    pub ends_section: bool,
}

impl Entry {
    pub fn duration_s(&self) -> f64 {
        (self.end_s - self.start_s).max(0.0)
    }

    /// Offset of `t` into this entry in whole milliseconds, clamped to the entry.
    pub fn offset_ms(&self, t: f64) -> u32 {
        secs_to_ms((t - self.start_s).clamp(0.0, self.duration_s()))
    }

    /// `(duration, position)` in seconds of `t` inside this entry, for the Media Session position state, which shows
    /// one segment at a time.
    pub fn window(&self, t: f64) -> (f64, f64) {
        let duration = self.duration_s();
        (duration, (t - self.start_s).clamp(0.0, duration))
    }
}

/// Where a media time falls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Index into [`Timeline::entries`].
    pub index: usize,
    pub segment: u32,
    pub offset_ms: u32,
    /// Index into the entry's word timings of the word being spoken (the last one that has started).
    pub word: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Timeline {
    entries: Vec<Entry>,
}

impl Timeline {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn first(&self) -> Option<&Entry> {
        self.entries.first()
    }

    pub fn last(&self) -> Option<&Entry> {
        self.entries.last()
    }

    /// Media time where the next append lands (0 for an empty timeline).
    pub fn end_s(&self) -> f64 {
        self.entries.last().map_or(0.0, |e| e.end_s)
    }

    /// Appends the next segment. `end_s` below `start_s` (a zero-length append) is clamped so the timeline stays
    /// ordered.
    pub fn push(&mut self, segment: u32, start_s: f64, end_s: f64, words: Vec<WordTiming>, ends_section: bool) {
        let end_s = end_s.max(start_s);
        self.entries.push(Entry { segment, start_s, end_s, words, ends_section });
    }

    pub fn entry(&self, segment: u32) -> Option<&Entry> {
        self.entries.iter().find(|e| e.segment == segment)
    }

    /// Maps a media time to (segment, offset, word). Times before the first entry map to its start and times at or
    /// past the end map to the end of the last entry, so the end of the stream still reports the last segment.
    pub fn locate(&self, t: f64) -> Option<Position> {
        let last = self.entries.len().checked_sub(1)?;
        let index = self.entries.partition_point(|e| e.end_s <= t).min(last);
        let entry = &self.entries[index];
        let offset_ms = entry.offset_ms(t);
        Some(Position { index, segment: entry.segment, offset_ms, word: word_at(&entry.words, offset_ms) })
    }
}

/// The word being spoken at `offset_ms`: the last word whose start is at or before it. Timings are sorted by start
/// (the engines emit them in reading order). None before the first word.
pub fn word_at(words: &[WordTiming], offset_ms: u32) -> Option<usize> {
    words.partition_point(|w| w.0 <= offset_ms).checked_sub(1)
}

/// Seconds to whole milliseconds, saturating; negative and NaN become 0.
pub fn secs_to_ms(s: f64) -> u32 {
    let ms = (s * 1000.0).round();
    if ms.is_nan() || ms <= 0.0 {
        0
    } else if ms >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        ms as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(start: u32, end: u32) -> WordTiming {
        WordTiming(start, end, 0, 0)
    }

    fn sample() -> Timeline {
        let mut t = Timeline::default();
        t.push(4, 0.0, 10.0, vec![w(0, 300), w(400, 900), w(1000, 1500)], false);
        t.push(5, 10.0, 25.5, vec![], true);
        t.push(6, 25.5, 30.0, vec![w(200, 600)], false);
        t
    }

    #[test]
    fn locate_maps_time_to_segment_offset_and_word() {
        let t = sample();
        assert_eq!(t.locate(0.0), Some(Position { index: 0, segment: 4, offset_ms: 0, word: Some(0) }));
        assert_eq!(t.locate(0.95), Some(Position { index: 0, segment: 4, offset_ms: 950, word: Some(1) }));
        assert_eq!(t.locate(12.0), Some(Position { index: 1, segment: 5, offset_ms: 2000, word: None }));
        assert_eq!(t.locate(25.6).map(|p| (p.segment, p.offset_ms, p.word)), Some((6, 100, None)));
        assert_eq!(t.locate(25.7).map(|p| p.word), Some(Some(0)));
    }

    #[test]
    fn boundaries_belong_to_the_next_entry() {
        let t = sample();
        assert_eq!(t.locate(10.0).map(|p| (p.segment, p.offset_ms)), Some((5, 0)));
        assert_eq!(t.locate(9.9999).map(|p| p.segment), Some(4));
    }

    #[test]
    fn locate_clamps_outside_the_timeline() {
        let t = sample();
        assert_eq!(t.locate(-3.0).map(|p| (p.segment, p.offset_ms)), Some((4, 0)));
        assert_eq!(t.locate(30.0).map(|p| (p.segment, p.offset_ms)), Some((6, 4500)));
        assert_eq!(t.locate(99.0).map(|p| (p.index, p.offset_ms)), Some((2, 4500)));
        assert_eq!(Timeline::default().locate(1.0), None);
    }

    #[test]
    fn word_lookup_uses_the_last_started_word() {
        let words = [w(100, 200), w(250, 400), w(400, 800)];
        assert_eq!(word_at(&words, 0), None);
        assert_eq!(word_at(&words, 99), None);
        assert_eq!(word_at(&words, 100), Some(0));
        assert_eq!(word_at(&words, 220), Some(0), "in the gap between words the previous one stays lit");
        assert_eq!(word_at(&words, 400), Some(2));
        assert_eq!(word_at(&words, 10_000), Some(2));
        assert_eq!(word_at(&[], 5), None);
    }

    #[test]
    fn entries_lookup_and_end() {
        let t = sample();
        assert_eq!(t.end_s(), 30.0);
        assert_eq!(t.entry(5).map(|e| e.start_s), Some(10.0));
        assert_eq!(t.entry(9), None);
        assert_eq!(t.first().map(|e| e.segment), Some(4));
        assert_eq!(t.last().map(|e| e.segment), Some(6));
        assert_eq!(Timeline::default().end_s(), 0.0);
    }

    #[test]
    fn zero_length_append_is_clamped() {
        let mut t = Timeline::default();
        t.push(0, 5.0, 4.0, vec![], false);
        assert_eq!(t.last().map(Entry::duration_s), Some(0.0));
        assert_eq!(t.locate(5.0).map(|p| p.offset_ms), Some(0));
    }

    #[test]
    fn window_is_relative_to_the_segment() {
        let t = sample();
        let e = &t.entries()[1];
        assert_eq!(e.window(12.0), (15.5, 2.0));
        assert_eq!(e.window(40.0), (15.5, 15.5));
        assert_eq!(e.window(1.0), (15.5, 0.0));
    }

    #[test]
    fn seconds_to_milliseconds_saturates() {
        assert_eq!(secs_to_ms(1.2345), 1235);
        assert_eq!(secs_to_ms(-1.0), 0);
        assert_eq!(secs_to_ms(f64::NAN), 0);
        assert_eq!(secs_to_ms(1e12), u32::MAX);
    }
}
