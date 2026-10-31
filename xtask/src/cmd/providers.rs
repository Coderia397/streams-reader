//! `auth-provider`: turn an OAuth sign-in provider on (or off) in the Supabase project. The client id is public and
//! comes from argv; the client secret is read from an env file (`~/.config/nolabs/oauth.env` by default) so it never
//! appears in the process table, the shell history or the output.
//!
//! Every provider console needs the same redirect (callback) URI: [`CALLBACK_URL`].
//! - Google: Google Cloud console, OAuth client of type "Web application", authorised redirect URI = callback.
//! - Microsoft: Entra admin centre, app registration with "Accounts in any organizational directory and personal
//!   Microsoft accounts", web redirect URI = callback; the tenant URL is `common` so personal accounts work.
//! - Apple: needs a paid Apple Developer membership. Services ID (the client id; add the iOS/macOS bundle id after a
//!   comma for native sign-in), return URL = callback; the "secret" is a JWT signed with the .p8 key that Apple
//!   limits to 6 months, so it must be regenerated and re-applied before it expires.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::ValueEnum;
use serde_json::{Value, json};

use crate::envfile::EnvFile;
use crate::names::SUPABASE_REF;
use crate::paths::{self, tilde};
use crate::secret::Secret;
use crate::supabase::Management;

pub const CALLBACK_URL: &str = "https://atxbmgfuxjtcwmoaldqy.supabase.co/auth/v1/callback";
/// Microsoft's multi-tenant endpoint: work, school and personal accounts.
pub const AZURE_COMMON_URL: &str = "https://login.microsoftonline.com/common";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Provider {
    Google,
    /// Microsoft accounts (Supabase calls the provider `azure`).
    Azure,
    Apple,
}

impl Provider {
    fn key(self) -> &'static str {
        match self {
            Provider::Google => "google",
            Provider::Azure => "azure",
            Provider::Apple => "apple",
        }
    }

    /// The env-file entry holding this provider's client secret.
    pub fn secret_var(self) -> &'static str {
        match self {
            Provider::Google => "GOOGLE_CLIENT_SECRET",
            Provider::Azure => "AZURE_CLIENT_SECRET",
            Provider::Apple => "APPLE_CLIENT_SECRET",
        }
    }
}

/// The auth-config PATCH body that enables a provider.
pub fn enable_body(provider: Provider, client_id: &str, secret: &Secret) -> Value {
    let p = provider.key();
    let mut body = json!({
        format!("external_{p}_enabled"): true,
        format!("external_{p}_client_id"): client_id,
        format!("external_{p}_secret"): secret.expose(),
    });
    if provider == Provider::Azure {
        body["external_azure_url"] = Value::String(AZURE_COMMON_URL.to_owned());
    }
    body
}

pub fn disable_body(provider: Provider) -> Value {
    json!({ format!("external_{}_enabled", provider.key()): false })
}

pub fn run(provider: Provider, client_id: Option<&str>, secret_from: Option<PathBuf>, disable: bool) -> Result<()> {
    let mgmt = Management::from_env()?;
    if disable {
        mgmt.patch_auth_config(&disable_body(provider))?;
        println!("{}: disabled", provider.key());
        return Ok(());
    }
    let Some(client_id) = client_id.map(str::trim).filter(|c| !c.is_empty()) else {
        bail!("--client-id is required");
    };
    let path = match secret_from {
        Some(p) => p,
        None => paths::oauth_env()?,
    };
    let secret = EnvFile::load(&path)?.secret(provider.secret_var())?.clone();
    mgmt.patch_auth_config(&enable_body(provider, client_id, &secret))?;
    let now = mgmt.auth_config()?;
    if now.get(format!("external_{}_enabled", provider.key())) != Some(&Value::Bool(true)) {
        bail!("{} did not turn on; check the client id and the secret in {}", provider.key(), tilde(&path));
    }
    println!("{}: enabled for project {SUPABASE_REF}; console redirect URI: {CALLBACK_URL}", provider.key());
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn bodies_name_the_supabase_fields() {
        let b = enable_body(Provider::Google, "id-1", &Secret::new("s"));
        assert_eq!(
            b,
            json!({"external_google_enabled": true, "external_google_client_id": "id-1", "external_google_secret": "s"})
        );
        let b = enable_body(Provider::Azure, "id-2", &Secret::new("s"));
        assert_eq!(b["external_azure_url"], AZURE_COMMON_URL);
        assert_eq!(disable_body(Provider::Apple), json!({"external_apple_enabled": false}));
        assert_eq!(Provider::Apple.secret_var(), "APPLE_CLIENT_SECRET");
    }

    #[test]
    fn callback_belongs_to_our_project() {
        assert!(CALLBACK_URL.contains(SUPABASE_REF));
    }
}
