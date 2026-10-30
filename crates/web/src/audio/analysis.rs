//! Checks on a recorded line before it becomes a voice reference. A clipped or noisy reference is cloned faithfully,
//! clipping and hiss included, so problems are caught per line while re-recording is cheap.

/// Peak above this is treated as clipping.
pub const CLIP_DBFS: f32 = -1.0;
/// Background noise above this is audible in a cloned voice.
pub const NOISE_DBFS: f32 = -50.0;
/// Speech below this is too far from the microphone.
pub const QUIET_DBFS: f32 = -32.0;
pub const MIN_LINE_MS: u32 = 1_200;
const FRAME_MS: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelReport {
    pub duration_ms: u32,
    pub peak_dbfs: f32,
    /// 10th percentile of 50 ms frame loudness: the room between words.
    pub noise_dbfs: f32,
    /// 90th percentile of 50 ms frame loudness: the voice.
    pub speech_dbfs: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    Clipping,
    Noisy,
    TooQuiet,
    TooShort,
}

impl Issue {
    pub fn message(self) -> &'static str {
        match self {
            Issue::Clipping => "Too loud: it distorted. Move back a little and read it again.",
            Issue::Noisy => "There's background noise. Find a quieter spot if you can.",
            Issue::TooQuiet => "Very quiet. Hold the phone a little closer.",
            Issue::TooShort => "That was very short. Read the whole line.",
        }
    }

    /// Clipping and silence make a reference unusable; noise is a warning.
    pub fn blocking(self) -> bool {
        !matches!(self, Issue::Noisy)
    }
}

pub fn dbfs(amplitude: f32) -> f32 {
    20.0 * amplitude.abs().max(1e-6).log10()
}

fn percentile(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return -120.0;
    }
    let i = ((sorted.len() - 1) as f32 * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

pub fn analyse(samples: &[f32], sample_rate: u32) -> LevelReport {
    let duration_ms = if sample_rate == 0 { 0 } else { (samples.len() as u64 * 1000 / u64::from(sample_rate)) as u32 };
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let frame = ((sample_rate * FRAME_MS / 1000) as usize).max(1);
    let mut frames: Vec<f32> = samples
        .chunks(frame)
        .filter(|c| c.len() * 2 >= frame)
        .map(|c| {
            let rms = (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt();
            dbfs(rms)
        })
        .collect();
    frames.sort_by(f32::total_cmp);
    LevelReport {
        duration_ms,
        peak_dbfs: dbfs(peak),
        noise_dbfs: percentile(&frames, 0.10),
        speech_dbfs: percentile(&frames, 0.90),
    }
}

pub fn issues(r: &LevelReport) -> Vec<Issue> {
    let mut out = Vec::new();
    if r.duration_ms < MIN_LINE_MS {
        out.push(Issue::TooShort);
    }
    if r.peak_dbfs > CLIP_DBFS {
        out.push(Issue::Clipping);
    }
    if r.speech_dbfs < QUIET_DBFS {
        out.push(Issue::TooQuiet);
    }
    if r.noise_dbfs > NOISE_DBFS {
        out.push(Issue::Noisy);
    }
    out
}

/// Live meter position 0.0–1.0 for an RMS level (−60 dBFS → 0, 0 dBFS → 1).
pub fn meter(rms: f32) -> f32 {
    ((dbfs(rms) + 60.0) / 60.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 8_000;

    /// Half a second of near-silence, then a second of a sine at `amp`.
    fn take(amp: f32, floor: f32) -> Vec<f32> {
        let mut s: Vec<f32> = (0..RATE / 2).map(|i| if i % 2 == 0 { floor } else { -floor }).collect();
        s.extend((0..RATE).map(|i| amp * (i as f32 * 0.3).sin()));
        s
    }

    #[test]
    fn a_clean_take_has_no_issues() {
        let r = analyse(&take(0.3, 0.0005), RATE);
        assert_eq!(r.duration_ms, 1500);
        assert!(r.peak_dbfs < CLIP_DBFS);
        assert!(r.noise_dbfs < NOISE_DBFS, "noise {}", r.noise_dbfs);
        assert!(r.speech_dbfs > QUIET_DBFS, "speech {}", r.speech_dbfs);
        assert!(issues(&r).is_empty());
    }

    #[test]
    fn flags_clipping_noise_quiet_and_short() {
        assert!(issues(&analyse(&take(1.0, 0.0005), RATE)).contains(&Issue::Clipping));
        assert!(issues(&analyse(&take(0.3, 0.02), RATE)).contains(&Issue::Noisy));
        assert!(issues(&analyse(&take(0.005, 0.0001), RATE)).contains(&Issue::TooQuiet));
        assert!(issues(&analyse(&[0.1; 800], RATE)).contains(&Issue::TooShort));
        assert!(Issue::Clipping.blocking());
        assert!(!Issue::Noisy.blocking());
    }

    #[test]
    fn meter_and_dbfs() {
        assert!((dbfs(1.0)).abs() < 1e-6);
        assert!((dbfs(0.1) + 20.0).abs() < 1e-4);
        assert_eq!(meter(0.0), 0.0);
        assert_eq!(meter(1.0), 1.0);
        assert!((meter(0.031_622_8) - 0.5).abs() < 1e-3);
        assert_eq!(analyse(&[], RATE).noise_dbfs, -120.0);
    }
}
