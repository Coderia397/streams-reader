//! `status`: the Space runtime from the Hugging Face API and the app's own `/api/status` (which needs a login, so it
//! uses the passphrase from app.env). Either half is reported even when the other fails.

use anyhow::{Context as _, Result, bail};
use nolabs_core::proto::api::SystemStatus;

use crate::api::Api;
use crate::envfile::EnvFile;
use crate::hf::Hf;
use crate::names::SPACE_REPO;
use crate::paths::{self, tilde};
use crate::secret::Secret;
use crate::state::State;

/// `--base`, else the URL `deploy` recorded.
pub fn resolve_base(base: Option<String>) -> Result<String> {
    if let Some(b) = base {
        return Ok(b);
    }
    let path = paths::state_file();
    match State::load(&path)?.worker {
        Some(w) => Ok(w.url),
        None => bail!("no deployed URL in {}: run `cargo xtask deploy` or pass --base", tilde(&path)),
    }
}

pub fn login_passphrase() -> Result<Secret> {
    let path = paths::app_env()?;
    let env = EnvFile::load(&path).with_context(|| "run `cargo xtask passphrase` first")?;
    Ok(env.secret("PASSPHRASE")?.clone())
}

pub fn print_system_status(s: &SystemStatus) {
    println!("app version   {}", s.version);
    println!("space         {} (healthy: {}, checked {} ms)", s.space.stage, s.space.healthy, s.space.checked_at_ms);
    println!("gpu budget    {}/{} s, {}/{} runs", s.gpu.used_s, s.gpu.cap_s, s.gpu.runs_used, s.gpu.runs_cap);
    if let Some(at) = s.gpu.resets_at_ms {
        println!("gpu resets    {at} ms");
    }
    println!("render queue  {} waiting", s.queued);
}

pub fn run(base: Option<String>) -> Result<()> {
    let space = space_status();
    if let Err(e) = &space {
        eprintln!("space: {e:#}");
    }
    let app = app_status(base);
    if let Err(e) = &app {
        eprintln!("app: {e:#}");
    }
    if space.is_err() || app.is_err() {
        bail!("status check failed");
    }
    Ok(())
}

fn space_status() -> Result<()> {
    let hf = Hf::from_env()?;
    let rt = hf.space_runtime(SPACE_REPO)?;
    let hw = rt.hardware.as_ref().and_then(|h| h.current.clone().or_else(|| h.requested.clone())).unwrap_or_default();
    println!("space {SPACE_REPO}: {} {hw}", rt.stage);
    if let Some(msg) = rt.error_message.filter(|m| !m.is_empty()) {
        println!("space error: {msg}");
    }
    Ok(())
}

fn app_status(base: Option<String>) -> Result<()> {
    let base = resolve_base(base)?;
    let passphrase = login_passphrase()?;
    let mut api = Api::new(&base)?;
    api.login(&passphrase)?;
    let status = api.get_json::<SystemStatus>("/api/status");
    let _ = api.logout();
    println!("app {}:", api.base());
    print_system_status(&status?);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn explicit_base_wins() {
        assert_eq!(resolve_base(Some("https://x.dev".into())).unwrap(), "https://x.dev");
    }
}
