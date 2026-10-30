//! Content-addressed keys for rendered audio. A key changes whenever anything that changes the sound changes, so a
//! re-recorded voice or a model update never serves stale audio. Playback speed is not part of the key (the app applies
//! it with `playbackRate`).

use crate::proto::api::{Boundary, Emotion, Engine};
use crate::proto::engine::Mode;

/// Bump to invalidate every key at once.
pub const KEY_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct KeyInput<'a> {
    pub engine: Engine,
    /// `repo@commit` of the model weights, from the Worker's engine config.
    pub model_rev: &'a str,
    /// Bump when the Space adapter changes how it drives the model.
    pub adapter_version: u32,
    /// Preset id, or `sha256(reference wav) + ":" + sha256(transcript)` for reference voices.
    pub voice_fingerprint: &'a str,
    pub mode: Mode,
    pub emotion: Emotion,
    pub boundary: Boundary,
    pub normaliser_version: u32,
    pub segmenter_version: u32,
    /// [`crate::proto::engine::AudioProfile::id`].
    pub audio_profile_id: u32,
    pub text: &'a str,
}

/// sha256 over a canonical, length-prefixed encoding of every field (u32 little-endian length before each string,
/// fixed tags for enums), lowercase hex, 64 chars. Locked by golden tests: changing the encoding requires bumping
/// [`KEY_VERSION`].
///
/// Field order: `KEY_VERSION`, engine, model_rev, adapter_version, voice_fingerprint, mode, emotion, boundary,
/// normaliser_version, segmenter_version, audio_profile_id, text. Integers are u32 little-endian; enum tags are one byte
/// each, assigned explicitly below so reordering an enum never changes a key.
pub fn cache_key(input: &KeyInput<'_>) -> String {
    use sha2::{Digest, Sha256};

    let mut h = Sha256::new();
    put_u32(&mut h, KEY_VERSION);
    h.update([engine_tag(input.engine)]);
    put_str(&mut h, input.model_rev);
    put_u32(&mut h, input.adapter_version);
    put_str(&mut h, input.voice_fingerprint);
    h.update([mode_tag(input.mode), emotion_tag(input.emotion), boundary_tag(input.boundary)]);
    put_u32(&mut h, input.normaliser_version);
    put_u32(&mut h, input.segmenter_version);
    put_u32(&mut h, input.audio_profile_id);
    put_str(&mut h, input.text);
    hex::encode(h.finalize())
}

fn put_u32(h: &mut sha2::Sha256, n: u32) {
    use sha2::Digest;
    h.update(n.to_le_bytes());
}

/// Length prefix first, so no two different field sequences can produce the same bytes.
fn put_str(h: &mut sha2::Sha256, s: &str) {
    use sha2::Digest;
    // Segment text is far below 4 GiB; saturating only guards the cast.
    put_u32(h, u32::try_from(s.len()).unwrap_or(u32::MAX));
    h.update(s.as_bytes());
}

fn engine_tag(engine: Engine) -> u8 {
    match engine {
        Engine::Kokoro => 1,
        Engine::Ming => 2,
    }
}

fn mode_tag(mode: Mode) -> u8 {
    match mode {
        Mode::Kokoro => 1,
        Mode::Ip => 2,
        Mode::Tts => 3,
        Mode::Emotion => 4,
    }
}

fn emotion_tag(emotion: Emotion) -> u8 {
    match emotion {
        Emotion::Neutral => 1,
        Emotion::Happy => 2,
        Emotion::Sad => 3,
        Emotion::Angry => 4,
        Emotion::Fearful => 5,
        Emotion::Disgusted => 6,
        Emotion::Surprised => 7,
    }
}

fn boundary_tag(boundary: Boundary) -> u8 {
    match boundary {
        Boundary::Sentence => 1,
        Boundary::Paragraph => 2,
        Boundary::Section => 3,
    }
}

