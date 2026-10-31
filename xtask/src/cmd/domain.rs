//! `domain`: attaches one of this project's hostnames to the Worker as a Workers Custom Domain (Cloudflare creates the
//! DNS record and certificate). Only the hostnames in [`ALLOWED`] are accepted, so this command can never touch the
//! rest of the `streams.watch` zone.

use anyhow::{Result, bail};
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use crate::cf::{Body, Cf};
use crate::envfile::EnvFile;
use crate::names::WORKER;
use crate::paths;

/// Canonical address first (decision D34); the second stays as a redirect to it.
pub const ALLOWED: &[&str] = &["reader.streams.watch", "listen.streams.watch"];

#[derive(Deserialize)]
struct Attached {
    hostname: String,
    service: String,
}

pub fn check(hostname: &str) -> Result<()> {
    if !ALLOWED.contains(&hostname) {
        bail!("{hostname} is not one of this project's hostnames ({})", ALLOWED.join(", "));
    }
    Ok(())
}

pub fn run(hostname: &str) -> Result<()> {
    check(hostname)?;
    // The zone id is an identifier, not a credential; it lives next to the Global key but is read as plain text.
    let zone = EnvFile::load(&paths::cloudflare_global_env()?)?.plain("CF_ZONE")?;
    let cf = Cf::from_deploy_env()?;
    let body = json!({ "hostname": hostname, "service": WORKER, "zone_id": zone, "environment": "production" });
    let attached: Attached = cf.mutate(WORKER, Method::PUT, &cf.acct("/workers/domains"), Body::Json(body))?;
    println!("{} → Worker {}", attached.hostname, attached.service);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_project_hostnames_are_accepted() {
        assert!(check("reader.streams.watch").is_ok());
        assert!(check("listen.streams.watch").is_ok());
        assert!(check("streams.watch").is_err());
        assert!(check("www.streams.watch").is_err());
        assert!(check("reader.streams.watch.evil.example").is_err());
    }
}
