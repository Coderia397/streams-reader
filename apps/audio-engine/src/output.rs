//! cpal output (0.18.2): CoreAudio on macOS, WASAPI on Windows, ALSA on Linux (CI needs `libasound2-dev`).
//!
//! - Opens the default device with `BufferSize::Fixed(~10 ms)` (ALSA's default buffer can be very large).
//! - Prefers 48 kHz f32; otherwise the decode thread resamples with `rubato` before the ring (never in the callback).
//! - Mono is copied to every device channel.
//! - The callback pops from the `rtrb` consumer; on underrun it writes silence, counts the missing frames and the
//!   engine reports `Buffering`.
//! - Frames played are published through an `AtomicU64` for the [`crate::clock`].
//! - Stream errors (device removed, default device changed) are sent to the decode thread, which pauses and
//!   re-opens on the new default device.
//!
//! Known limit (cpal 0.18 README): CoreAudio support needs macOS 14.2+. The owner's Mac runs macOS 26; the release
//! sets `minimumSystemVersion` to 14.2 so older Macs get a clear refusal instead of silence.
//!
//! TODO(phase 2): `Output::open(consumer, played: Arc<AtomicU64>, errors: Sender<String>) -> Result<Output, AudioError>`.

/// Ring capacity in samples for [`crate::RING_MS`] at the engine rate.
pub fn ring_capacity() -> usize {
    crate::source::samples_for_ms(crate::RING_MS)
}
