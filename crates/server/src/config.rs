//! Deployment configuration (Worker vars and secrets, names as in notes/09-build-spec.md) and the fixed engine
//! settings that feed the cache key.

use nolabs_core::budget::GpuPolicy;
use nolabs_core::proto::api::Engine;
use nolabs_core::proto::engine::{AudioProfile, PauseProfile};

/// Bump when the Space adapter changes how it drives a model; part of every cache key.
pub const ADAPTER_VERSION: u32 = 1;

const PAUSES: PauseProfile = PauseProfile { sentence_ms: 250, paragraph_ms: 600, section_ms: 1200 };

/// Kokoro: 24 kHz source, WebM Opus 32 kbps, per-segment correction up to ±4 dB (notes/09, "Audio").
pub const KOKORO_AUDIO: AudioProfile = AudioProfile {
    id: 1,
    bitrate_kbps: 32,
    sample_rate: 24_000,
    target_lufs: -16.0,
    max_correction_db: 4.0,
    peak_ceiling_dbfs: -1.5,
    pauses: PAUSES,
};

/// Ming: 44.1 kHz source resampled to 48 kHz, WebM Opus 56 kbps, per-segment correction up to ±2 dB.
pub const MING_AUDIO: AudioProfile = AudioProfile {
    id: 2,
    bitrate_kbps: 56,
    sample_rate: 48_000,
    target_lufs: -16.0,
    max_correction_db: 2.0,
    peak_ceiling_dbfs: -1.5,
    pauses: PAUSES,
};

pub fn audio_profile(engine: Engine) -> AudioProfile {
    match engine {
        Engine::Kokoro => KOKORO_AUDIO,
        Engine::Ming => MING_AUDIO,
    }
}

/// Everything the services read from the environment. Secrets are held as decoded bytes and never logged.
#[derive(Clone)]
pub struct Config {
    /// Base URL of the Space, without a trailing slash.
    pub space_url: String,
    /// `owner/name` of the Space, for the Hugging Face runtime API.
    pub space_repo: String,
    pub kokoro_rev: String,
    pub ming_rev: String,
    pub app_version: String,
    pub hf_token: String,
    pub session_key: Vec<u8>,
    pub passphrase_hash: Vec<u8>,
    pub gpu: GpuPolicy,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("space_url", &self.space_url)
            .field("space_repo", &self.space_repo)
            .field("kokoro_rev", &self.kokoro_rev)
            .field("ming_rev", &self.ming_rev)
            .field("app_version", &self.app_version)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("missing worker variable or secret {0}")]
    Missing(&'static str),
    #[error("worker secret {0} is not valid hex of the expected length")]
    Malformed(&'static str),
}

impl Config {
    /// Build from a lookup over Worker vars and secrets. Errors name the variable, never its value.
    pub fn from_lookup(lookup: impl Fn(&'static str) -> Option<String>) -> Result<Config, ConfigError> {
        let get = |name: &'static str| {
            lookup(name).map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()).ok_or(ConfigError::Missing(name))
        };
        let hex32 = |name: &'static str| -> Result<Vec<u8>, ConfigError> {
            let bytes = hex::decode(get(name)?).map_err(|_| ConfigError::Malformed(name))?;
            if bytes.len() == 32 { Ok(bytes) } else { Err(ConfigError::Malformed(name)) }
        };
        Ok(Config {
            space_url: get("SPACE_URL")?.trim_end_matches('/').to_owned(),
            space_repo: get("SPACE_REPO")?,
            kokoro_rev: get("KOKORO_REV")?,
            ming_rev: get("MING_REV")?,
            app_version: lookup("APP_VERSION").unwrap_or_else(|| "dev".to_owned()),
            hf_token: get("HF_TOKEN")?,
            session_key: hex32("SESSION_KEY")?,
            passphrase_hash: hex32("PASSPHRASE_HASH")?,
            gpu: GpuPolicy::default(),
        })
    }

    pub fn model_rev(&self, engine: Engine) -> &str {
        match engine {
            Engine::Kokoro => &self.kokoro_rev,
            Engine::Ming => &self.ming_rev,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn vars() -> HashMap<&'static str, String> {
        HashMap::from([
            ("SPACE_URL", "https://x.hf.space/".to_owned()),
            ("SPACE_REPO", "o/x".to_owned()),
            ("KOKORO_REV", "k@1".to_owned()),
            ("MING_REV", "m@1".to_owned()),
            ("HF_TOKEN", "hf_x".to_owned()),
            ("SESSION_KEY", "11".repeat(32)),
            ("PASSPHRASE_HASH", "22".repeat(32)),
        ])
    }

    #[test]
    fn reads_all_values_and_trims_the_url() {
        let v = vars();
        let c = Config::from_lookup(|k| v.get(k).cloned()).unwrap();
        assert_eq!(c.space_url, "https://x.hf.space");
        assert_eq!(c.session_key, vec![0x11; 32]);
        assert_eq!(c.app_version, "dev");
        assert_eq!(c.model_rev(Engine::Ming), "m@1");
        assert!(!format!("{c:?}").contains("hf_x"));
    }

    #[test]
    fn names_missing_and_malformed_secrets() {
        let mut v = vars();
        v.remove("HF_TOKEN");
        assert_eq!(Config::from_lookup(|k| v.get(k).cloned()).unwrap_err(), ConfigError::Missing("HF_TOKEN"));
        let mut v = vars();
        v.insert("SESSION_KEY", "abcd".into());
        assert_eq!(Config::from_lookup(|k| v.get(k).cloned()).unwrap_err(), ConfigError::Malformed("SESSION_KEY"));
    }
}
