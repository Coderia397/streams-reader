//! `deploy/state.toml`: ids and URLs recorded by `setup` and `deploy`, so later commands and people can see what was
//! created. Gitignored. Commands still resolve resources by name through the API before mutating them; this file is a
//! record, not the source of truth.

use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::paths::tilde;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d1: Option<Named>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<Named>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r2: Option<Bucket>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<Worker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Named {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bucket {
    pub name: String,
    pub lifecycle_rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worker {
    pub name: String,
    pub url: String,
    pub app_version: String,
    pub kokoro_rev: String,
    pub ming_rev: String,
    pub deployed_at: String,
}

const HEADER: &str =
    "# Written by `cargo xtask setup` / `cargo xtask deploy`. Gitignored; safe to delete (it is rebuilt).\n";

impl State {
    /// The recorded state, or an empty one when the file does not exist yet.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path).with_context(|| format!("cannot read {}", tilde(path)))?;
        toml::from_str(&text).with_context(|| format!("cannot parse {}", tilde(path)))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", tilde(dir)))?;
        }
        let body = toml::to_string_pretty(self).context("cannot serialise state")?;
        fs::write(path, format!("{HEADER}{body}")).with_context(|| format!("cannot write {}", tilde(path)))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_empty_state() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(State::load(&dir.path().join("none.toml")).unwrap(), State::default());
    }

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deploy/state.toml");
        let state = State {
            d1: Some(Named { name: "nolabs".into(), id: "uuid-1".into() }),
            queue: Some(Named { name: "nolabs-render".into(), id: "q1".into() }),
            r2: Some(Bucket { name: "nolabs-media".into(), lifecycle_rule: "r".into() }),
            worker: Some(Worker {
                name: "nolabs-api".into(),
                url: "https://nolabs-api.x.workers.dev".into(),
                app_version: "abc1234".into(),
                kokoro_rev: "hexgrad/Kokoro-82M@1".into(),
                ming_rev: "inclusionAI/Ming-3.0-tts-0.5B@2".into(),
                deployed_at: "2026-10-30T20:00:00Z".into(),
            }),
        };
        state.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# Written by"));
        assert_eq!(State::load(&path).unwrap(), state);
    }
}
