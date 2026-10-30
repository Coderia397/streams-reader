//! `build`: the web app first (the Worker embeds `crates/web/dist`), then the Worker. Reports artefact sizes,
//! including the gzip size Cloudflare checks against the Worker size limit.

use std::fs;
use std::io::Write as _;
use std::path::Path;

use anyhow::{Context as _, Result};
use flate2::Compression;
use flate2::write::GzEncoder;

use crate::cmd::run_tool;
use crate::paths::{self, tilde};
use crate::worker_bundle::collect_modules;

/// Workers Free plan limit on the compressed script (assumed current; Paid is 10 MiB).
const FREE_PLAN_GZIP_LIMIT: u64 = 3 * 1024 * 1024;

pub fn run() -> Result<()> {
    let web = paths::web_crate();
    run_tool("trunk", &["build", "--release"], &web, &[])?;
    let dist = web.join("dist");
    let (files, bytes) = dir_size(&dist)?;
    println!("web: {} in {files} files ({})", tilde(&dist), human(bytes));

    let worker = paths::worker_crate();
    run_tool("worker-build", &["--release"], &worker, &[])?;
    let build = worker.join("build");
    let modules = collect_modules(&build)?;
    let mut total_gz = 0;
    for m in &modules {
        let raw = fs::read(&m.path).with_context(|| format!("cannot read {}", tilde(&m.path)))?;
        let gz = gzip_len(&raw)?;
        total_gz += gz;
        println!("worker: {:<40} {:>10}  gzip {:>10}  {}", m.name, human(raw.len() as u64), human(gz), m.content_type);
    }
    println!("worker: total gzip {}", human(total_gz));
    if total_gz > FREE_PLAN_GZIP_LIMIT {
        eprintln!(
            "warning: over the {} Workers Free plan limit; the upload will be rejected",
            human(FREE_PLAN_GZIP_LIMIT)
        );
    }
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
    fn dir_size_counts_nested_files() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("sub")).unwrap();
        fs::write(d.path().join("a"), [0; 10]).unwrap();
        fs::write(d.path().join("sub/b"), [0; 5]).unwrap();
        assert_eq!(dir_size(d.path()).unwrap(), (2, 15));
    }
}
