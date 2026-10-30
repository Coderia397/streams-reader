//! 16-bit PCM mono WAV, the format the Space's `prepare_reference` reads. Lines are joined with short silences so the
//! Space can cut a sentence-aligned span.

/// Silence between recorded lines.
pub const LINE_GAP_MS: u32 = 400;

/// Little-endian RIFF/WAVE, PCM 16-bit, one channel. Samples are clamped to -1.0..=1.0.
pub fn encode_pcm16(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Concatenates takes with [`LINE_GAP_MS`] of silence between them.
pub fn join_lines(lines: &[Vec<f32>], sample_rate: u32) -> Vec<f32> {
    let gap = (sample_rate as u64 * u64::from(LINE_GAP_MS) / 1000) as usize;
    let total: usize = lines.iter().map(Vec::len).sum::<usize>() + gap * lines.len().saturating_sub(1);
    let mut out = Vec::with_capacity(total);
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.resize(out.len() + gap, 0.0);
        }
        out.extend_from_slice(line);
    }
    out
}

/// Linear resampling, used only when takes were captured at different rates (a new audio device mid-session).
pub fn resample_linear(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || samples.is_empty() || from == 0 || to == 0 {
        return samples.to_vec();
    }
    let out_len = (samples.len() as u64 * u64::from(to) / u64::from(from)) as usize;
    let step = f64::from(from) / f64::from(to);
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * step;
            let j = pos.floor() as usize;
            let frac = (pos - j as f64) as f32;
            let a = samples[j.min(samples.len() - 1)];
            let b = samples[(j + 1).min(samples.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_samples() {
        let wav = encode_pcm16(&[0.0, 1.0, -1.0, 2.0], 48_000);
        assert_eq!(wav.len(), 44 + 8);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes([wav[4], wav[5], wav[6], wav[7]]), 36 + 8);
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]), 48_000);
        assert_eq!(u32::from_le_bytes([wav[28], wav[29], wav[30], wav[31]]), 96_000);
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(i16::from_le_bytes([wav[44], wav[45]]), 0);
        assert_eq!(i16::from_le_bytes([wav[46], wav[47]]), i16::MAX);
        assert_eq!(i16::from_le_bytes([wav[48], wav[49]]), -i16::MAX);
        assert_eq!(i16::from_le_bytes([wav[50], wav[51]]), i16::MAX);
    }

    #[test]
    fn resamples_linearly() {
        assert_eq!(resample_linear(&[0.0, 1.0], 1, 1), vec![0.0, 1.0]);
        let up = resample_linear(&[0.0, 1.0], 1, 2);
        assert_eq!(up, vec![0.0, 0.5, 1.0, 1.0]);
        assert_eq!(resample_linear(&[0.0; 48_000], 48_000, 44_100).len(), 44_100);
        assert!(resample_linear(&[], 1, 2).is_empty());
    }

    #[test]
    fn joins_with_gaps() {
        let joined = join_lines(&[vec![0.5; 3], vec![0.25; 2]], 1000);
        assert_eq!(joined.len(), 3 + 400 + 2);
        assert_eq!(joined[3], 0.0);
        assert_eq!(joined[403], 0.25);
        assert!(join_lines(&[], 1000).is_empty());
    }
}
