//! One module per subcommand, plus the helpers they share for running external tools.

pub mod bootstrap;
pub mod build;
pub mod deploy;
pub mod migrate;
pub mod providers;
pub mod resources;
pub mod setup;
pub mod smoke;
pub mod space;
pub mod status;
pub mod supabase;

use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use anyhow::{Result, bail};

use crate::paths::{self, tilde};

/// Runs a tool to completion with inherited stdio. Secrets go in through `env` only (never argv, which other local
/// users can read from the process table), and the error message names the tool, not its environment.
pub fn run_tool(program: &str, args: &[&str], dir: &Path, env: &[(&str, &str)]) -> Result<()> {
    println!("$ {program} {} (in {})", args.join(" "), tilde(dir));
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(dir);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let status = match cmd.status() {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::NotFound => bail!("{program} is not installed or not on PATH"),
        Err(e) => bail!("cannot start {program}: {e}"),
    };
    if !status.success() {
        bail!("{program} failed ({status})");
    }
    Ok(())
}

/// The repository's short commit id for `APP_VERSION`, or `"dev"` when git or a commit is unavailable.
pub fn git_short_sha() -> String {
    let out = Command::new("git").args(["rev-parse", "--short", "HEAD"]).current_dir(paths::repo_root()).output();
    match out {
        Ok(o) if o.status.success() => {
            let sha = String::from_utf8_lossy(&o.stdout).trim().to_owned();
            if sha.is_empty() { "dev".to_owned() } else { sha }
        }
        _ => {
            eprintln!("warning: no git commit found; APP_VERSION will be \"dev\"");
            "dev".to_owned()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn missing_tool_is_a_clear_error() {
        let err = run_tool("nolabs-no-such-tool", &[], &paths::repo_root(), &[]).unwrap_err().to_string();
        assert!(err.contains("not installed"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn env_reaches_the_child_and_failures_are_reported() {
        let secret = crate::secret::Secret::new("expected-value");
        let dir = paths::repo_root();
        let env = [("XT_SECRET", secret.expose())];
        run_tool("sh", &["-c", "test \"$XT_SECRET\" = expected-value"], &dir, &env).unwrap();
        let err = run_tool("sh", &["-c", "exit 3"], &dir, &env).unwrap_err().to_string();
        assert!(err.contains("sh failed") && !err.contains("expected-value"), "{err}");
    }
}
