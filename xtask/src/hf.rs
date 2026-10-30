//! Hugging Face Hub API: Space runtime stage, the Space's head commit, and model head revisions. The token goes only
//! into the `Authorization` header and is redacted from every error.

use std::time::Duration;

use anyhow::{Context as _, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::cf::http_client;
use crate::envfile::EnvFile;
use crate::paths;
use crate::secret::{Redactor, Secret};

const HUB: &str = "https://huggingface.co";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Runtime {
    pub stage: String,
    /// Commit the runtime was built from (assumed field; absent on some responses).
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default, rename = "errorMessage")]
    pub error_message: Option<String>,
    #[serde(default)]
    pub hardware: Option<Hardware>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Hardware {
    #[serde(default)]
    pub current: Option<String>,
    #[serde(default)]
    pub requested: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepoInfo {
    sha: String,
}

pub struct Hf {
    http: Client,
    token: Secret,
    redactor: Redactor,
}

pub fn load_token() -> Result<Secret> {
    Ok(EnvFile::load(&paths::huggingface_env()?)?.secret("HF_TOKEN")?.clone())
}

impl Hf {
    pub fn new(token: Secret) -> Result<Self> {
        let redactor = Redactor::new().with(&token);
        Ok(Self { http: http_client(Duration::from_secs(60))?, token, redactor })
    }

    pub fn from_env() -> Result<Self> {
        Self::new(load_token()?)
    }

    pub fn space_runtime(&self, repo: &str) -> Result<Runtime> {
        self.get(&format!("/api/spaces/{repo}/runtime"), true)
    }

    pub fn space_sha(&self, repo: &str) -> Result<String> {
        self.get::<RepoInfo>(&format!("/api/spaces/{repo}"), true).map(|r| r.sha)
    }

    /// Head commit of a public model's `main` branch. Sent without the token: it is public.
    pub fn model_rev(&self, repo: &str) -> Result<String> {
        self.get::<RepoInfo>(&format!("/api/models/{repo}/revision/main"), false).map(|r| r.sha)
    }

    fn get<T: DeserializeOwned>(&self, path: &str, auth: bool) -> Result<T> {
        let mut req = self.http.get(format!("{HUB}{path}"));
        if auth {
            req = req.bearer_auth(self.token.expose());
        }
        let resp = req.send().map_err(|e| anyhow::anyhow!("GET {path}: {}", self.redactor.redact(&e.to_string())))?;
        let status = resp.status();
        let text = self.redactor.redact(&resp.text().unwrap_or_default());
        if !status.is_success() {
            let short: String = text.chars().take(300).collect();
            anyhow::bail!("GET {HUB}{path}: HTTP {}: {short}", status.as_u16());
        }
        serde_json::from_str(&text).with_context(|| format!("GET {path}: unexpected response shape"))
    }
}

/// What to do after one runtime poll while waiting for a new build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Wait,
    Done,
    Failed(String),
}

/// Tracks a Space rebuild. Right after an upload the runtime can still report the old build as RUNNING, so RUNNING
/// only counts once the runtime reports the expected commit, or after a non-RUNNING stage was seen, or when RUNNING
/// has persisted for `settle` polls with no commit info (an upload that changed nothing does not rebuild).
#[derive(Debug, Clone)]
pub struct Watch {
    expected_sha: Option<String>,
    left_running: bool,
    running_polls: u32,
    settle: u32,
}

impl Watch {
    pub fn new(expected_sha: Option<String>, settle: u32) -> Self {
        Self { expected_sha, left_running: false, running_polls: 0, settle }
    }

    pub fn observe(&mut self, rt: &Runtime) -> Step {
        let stage = rt.stage.as_str();
        if stage.ends_with("_ERROR") || stage == "NO_APP_FILE" {
            let why = rt.error_message.clone().unwrap_or_default();
            return Step::Failed(if why.is_empty() { stage.to_owned() } else { format!("{stage}: {why}") });
        }
        if stage != "RUNNING" {
            self.left_running = true;
            self.running_polls = 0;
            return Step::Wait;
        }
        self.running_polls += 1;
        match (&self.expected_sha, &rt.sha) {
            (Some(want), Some(have)) if want == have => Step::Done,
            (Some(_), Some(_)) => Step::Wait,
            _ if self.left_running || self.running_polls >= self.settle => Step::Done,
            _ => Step::Wait,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn rt(stage: &str, sha: Option<&str>) -> Runtime {
        Runtime { stage: stage.into(), sha: sha.map(Into::into), error_message: None, hardware: None }
    }

    #[test]
    fn parses_runtime_json() {
        let r: Runtime = serde_json::from_str(
            r#"{"stage":"RUNTIME_ERROR","hardware":{"current":null,"requested":"zero-a10g"},"errorMessage":"boom","gcTimeout":172800}"#,
        )
        .unwrap();
        assert_eq!(r.stage, "RUNTIME_ERROR");
        assert_eq!(r.error_message.as_deref(), Some("boom"));
        assert_eq!(r.hardware.unwrap().requested.as_deref(), Some("zero-a10g"));
    }

    #[test]
    fn done_when_expected_commit_runs() {
        let mut w = Watch::new(Some("new".into()), 3);
        assert_eq!(w.observe(&rt("RUNNING", Some("old"))), Step::Wait);
        assert_eq!(w.observe(&rt("BUILDING", Some("new"))), Step::Wait);
        assert_eq!(w.observe(&rt("APP_STARTING", Some("new"))), Step::Wait);
        assert_eq!(w.observe(&rt("RUNNING", Some("new"))), Step::Done);
    }

    #[test]
    fn without_sha_needs_a_transition_or_settling() {
        let mut w = Watch::new(Some("new".into()), 3);
        assert_eq!(w.observe(&rt("RUNNING", None)), Step::Wait);
        assert_eq!(w.observe(&rt("BUILDING", None)), Step::Wait);
        assert_eq!(w.observe(&rt("RUNNING", None)), Step::Done);

        let mut quiet = Watch::new(None, 3);
        assert_eq!(quiet.observe(&rt("RUNNING", None)), Step::Wait);
        assert_eq!(quiet.observe(&rt("RUNNING", None)), Step::Wait);
        assert_eq!(quiet.observe(&rt("RUNNING", None)), Step::Done);
    }

    #[test]
    fn old_commit_running_keeps_waiting() {
        let mut w = Watch::new(Some("new".into()), 1);
        for _ in 0..5 {
            assert_eq!(w.observe(&rt("RUNNING", Some("old"))), Step::Wait);
        }
    }

    #[test]
    fn error_stages_fail_with_message() {
        let mut w = Watch::new(None, 3);
        let mut r = rt("BUILD_ERROR", None);
        r.error_message = Some("pip failed".into());
        assert_eq!(w.observe(&r), Step::Failed("BUILD_ERROR: pip failed".into()));
        assert_eq!(w.observe(&rt("CONFIG_ERROR", None)), Step::Failed("CONFIG_ERROR".into()));
        assert_eq!(w.observe(&rt("NO_APP_FILE", None)), Step::Failed("NO_APP_FILE".into()));
    }
}
