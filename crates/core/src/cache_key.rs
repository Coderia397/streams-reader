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
pub fn cache_key(input: &KeyInput<'_>) -> String {
    let _ = input;
    todo!("implemented in the core crate")
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
