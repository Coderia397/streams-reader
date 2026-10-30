//! Pure planning for a render request: the engine voice and fingerprint, the mode, cache key and object key of each
//! item, and how claimed items are grouped into jobs.

use std::ops::Range;

use nolabs_core::cache_key::{self, KeyInput};
use nolabs_core::proto::api::{Emotion, Engine, Priority, RenderItem};
use nolabs_core::proto::engine::{EngineItem, EngineVoice, Mode};
use nolabs_core::text::{NORMALISER_VERSION, SEGMENTER_VERSION};

use crate::config::{ADAPTER_VERSION, Config, audio_profile};

/// A voice ready to render with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedVoice {
    /// Catalogue id (`kokoro:…`, `ming:…`, `user:…`), stored on each render row.
    pub catalog_id: String,
    pub engine: Engine,
    pub voice: EngineVoice,
    /// Preset id, or `sha256(reference) + ":" + sha256(transcript)` for a user voice.
    pub fingerprint: String,
    pub is_user: bool,
}

/// Mode and effective emotion for one item. Kokoro has no emotion control, so its items are keyed as neutral and two
/// requests that differ only in emotion share audio.
pub fn mode_for(voice: &ResolvedVoice, emotion: Emotion) -> (Mode, Emotion) {
    match (voice.engine, voice.is_user, emotion) {
        (Engine::Kokoro, _, _) => (Mode::Kokoro, Emotion::Neutral),
        (Engine::Ming, false, Emotion::Neutral) => (Mode::Ip, Emotion::Neutral),
        (Engine::Ming, true, Emotion::Neutral) => (Mode::Tts, Emotion::Neutral),
        (Engine::Ming, _, e) => (Mode::Emotion, e),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedItem {
    pub key: String,
    pub item: EngineItem,
    pub chars: usize,
}

/// Plan every request item in order (duplicates included; they share a key).
pub fn plan_items(config: &Config, voice: &ResolvedVoice, items: &[RenderItem]) -> Vec<PlannedItem> {
    let profile = audio_profile(voice.engine);
    items
        .iter()
        .map(|it| {
            let (mode, emotion) = mode_for(voice, it.emotion);
            let key = cache_key::cache_key(&KeyInput {
                engine: voice.engine,
                model_rev: config.model_rev(voice.engine),
                adapter_version: ADAPTER_VERSION,
                voice_fingerprint: &voice.fingerprint,
                mode,
                emotion,
                boundary: it.boundary,
                normaliser_version: NORMALISER_VERSION,
                segmenter_version: SEGMENTER_VERSION,
                audio_profile_id: profile.id,
                text: &it.text,
            });
            let object_key = cache_key::audio_object_key(voice.engine, &key);
            PlannedItem {
                chars: it.text.chars().count(),
                item: EngineItem {
                    key: key.clone(),
                    text: it.text.clone(),
                    mode,
                    emotion,
                    boundary: it.boundary,
                    object_key,
                },
                key,
            }
        })
        .collect()
}

/// Largest Kokoro job (about 4 minutes of audio, under 2 minutes on the Space CPU).
pub const KOKORO_JOB_ITEMS: usize = 12;

/// Kokoro job boundaries over `n` claimed items. Interactive requests start with a 1-item job and a 3-item job so the
/// first audio is ready within seconds; everything else goes in jobs of [`KOKORO_JOB_ITEMS`].
pub fn kokoro_groups(n: usize, priority: Priority) -> Vec<Range<usize>> {
    let ramp: &[usize] = match priority {
        Priority::Interactive => &[1, 3],
        Priority::Background => &[],
    };
    let mut out = Vec::new();
    let mut at = 0;
    let mut step = 0;
    while at < n {
        let size = ramp.get(step).copied().unwrap_or(KOKORO_JOB_ITEMS);
        let end = (at + size).min(n);
        out.push(at..end);
        at = end;
        step += 1;
    }
    out
}

/// Ming job boundaries: greedily pack consecutive items while the job stays within `max_chars` (one GPU call, one of
/// the day's few runs, so jobs are as large as a call allows). Every job holds at least one item.
pub fn ming_groups(chars: &[usize], max_chars: usize) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut total = 0;
    for (i, &c) in chars.iter().enumerate() {
        if i > start && total + c > max_chars {
            out.push(start..i);
            start = i;
            total = 0;
        }
        total += c;
    }
    if start < chars.len() {
        out.push(start..chars.len());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(engine: Engine, is_user: bool) -> ResolvedVoice {
        ResolvedVoice {
            catalog_id: String::new(),
            engine,
            voice: EngineVoice::Preset { id: "x".into() },
            fingerprint: "x".into(),
            is_user,
        }
    }

    #[test]
    fn modes() {
        let k = voice(Engine::Kokoro, false);
        let m = voice(Engine::Ming, false);
        let u = voice(Engine::Ming, true);
        assert_eq!(mode_for(&k, Emotion::Happy), (Mode::Kokoro, Emotion::Neutral));
        assert_eq!(mode_for(&m, Emotion::Neutral), (Mode::Ip, Emotion::Neutral));
        assert_eq!(mode_for(&m, Emotion::Sad), (Mode::Emotion, Emotion::Sad));
        assert_eq!(mode_for(&u, Emotion::Neutral), (Mode::Tts, Emotion::Neutral));
        assert_eq!(mode_for(&u, Emotion::Angry), (Mode::Emotion, Emotion::Angry));
    }

    #[test]
    fn kokoro_grouping() {
        assert_eq!(kokoro_groups(0, Priority::Interactive), Vec::<Range<usize>>::new());
        assert_eq!(kokoro_groups(2, Priority::Interactive), vec![0..1, 1..2]);
        assert_eq!(kokoro_groups(20, Priority::Interactive), vec![0..1, 1..4, 4..16, 16..20]);
        assert_eq!(kokoro_groups(25, Priority::Background), vec![0..12, 12..24, 24..25]);
    }

    #[test]
    fn ming_grouping() {
        assert_eq!(ming_groups(&[], 10), Vec::<Range<usize>>::new());
        assert_eq!(ming_groups(&[4, 4, 4, 4], 10), vec![0..2, 2..4]);
        assert_eq!(ming_groups(&[4, 6, 1], 10), vec![0..2, 2..3]);
        // An item larger than the limit still gets a job of its own.
        assert_eq!(ming_groups(&[20, 3], 10), vec![0..1, 1..2]);
    }
}
