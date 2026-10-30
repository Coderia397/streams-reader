//! Where things live: the repository (found from this crate's manifest, so commands work from any directory) and the
//! credential files under `~/.config`.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

pub fn home() -> Result<PathBuf> {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from).context("HOME is not set")
}

/// The workspace root (the parent of `xtask/`).
pub fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap_or(manifest).to_path_buf()
}

/// Cloudflare Global API Key file. Read by `bootstrap-token` and nothing else.
pub fn cloudflare_global_env() -> Result<PathBuf> {
    Ok(home()?.join(".config/streams/cloudflare.env"))
}

pub fn huggingface_env() -> Result<PathBuf> {
    Ok(home()?.join(".config/streams/huggingface.env"))
}

/// Scoped deploy token minted by `bootstrap-token`; every other Cloudflare command reads only this.
pub fn deploy_env() -> Result<PathBuf> {
    Ok(home()?.join(".config/nolabs/cloudflare-deploy.env"))
}

/// Passphrase, session key and passphrase hash written by `passphrase`.
pub fn app_env() -> Result<PathBuf> {
    Ok(home()?.join(".config/nolabs/app.env"))
}

/// Resource ids recorded by `setup` and `deploy` (gitignored).
pub fn state_file() -> PathBuf {
    repo_root().join("deploy/state.toml")
}

pub fn migrations_dir() -> PathBuf {
    repo_root().join("migrations")
}

pub fn web_crate() -> PathBuf {
    repo_root().join("crates/web")
}

pub fn worker_crate() -> PathBuf {
    repo_root().join("crates/worker")
}

pub fn space_dir() -> PathBuf {
    repo_root().join("space")
}

/// A path for messages: `~/…` under the home directory, relative to the repo inside it, absolute otherwise.
pub fn tilde(path: &Path) -> String {
    if let Ok(rel) = path.strip_prefix(repo_root()) {
        return rel.display().to_string();
    }
    match home() {
        Ok(h) => match path.strip_prefix(&h) {
            Ok(rel) => format!("~/{}", rel.display()),
            Err(_) => path.display().to_string(),
        },
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn repo_root_holds_the_workspace_manifest() {
        assert!(repo_root().join("Cargo.toml").is_file());
        assert!(repo_root().join("xtask").is_dir());
    }

    #[test]
    fn tilde_shortens_home_and_repo_paths() {
        let app = app_env().unwrap();
        assert_eq!(tilde(&app), "~/.config/nolabs/app.env");
        assert_eq!(tilde(&state_file()), "deploy/state.toml");
        assert_eq!(tilde(Path::new("/elsewhere/x")), "/elsewhere/x");
    }
}
