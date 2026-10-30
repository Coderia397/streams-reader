//! `space`: uploads `space/` to the Hugging Face Space with the pinned `hf` CLI (run through uvx, so nothing is
//! installed globally), then follows the runtime until the new build is RUNNING or fails.

use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use crate::cmd::{git_short_sha, run_tool};
use crate::hf::{self, Hf, Step, Watch};
use crate::names::{SPACE_REPO, guard, repo_name};
use crate::paths;
use crate::secret::Secret;

const HF_CLI: &str = "huggingface_hub==0.36.2";
/// Not uploaded: tests, caches and editor/OS litter. `hf upload` matches with fnmatch, where `*` also crosses `/`.
pub const EXCLUDE: &[&str] =
    &["tests/*", "*__pycache__*", "*.pyc", "*.pytest_cache*", "*.DS_Store", "*.ruff_cache*", "*.venv*", "*.egg-info*"];
const POLL: Duration = Duration::from_secs(10);
const TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Polls of unchanged RUNNING (with no commit info) that count as "nothing to rebuild".
const SETTLE_POLLS: u32 = 9;

/// Arguments to `uvx`. The token is not among them: it goes through the `HF_TOKEN` environment variable.
pub fn upload_args(space_dir: &str, message: &str) -> Vec<String> {
    let mut args: Vec<String> = ["--from", HF_CLI, "hf", "upload", SPACE_REPO, space_dir, ".", "--repo-type", "space"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    args.push("--commit-message".to_owned());
    args.push(message.to_owned());
    args.push("--exclude".to_owned());
    args.extend(EXCLUDE.iter().map(|s| (*s).to_owned()));
    args
}

pub fn run(wait: bool) -> Result<()> {
    guard(repo_name(SPACE_REPO));
    let dir = paths::space_dir();
    for required in ["app.py", "README.md", "requirements.txt"] {
        if !dir.join(required).is_file() {
            bail!("space/{required} is missing");
        }
    }
    let token: Secret = hf::load_token()?;
    let message = format!("nolabs-reader {}", git_short_sha());
    let dir_str = dir.to_string_lossy().into_owned();
    let args = upload_args(&dir_str, &message);
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let env = [("HF_TOKEN", token.expose()), ("HF_HUB_DISABLE_TELEMETRY", "1")];
    run_tool("uvx", &arg_refs, &paths::repo_root(), &env)?;

    if !wait {
        return Ok(());
    }
    let hf = Hf::new(token)?;
    let expected = hf.space_sha(SPACE_REPO).ok();
    wait_running(&hf, expected)
}

fn wait_running(hf: &Hf, expected: Option<String>) -> Result<()> {
    let started = Instant::now();
    let mut watch = Watch::new(expected, SETTLE_POLLS);
    let mut last_stage = String::new();
    loop {
        let rt = hf.space_runtime(SPACE_REPO)?;
        if rt.stage != last_stage {
            println!("[{:>4}s] {}", started.elapsed().as_secs(), rt.stage);
            last_stage.clone_from(&rt.stage);
        }
        match watch.observe(&rt) {
            Step::Done => {
                println!("Space {SPACE_REPO} is RUNNING ({} s)", started.elapsed().as_secs());
                return Ok(());
            }
            Step::Failed(why) => bail!("Space {SPACE_REPO} failed: {why}"),
            Step::Wait if started.elapsed() > TIMEOUT => {
                bail!("Space still {} after {} min", rt.stage, TIMEOUT.as_secs() / 60)
            }
            Step::Wait => sleep(POLL),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn upload_args_pin_the_cli_and_carry_no_token() {
        let args = upload_args("/repo/space", "nolabs-reader abc");
        assert_eq!(&args[..4], ["--from", "huggingface_hub==0.36.2", "hf", "upload"]);
        assert_eq!(&args[4..9], ["ibrahimar397/nolabs-engine", "/repo/space", ".", "--repo-type", "space"]);
        let ex = args.iter().position(|a| a == "--exclude").unwrap();
        assert_eq!(&args[ex + 1..], EXCLUDE);
        assert!(args.iter().all(|a| !a.contains("token") && !a.starts_with("hf_")));
    }

    #[test]
    fn excludes_cover_tests_and_caches() {
        assert!(EXCLUDE.contains(&"tests/*"));
        assert!(EXCLUDE.iter().any(|p| p.contains("__pycache__")));
    }
}
