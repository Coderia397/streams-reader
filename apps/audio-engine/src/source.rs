//! One segment file → mono f32 PCM at 48 kHz.
//!
//! - Demux: `symphonia` 0.6.1 `mkv` reader. It maps `A_OPUS` to the Opus codec id, passes the OpusHead as codec
//!   extra data and applies CodecDelay to timestamps; it does not trim DiscardPadding (end padding).
//! - Decode: `symphonia-adapter-libopus` 0.3.0 (libopus 1.6.1), which drops the OpusHead pre-skip.
//! - End trim: decoded samples beyond `SegmentFile::duration_ms` (from the render sidecar) are dropped, so the next
//!   segment starts exactly where this one ends. Our segments end in an appended pause (250/600/1200 ms), so an
//!   off-by-a-few-ms trim is inaudible; it only matters for the clock.
//! - Seek inside a file: decode from the start and discard (segments are 5–40 s; decoding Opus is ~100x realtime),
//!   which is simpler and exact compared with container seeks.
//!
//! TODO(phase 2): implement `decode_into` with `symphonia::default::formats` replaced by the mkv reader alone and
//! the adapter's decoder registered in a `CodecRegistry`; unit-test with the 3 s fixture
//! `crates/player/harness` already uses (copy, do not reference another team's files at runtime).

use crate::{AudioError, SegmentFile};

/// Samples of one segment from `start_ms` to its end. Allocates; runs on the decode thread only.
pub fn decode(file: &SegmentFile, start_ms: u32) -> Result<Vec<f32>, AudioError> {
    let _ = start_ms;
    Err(AudioError::Decode { path: file.path.clone(), message: "decoder not built yet (phase 2)".into() })
}

/// Number of samples a duration covers at the engine rate.
pub fn samples_for_ms(ms: u32) -> usize {
    (u64::from(ms) * u64::from(crate::ENGINE_RATE) / 1000) as usize
}

#[cfg(test)]
mod tests {
    #[test]
    fn samples_for_ms_at_48k() {
        assert_eq!(super::samples_for_ms(1000), 48_000);
        assert_eq!(super::samples_for_ms(20), 960);
    }
}