/// True for 64 lowercase hex chars.
pub fn is_key(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// R2 object path of a rendered segment. Engines get separate prefixes so lifecycle rules can expire Kokoro audio
/// (cheap to re-render) without touching Ming audio.
pub fn audio_object_key(engine: Engine, key: &str) -> String {
    let engine = match engine {
        Engine::Kokoro => "kokoro",
        Engine::Ming => "ming",
    };
    format!("audio/{engine}/{key}.webm")
}

/// sha256 hex of arbitrary bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kokoro_input() -> KeyInput<'static> {
        KeyInput {
            engine: Engine::Kokoro,
            model_rev: "hexgrad/Kokoro-82M@f3ff3571791e39611d31c381e3a41a3af07b4987",
            adapter_version: 1,
            voice_fingerprint: "af_heart",
            mode: Mode::Kokoro,
            emotion: Emotion::Neutral,
            boundary: Boundary::Sentence,
            normaliser_version: 1,
            segmenter_version: 1,
            audio_profile_id: 1,
            text: "Hello, world.",
        }
    }

    const MING_FINGERPRINT: &str = concat!(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ":",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );

    fn ming_input() -> KeyInput<'static> {
        KeyInput {
            engine: Engine::Ming,
            model_rev: "inclusionAI/Ming-3.0-tts-0.5B@0123456789abcdef",
            adapter_version: 2,
            voice_fingerprint: MING_FINGERPRINT,
            mode: Mode::Emotion,
            emotion: Emotion::Surprised,
            boundary: Boundary::Section,
            normaliser_version: 1,
            segmenter_version: 1,
            audio_profile_id: 2,
            text: "“Wait…” — she said. Café ünïcödé 😀",
        }
    }

    /// Golden values, cross-checked with an independent Python implementation of the encoding (hashlib + struct).
    /// If one of these fails, the encoding changed: bump `KEY_VERSION` and update the values deliberately.
    #[test]
    fn golden_keys_are_stable() {
        assert_eq!(KEY_VERSION, 1);
        assert_eq!(cache_key(&kokoro_input()), "b254c75b2f3594c2e465feff2a1d87b340f97e857482067eff777ad17896978f");
        assert_eq!(cache_key(&ming_input()), "d4ac3021985b85400bcf4163504cbe2aa622f76e963a2423bf4cb8288b492bbb");
        let empty = KeyInput {
            engine: Engine::Kokoro,
            model_rev: "",
            adapter_version: 0,
            voice_fingerprint: "",
            mode: Mode::Kokoro,
            emotion: Emotion::Neutral,
            boundary: Boundary::Sentence,
            normaliser_version: 0,
            segmenter_version: 0,
            audio_profile_id: 0,
            text: "",
        };
        assert_eq!(cache_key(&empty), "8b178782657bac783d1f9dbb6276096caef68ad6eb15f683c5f7425ebefe6759");
    }

    #[test]
    fn keys_are_lowercase_hex() {
        assert!(is_key(&cache_key(&kokoro_input())));
        assert!(is_key(&cache_key(&ming_input())));
    }

    #[test]
    fn every_field_changes_the_key() {
        let base = ming_input();
        let base_key = cache_key(&base);
        let variants = [
            KeyInput { engine: Engine::Kokoro, ..base.clone() },
            KeyInput { model_rev: "inclusionAI/Ming-3.0-tts-0.5B@fedcba9876543210", ..base.clone() },
            KeyInput { adapter_version: 3, ..base.clone() },
            KeyInput { voice_fingerprint: "US_Emma", ..base.clone() },
            KeyInput { mode: Mode::Tts, ..base.clone() },
            KeyInput { emotion: Emotion::Sad, ..base.clone() },
            KeyInput { boundary: Boundary::Paragraph, ..base.clone() },
            KeyInput { normaliser_version: 2, ..base.clone() },
            KeyInput { segmenter_version: 2, ..base.clone() },
            KeyInput { audio_profile_id: 1, ..base.clone() },
            KeyInput { text: "“Wait…” — she said. Café ünïcödé 😀 ", ..base.clone() },
        ];
        let mut keys = vec![base_key];
        for v in &variants {
            let key = cache_key(v);
            assert!(!keys.contains(&key), "{v:?} collides");
            keys.push(key);
        }
    }

    #[test]
    fn length_prefixes_keep_field_boundaries_apart() {
        // Moving a char from one string field to the next must change the key.
        let a = KeyInput { model_rev: "ab", voice_fingerprint: "c", ..kokoro_input() };
        let b = KeyInput { model_rev: "a", voice_fingerprint: "bc", ..kokoro_input() };
        assert_ne!(cache_key(&a), cache_key(&b));
    }

    #[test]
    fn enum_tags_are_distinct() {
        let modes = [Mode::Kokoro, Mode::Ip, Mode::Tts, Mode::Emotion].map(mode_tag);
        let emotions = [
            Emotion::Neutral,
            Emotion::Happy,
            Emotion::Sad,
            Emotion::Angry,
            Emotion::Fearful,
            Emotion::Disgusted,
            Emotion::Surprised,
        ]
        .map(emotion_tag);
        let boundaries = [Boundary::Sentence, Boundary::Paragraph, Boundary::Section].map(boundary_tag);
        for tags in
            [&modes[..], &emotions[..], &boundaries[..], &[engine_tag(Engine::Kokoro), engine_tag(Engine::Ming)]]
        {
            let mut sorted = tags.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), tags.len());
        }
    }

    #[test]
    fn object_keys_separate_engines() {
        let key = cache_key(&kokoro_input());
        assert_eq!(audio_object_key(Engine::Kokoro, &key), format!("audio/kokoro/{key}.webm"));
        assert_eq!(audio_object_key(Engine::Ming, &key), format!("audio/ming/{key}.webm"));
    }

    #[test]
    fn is_key_rejects_near_misses() {
        assert!(!is_key(""));
        assert!(!is_key(&"A".repeat(64)));
        assert!(!is_key(&"g".repeat(64)));
        assert!(!is_key(&"a".repeat(63)));
        assert!(!is_key(&"a".repeat(65)));
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
