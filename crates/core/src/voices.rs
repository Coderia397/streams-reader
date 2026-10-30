//! Voice ids and the built-in catalogue. Ids are `kokoro:<voice>`, `ming:<preset>` or `user:<id>`.

use crate::ids;
use crate::proto::api::{Engine, Tier, VoiceInfo, VoiceKind, VoiceState};

pub const DEFAULT_VOICE: &str = "kokoro:af_heart";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceRef {
    Kokoro(String),
    MingPreset(String),
    User(String),
}

impl VoiceRef {
    pub fn parse(id: &str) -> Option<VoiceRef> {
        let (scheme, rest) = id.split_once(':')?;
        match scheme {
            "kokoro" if KOKORO.iter().any(|v| v.id == rest) => Some(VoiceRef::Kokoro(rest.to_owned())),
            "ming" if MING.iter().any(|v| v.id == rest) => Some(VoiceRef::MingPreset(rest.to_owned())),
            "user" if ids::is_id(rest) => Some(VoiceRef::User(rest.to_owned())),
            _ => None,
        }
    }

    /// User voices are rendered by Ming from their reference clip.
    pub fn engine(&self) -> Engine {
        match self {
            VoiceRef::Kokoro(_) => Engine::Kokoro,
            VoiceRef::MingPreset(_) | VoiceRef::User(_) => Engine::Ming,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub gender: &'static str,
    pub accent: &'static str,
    pub description: &'static str,
    pub featured: bool,
}

const fn p(
    id: &'static str,
    name: &'static str,
    gender: &'static str,
    accent: &'static str,
    description: &'static str,
    featured: bool,
) -> Preset {
    Preset { id, name, gender, accent, description, featured }
}

/// English Kokoro-82M voices. Featured ones are the best graded in the model's VOICES.md.
pub const KOKORO: &[Preset] = &[
    p("af_heart", "Heart", "female", "American", "Warm, clear storyteller. The default.", true),
    p("af_bella", "Bella", "female", "American", "Bright and lively, good for fiction.", true),
    p("af_nicole", "Nicole", "female", "American", "Soft and close, like a quiet room.", true),
    p("bf_emma", "Emma", "female", "British", "Composed British narrator.", true),
    p("am_michael", "Michael", "male", "American", "Even, steady and friendly.", true),
    p("am_fenrir", "Fenrir", "male", "American", "Deeper voice with weight.", true),
    p("am_puck", "Puck", "male", "American", "Light and animated.", true),
    p("bm_george", "George", "male", "British", "Classic British reader.", true),
    p("bm_fable", "Fable", "male", "British", "Gentle British storyteller.", true),
    p("af_aoede", "Aoede", "female", "American", "Smooth and calm.", false),
    p("af_kore", "Kore", "female", "American", "Crisp and direct.", false),
    p("af_sarah", "Sarah", "female", "American", "Neutral and plain.", false),
    p("af_alloy", "Alloy", "female", "American", "Even, synthetic tone.", false),
    p("af_nova", "Nova", "female", "American", "Upbeat.", false),
    p("af_sky", "Sky", "female", "American", "Airy.", false),
    p("af_jessica", "Jessica", "female", "American", "Casual.", false),
    p("af_river", "River", "female", "American", "Relaxed.", false),
    p("am_echo", "Echo", "male", "American", "Flat and clear.", false),
    p("am_eric", "Eric", "male", "American", "Matter-of-fact.", false),
    p("am_liam", "Liam", "male", "American", "Young and casual.", false),
    p("am_onyx", "Onyx", "male", "American", "Low and dry.", false),
    p("am_santa", "Santa", "male", "American", "Jolly.", false),
    p("am_adam", "Adam", "male", "American", "Rough, experimental.", false),
    p("bf_isabella", "Isabella", "female", "British", "Bright British voice.", false),
    p("bf_alice", "Alice", "female", "British", "Light British voice.", false),
    p("bf_lily", "Lily", "female", "British", "Young British voice.", false),
    p("bm_lewis", "Lewis", "male", "British", "Low British voice.", false),
    p("bm_daniel", "Daniel", "male", "British", "Plain British voice.", false),
];

/// English built-in voices of Ming-3.0-tts-0.5B (expressive tier, GPU budget).
pub const MING: &[Preset] = &[
    p("US_Emma", "Emma", "female", "American", "Expressive American narrator, emotion-aware.", true),
    p("US_James", "James", "male", "American", "Expressive American narrator, emotion-aware.", true),
    p("UK_George", "George", "male", "British", "Expressive British narrator, emotion-aware.", true),
    p("UK_Charlotte", "Charlotte", "female", "British", "Expressive British narrator, emotion-aware.", true),
    p("US_Stella", "Stella", "female", "American", "Expressive American voice.", false),
];

fn info(preset: &Preset, engine: Engine) -> VoiceInfo {
    let (scheme, tier) = match engine {
        Engine::Kokoro => ("kokoro", Tier::Standard),
        Engine::Ming => ("ming", Tier::Expressive),
    };
    VoiceInfo {
        id: format!("{scheme}:{}", preset.id),
        engine,
        tier,
        kind: VoiceKind::Preset,
        name: preset.name.to_owned(),
        gender: Some(preset.gender.to_owned()),
        accent: Some(preset.accent.to_owned()),
        description: preset.description.to_owned(),
        featured: preset.featured,
        state: VoiceState::Ready,
        preview_key: None,
    }
}

/// Every built-in voice, Kokoro first.
pub fn builtin_catalog() -> Vec<VoiceInfo> {
    KOKORO.iter().map(|v| info(v, Engine::Kokoro)).chain(MING.iter().map(|v| info(v, Engine::Ming))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_ids_only() {
        assert_eq!(VoiceRef::parse("kokoro:af_heart"), Some(VoiceRef::Kokoro("af_heart".into())));
        assert_eq!(VoiceRef::parse("ming:UK_George").map(|v| v.engine()), Some(Engine::Ming));
        assert_eq!(VoiceRef::parse("user:0123456789abcdef"), Some(VoiceRef::User("0123456789abcdef".into())));
        assert_eq!(VoiceRef::parse("kokoro:nobody"), None);
        assert_eq!(VoiceRef::parse("user:../x"), None);
        assert_eq!(VoiceRef::parse("af_heart"), None);
    }

    #[test]
    fn catalog_has_default_and_nine_featured_kokoro_voices() {
        let cat = builtin_catalog();
        assert!(cat.iter().any(|v| v.id == DEFAULT_VOICE));
        assert_eq!(cat.iter().filter(|v| v.engine == Engine::Kokoro && v.featured).count(), 9);
        assert_eq!(cat.iter().filter(|v| v.engine == Engine::Kokoro).count(), 28);
    }
}
