//! App secrets this tool generates and keeps in `~/.config/nolabs/app.env`: `AUDIO_URL_KEY`, the HMAC key the Worker
//! signs short-lived audio URLs with (media elements cannot send an `Authorization` header).

use anyhow::Result;

use crate::envfile::{self, EnvFile};
use crate::paths::{self, tilde};
use crate::secret::Secret;

pub const AUDIO_URL_KEY: &str = "AUDIO_URL_KEY";

/// 32 random bytes as 64 lowercase hex chars.
pub fn generate_key_hex() -> Result<Secret> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no OS randomness: {e}"))?;
    let out = Secret::new(hex::encode(bytes));
    zeroize::Zeroize::zeroize(&mut bytes);
    Ok(out)
}

pub fn is_valid_key_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// The audio URL key from app.env, generated (and the file created) on first use. Rotating it only invalidates audio
/// URLs minted in the last six hours, so a malformed value is replaced rather than refused.
pub fn audio_url_key() -> Result<Secret> {
    let path = paths::app_env()?;
    let existing = EnvFile::load_optional(&path)?;
    if let Some(k) = existing.as_ref().and_then(|e| e.get(AUDIO_URL_KEY)).filter(|k| is_valid_key_hex(k.expose())) {
        return Ok(k.clone());
    }
    let key = generate_key_hex()?;
    let content = envfile::render(
        "nolabs app secrets. Written by `cargo xtask deploy`; never commit.\n\
         AUDIO_URL_KEY signs the audio URLs the Worker mints (Worker secret).",
        &[(AUDIO_URL_KEY, key.expose())],
    )?;
    envfile::write_private(&path, &content)?;
    println!("generated {AUDIO_URL_KEY} in {}", tilde(&path));
    Ok(key)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn key_shape() {
        let k = generate_key_hex().unwrap();
        assert!(is_valid_key_hex(k.expose()));
        assert_ne!(k, generate_key_hex().unwrap());
        assert!(!is_valid_key_hex(&"A".repeat(64)));
        assert!(!is_valid_key_hex(&"a".repeat(63)));
    }
}
