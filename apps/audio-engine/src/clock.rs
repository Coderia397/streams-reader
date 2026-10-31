//! Output-frame clock. The decode thread tags each chunk it pushes into the ring with where it came from
//! (segment, source position, rate); the callback counts frames played. Together they answer "which word is being
//! spoken now" at any rate without asking the device for timestamps.
//!
//! Pure and allocation-free on the reading side, so it is unit-tested on the host.

/// One chunk written to the ring: `frames` output frames that cover source audio starting at `source_ms` of
/// `segment`, played at `rate`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tag {
    pub first_frame: u64,
    pub frames: u32,
    pub segment: u32,
    pub source_ms: f64,
    pub rate: f32,
}

/// Where the listener is, in source time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub segment: u32,
    pub segment_pos_ms: u32,
}

/// Fixed-capacity history of tags (the ring holds ≈400 ms, so a few dozen tags are enough).
pub struct Clock {
    tags: Vec<Tag>,
    capacity: usize,
    rate_hz: f64,
}

impl Clock {
    pub fn new(rate_hz: u32, capacity: usize) -> Self {
        Self { tags: Vec::with_capacity(capacity), capacity, rate_hz: f64::from(rate_hz) }
    }

    /// Record a chunk (decode thread). Oldest tags fall off.
    pub fn push(&mut self, tag: Tag) {
        if self.tags.len() == self.capacity {
            self.tags.remove(0);
        }
        self.tags.push(tag);
    }

    /// Forget everything (seek, new document).
    pub fn clear(&mut self) {
        self.tags.clear();
    }

    /// Source position of output frame `played` (frames the callback has consumed since the stream started).
    pub fn position(&self, played: u64) -> Option<Position> {
        let tag = self.tags.iter().rev().find(|t| t.first_frame <= played)?;
        let into = (played - tag.first_frame).min(u64::from(tag.frames));
        // Frames are counts well below 2^52, so the f64 conversion is exact.
        #[allow(clippy::cast_precision_loss)]
        let out_ms = into as f64 * 1000.0 / self.rate_hz;
        let src_ms = tag.source_ms + out_ms * f64::from(tag.rate);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let segment_pos_ms = src_ms.max(0.0).round() as u32;
        Some(Position { segment: tag.segment, segment_pos_ms })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_output_frames_to_source_time_at_double_speed() {
        let mut c = Clock::new(48_000, 8);
        c.push(Tag { first_frame: 0, frames: 48_000, segment: 3, source_ms: 1_000.0, rate: 2.0 });
        // Half a second of output at 2x = one second of source.
        assert_eq!(c.position(24_000), Some(Position { segment: 3, segment_pos_ms: 2_000 }));
    }

    #[test]
    fn picks_the_latest_tag_at_a_segment_boundary() {
        let mut c = Clock::new(48_000, 8);
        c.push(Tag { first_frame: 0, frames: 4_800, segment: 0, source_ms: 900.0, rate: 1.0 });
        c.push(Tag { first_frame: 4_800, frames: 4_800, segment: 1, source_ms: 0.0, rate: 1.0 });
        assert_eq!(c.position(4_800), Some(Position { segment: 1, segment_pos_ms: 0 }));
        assert_eq!(c.position(2_400), Some(Position { segment: 0, segment_pos_ms: 950 }));
    }

    #[test]
    fn nothing_before_the_first_tag() {
        let c = Clock::new(48_000, 4);
        assert_eq!(c.position(10), None);
    }
}
