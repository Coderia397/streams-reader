//! `cargo xtask`: builds, deploys and operates nolabs-reader through the Cloudflare and Hugging Face HTTP APIs, with
//! no Node and no wrangler (decision D3). Run `cargo run -p xtask -- --help`.
//!
//! Credentials are read from `~/.config/streams/*.env` and `~/.config/nolabs/*.env` at runtime, held in
//! [`secret::Secret`], and only ever sent in request headers, request bodies or a child process's environment.

mod api;
mod cf;
mod cmd;
mod envfile;
mod hf;
mod lifecycle;
mod migrations;
mod names;
mod paths;
mod secret;
mod secrets_gen;
mod state;
mod timefmt;
mod worker_bundle;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "xtask", version, about = "Build, deploy and operate nolabs-reader (no Node, no wrangler)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Mint the scoped Cloudflare token "nolabs-deploy" from the Global API Key (the only command that reads it).
    BootstrapToken {
        /// Replace an existing token file and revoke the older nolabs-deploy tokens.
        #[arg(long)]
        rotate: bool,
    },
    /// Ensure D1 "nolabs", Queue "nolabs-render" and a private R2 bucket "nolabs-media" with the audio lifecycle rule.
    Setup,
    /// Apply migrations/NNNN_*.sql to D1 in order, tracked in the _migrations ledger.
    Migrate {
        /// Show what would be applied without changing the database.
        #[arg(long)]
        dry_run: bool,
    },
    /// Create (or reuse) the login passphrase and session key in ~/.config/nolabs/app.env.
    Passphrase {
        /// Generate a new passphrase even if one exists (the session key is kept).
        #[arg(long)]
        rotate: bool,
    },
    /// Build the web app (trunk) and the Worker (worker-build) in release mode and report sizes.
    Build,
    /// Upload the Worker "nolabs-api" with its bindings, secrets, queue consumer, cron and workers.dev URL.
    Deploy {
        /// Kokoro revision (commit sha) to record in KOKORO_REV; default: the model's current main.
        #[arg(long)]
        kokoro_rev: Option<String>,
        /// Ming revision (commit sha) to record in MING_REV; default: the model's current main.
        #[arg(long)]
        ming_rev: Option<String>,
    },
    /// Upload space/ to the Hugging Face Space and wait until the new build is RUNNING.
    Space {
        /// Return right after the upload.
        #[arg(long)]
        no_wait: bool,
    },
    /// End-to-end check against the deployed API: login, import, render, fetch audio, status, delete.
    Smoke {
        /// API origin; default: the URL recorded by `deploy` in deploy/state.toml.
        #[arg(long)]
        base: Option<String>,
    },
    /// Show the Space runtime and the app's /api/status.
    Status {
        /// API origin; default: the URL recorded by `deploy` in deploy/state.toml.
        #[arg(long)]
        base: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::BootstrapToken { rotate } => cmd::bootstrap::run(rotate),
        Command::Setup => cmd::setup::run(),
        Command::Migrate { dry_run } => cmd::migrate::run(dry_run),
        Command::Passphrase { rotate } => cmd::passphrase::run(rotate),
        Command::Build => cmd::build::run(),
        Command::Deploy { kokoro_rev, ming_rev } => cmd::deploy::run(cmd::deploy::Options { kokoro_rev, ming_rev }),
        Command::Space { no_wait } => cmd::space::run(!no_wait),
        Command::Smoke { base } => cmd::smoke::run(base),
        Command::Status { base } => cmd::status::run(base),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use clap::CommandFactory as _;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_subcommands() {
        let c = Cli::try_parse_from(["xtask", "migrate", "--dry-run"]).unwrap();
        assert!(matches!(c.command, Command::Migrate { dry_run: true }));
        let c = Cli::try_parse_from(["xtask", "smoke", "--base", "https://x.dev"]).unwrap();
        assert!(matches!(c.command, Command::Smoke { base: Some(ref b) } if b == "https://x.dev"));
        let c = Cli::try_parse_from(["xtask", "bootstrap-token"]).unwrap();
        assert!(matches!(c.command, Command::BootstrapToken { rotate: false }));
        assert!(Cli::try_parse_from(["xtask", "nope"]).is_err());
    }
}
