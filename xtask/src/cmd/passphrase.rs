//! `passphrase`: creates or reuses the login passphrase and the session key in `~/.config/nolabs/app.env` and stores
//! the hash the Worker checks logins against. Prints nothing secret: the passphrase is read from the file.

use anyhow::{Result, bail};

use crate::envfile::{self, EnvFile};
use crate::paths::{self, tilde};
use crate::secret::Secret;
use crate::secrets_gen::{
    generate_passphrase, generate_session_key, is_valid_passphrase, is_valid_session_key, passphrase_hash,
};

pub fn run(rotate: bool) -> Result<()> {
    let path = paths::app_env()?;
    let existing = EnvFile::load_optional(&path)?;

    let passphrase = match existing.as_ref().and_then(|e| e.get("PASSPHRASE")) {
        Some(p) if !rotate => {
            if !is_valid_passphrase(p.expose()) {
                bail!("PASSPHRASE in {} is malformed; pass --rotate to replace it", tilde(&path));
            }
            p.clone()
        }
        _ => generate_passphrase()?,
    };
    let session_key = match existing.as_ref().and_then(|e| e.get("SESSION_KEY")) {
        Some(k) => {
            if !is_valid_session_key(k.expose()) {
                bail!("SESSION_KEY in {} is not 64 lowercase hex chars", tilde(&path));
            }
            k.clone()
        }
        None => generate_session_key()?,
    };
    let hash: Secret = passphrase_hash(&session_key, &passphrase)?;

    let content = envfile::render(
        "nolabs login secrets. Written by `cargo xtask passphrase`; never commit.\n\
         PASSPHRASE is what you type to log in; SESSION_KEY and PASSPHRASE_HASH become Worker secrets.",
        &[
            ("PASSPHRASE", passphrase.expose()),
            ("SESSION_KEY", session_key.expose()),
            ("PASSPHRASE_HASH", hash.expose()),
        ],
    )?;
    envfile::write_private(&path, &content)?;
    println!("passphrase stored in {}", tilde(&path));
    Ok(())
}
