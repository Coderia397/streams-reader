//! `build`: the landing site (`site-gen`), the web app (`trunk build --release`), then the Worker, which embeds both
//! (`crates/worker/build.rs`). Reports each build's size and the Worker's modules against Cloudflare's size limit.

use std::fs;
use std::io::Write as _;
use std::path::Path;

use anyhow::{Context as _, Result};
use flate2::Compression;
use flate2::write::GzEncoder;

use crate::cmd::run_tool;
use crate::paths::{self, tilde};
use crate::worker_bundle::collect_modules;

/// Worker size limit on every plan: 64 MiB uncompressed; there is no compressed limit (verified 2026-10-31 on
/// developers.cloudflare.com/workers/platform/limits, page dated 2026-10-29). gzip is printed for reference.
const WORKER_SIZE_LIMIT: u64 = 64 * 1024 * 1024;
/// Above this the bundle still uploads, but startup (1 s limit) and memory (128 MB per isolate) deserve a look.
const WORKER_SIZE_WARN: u64 = 32 * 1024 * 1024;

pub fn run() -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    run_tool(&cargo, &["run", "-q", "-p", "nolabs-site", "--bin", "site-gen"], &paths::repo_root(), &[])?;
    report_dir("site", &paths::site_crate().join("dist"))?;

    let web = paths::web_crate();
    run_tool("trunk", &["build", "--release"], &web, &[])?;
    report_dir("web", &web.join("dist"))?;

    let worker = paths::worker_crate();
    run_tool("worker-build", &["--release"], &worker, &[])?;
    let modules = collect_modules(&worker.join("build"))?;
    let (mut total, mut total_gz) = (0, 0);
    for m in &modules {
        let raw = fs::read(&m.path).with_context(|| format!("cannot read {}", tilde(&m.path)))?;
        let gz = gzip_len(&raw)?;
        total += raw.len() as u64;
        total_gz += gz;
        println!("worker: {:<40} {:>10}  gzip {:>10}  {}", m.name, human(raw.len() as u64), human(gz), m.content_type);
    }
    println!(
        "worker: total {} of the {} limit (gzip {}, not limited)",
        human(total),
        human(WORKER_SIZE_LIMIT),
        human(total_gz)
    );
    check_size(total)
}

/// Fails over the limit (the upload would be rejected); warns past half of it.
fn check_size(total: u64) -> Result<()> {
    if total > WORKER_SIZE_LIMIT {
        anyhow::bail!(
            "the Worker is {} and over the {} limit; the upload would be rejected",
            human(total),
            human(WORKER_SIZE_LIMIT)
        );
    }
    if total > WORKER_SIZE_WARN {
        eprintln!(
            "warning: the Worker is past {}; check startup time and memory after deploy",
            human(WORKER_SIZE_WARN)
        );
    }
    Ok(())
}

fn report_dir(label: &str, dir: &Path) -> Result<()> {
    let (files, bytes) = dir_size(dir)?;
    println!("{label}: {} in {files} files ({})", tilde(dir), human(bytes));
    Ok(())
}

fn dir_size(dir: &Path) -> Result<(usize, u64)> {
    let mut files = 0;
    let mut bytes = 0;
    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", tilde(dir)))? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            let (f, b) = dir_size(&entry.path())?;
            files += f;
            bytes += b;
        } else if kind.is_file() {
            files += 1;
            bytes += entry.metadata()?.len();
        }
    }
    Ok((files, bytes))
}

fn gzip_len(bytes: &[u8]) -> Result<u64> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(bytes)?;
    Ok(enc.finish()?.len() as u64)
}

pub fn human(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= KIB * KIB {
        format!("{:.2} MiB", b / KIB / KIB)
    } else if b >= KIB {
        format!("{:.1} KiB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn human_sizes() {
        assert_eq!(human(512), "512 B");
        assert_eq!(human(1536), "1.5 KiB");
        assert_eq!(human(3 * 1024 * 1024), "3.00 MiB");
    }

    #[test]
    fn gzip_shrinks_repetitive_input() {
        let n = gzip_len(&vec![b'a'; 10_000]).unwrap();
        assert!(n > 0 && n < 200, "{n}");
    }

    #[test]
    fn size_check_fails_only_over_the_limit() {
        assert!(check_size(WORKER_SIZE_LIMIT).is_ok());
        assert!(check_size(WORKER_SIZE_WARN + 1).is_ok());
        assert!(check_size(WORKER_SIZE_LIMIT + 1).unwrap_err().to_string().contains("over the"));
    }

    #[test]
    fn dir_size_counts_nested_files() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("sub")).unwrap();
        fs::write(d.path().join("a"), [0; 10]).unwrap();
        fs::write(d.path().join("sub/b"), [0; 5]).unwrap();
        assert_eq!(dir_size(d.path()).unwrap(), (2, 15));
    }
}
