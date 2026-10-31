//! Pitch-preserving rate change for speech.
//!
//! Choice (notes/18, verified on crates.io 2026-10-30): `libsonic` 0.1.0, a pure-Rust port of Bill Cox's Sonic,
//! the algorithm built for speeding up speech (pitch-synchronous overlap-add, latency of a few pitch periods, very
//! low CPU). Android's ExoPlayer uses the same algorithm (`SonicAudioProcessor`), so both platforms sound alike.
//! Fallback: `signalsmith-stretch` (feature `signalsmith`, spectral, ~100 ms latency, more CPU) if Sonic sounds
//! rough at 0.5x. Rejected: rubberband (GPL), bungee bindings (young, cmake), tdpsola (AGPL), timestretch (tuned for
//! music, untested on speech).

/// A mono time-stretcher. `push` input at the engine rate, `pull` output at the same rate.
pub trait Stretcher: Send {
    fn set_rate(&mut self, rate: f32);
    fn push(&mut self, input: &[f32]);
    /// Up to `max` frames of output (may return fewer, or none until enough input arrived).
    fn pull(&mut self, max: usize) -> Vec<f32>;
    /// End of stream (seek, close): emit what is buffered.
    fn flush(&mut self);
    /// Drop buffered audio without emitting it (seek).
    fn reset(&mut self);
}

/// Sonic, through `libsonic`.
pub struct Sonic {
    stream: libsonic::SonicStream,
    sample_rate: u32,
    rate: f32,
}

impl Sonic {
    pub fn new(sample_rate: u32) -> Option<Self> {
        let stream = libsonic::SonicStream::new(sample_rate, 1).ok()?;
        Some(Self { stream, sample_rate, rate: 1.0 })
    }
}

impl Stretcher for Sonic {
    fn set_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(crate::MIN_RATE, crate::MAX_RATE);
        self.stream.set_speed(self.rate);
    }
    fn push(&mut self, input: &[f32]) {
        self.stream.write_f32(input);
    }
    fn pull(&mut self, max: usize) -> Vec<f32> {
        self.stream.read_f32(max)
    }
    fn flush(&mut self) {
        self.stream.flush();
    }
    fn reset(&mut self) {
        if let Ok(mut fresh) = libsonic::SonicStream::new(self.sample_rate, 1) {
            fresh.set_speed(self.rate);
            self.stream = fresh;
        }
    }
}

// TODO(phase 2): `Signalsmith` behind `#[cfg(feature = "signalsmith")]` with the same trait, and a listening A/B at
// 0.75x, 1.5x, 2x, 3x on one Kokoro and one Ming chapter (acceptance check in notes/18).
