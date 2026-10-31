//! site-gen: the public Streams Reader website as static files.
//!
//! Every page is prebuilt HTML in eight languages with one stylesheet compiled from SCSS (grass) on top of the
//! central brand tokens (`brand/dist/tokens`), plus one small external script (the hero demo and film, OS detection, the classics row, scroll reveals).
//! No inline scripts, styles or event handlers, so the Worker can serve it under a strict CSP.

pub mod assets;
pub mod build;
pub mod config;
pub mod css;
pub mod data;
pub mod html;
pub mod i18n;
pub mod icons;
pub mod layout;
pub mod pages;
pub mod seo;
pub mod tokens;

use std::path::{Path, PathBuf};

pub use build::{BuildReport, Paths, build};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("locale {lang}: {detail}")]
    Fluent { lang: &'static str, detail: String },
    #[error("missing translations: {0}")]
    Missing(String),
    #[error("scss: {0}")]
    Scss(String),
    #[error("tokens: {0}")]
    Tokens(String),
    #[error("data: {0}")]
    Data(String),
}

impl Error {
    pub fn io(path: &Path, source: std::io::Error) -> Self {
        Error::Io { path: path.to_path_buf(), source }
    }
}

/// FNV-1a 64, used for short content hashes in asset file names (cache busting, not security).
pub fn content_hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:010x}", h >> 24)
}

#[cfg(test)]
mod tests {
    #[test]
    fn content_hash_is_stable_and_short() {
        assert_eq!(super::content_hash(b"abc"), super::content_hash(b"abc"));
        assert_ne!(super::content_hash(b"abc"), super::content_hash(b"abd"));
        assert_eq!(super::content_hash(b"").len(), 10);
    }
}
