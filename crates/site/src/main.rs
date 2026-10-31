//! `site-gen [--out <dir>] [--repo <root>] [--check]`
//!
//! Builds the public website into `crates/site/dist` (or `--out`). `--check` only lints the SCSS for raw design
//! values (hex colours, rgb()/hsl() literals, literal radii) and exits non-zero if any are found.

use std::path::PathBuf;
use std::process::ExitCode;

use nolabs_site::{Paths, build, css, seo::Date};

fn main() -> ExitCode {
    let mut paths = Paths::from_manifest();
    let mut check_only = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => match args.next() {
                Some(v) => paths.out = PathBuf::from(v),
                None => return usage("--out needs a folder"),
            },
            "--repo" => match args.next() {
                Some(v) => paths.repo_root = PathBuf::from(v),
                None => return usage("--repo needs a folder"),
            },
            "--check" => check_only = true,
            "-h" | "--help" => return usage(""),
            other => return usage(&format!("unknown argument {other}")),
        }
    }

    if check_only {
        return match css::lint_dir(&paths.crate_dir.join("style")) {
            Ok(v) if v.is_empty() => {
                println!("site-gen: SCSS uses tokens only");
                ExitCode::SUCCESS
            }
            Ok(v) => {
                eprintln!("site-gen: raw design values outside the token layer:\n{}", v.join("\n"));
                ExitCode::FAILURE
            }
            Err(e) => {
                eprintln!("site-gen: {e}");
                ExitCode::FAILURE
            }
        };
    }

    match build(&paths, Date::today()) {
        Ok(r) => {
            for w in &r.warnings {
                eprintln!("site-gen: warning: {w}");
            }
            println!(
                "site-gen: {} pages, {} files in {} (css {} B, js {} B, brand files {})",
                r.pages,
                r.files,
                paths.out.display(),
                r.css_bytes,
                r.js_bytes,
                r.brand_files
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("site-gen: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage(problem: &str) -> ExitCode {
    if !problem.is_empty() {
        eprintln!("site-gen: {problem}");
    }
    eprintln!("usage: site-gen [--out <dir>] [--repo <root>] [--check]");
    if problem.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(2) }
}
