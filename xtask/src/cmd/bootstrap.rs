//! `bootstrap-token`: the only code that reads the Cloudflare Global API Key. It mints the scoped `nolabs-deploy`
//! token (90 days) and writes it to `~/.config/nolabs/cloudflare-deploy.env`; every other command uses that token.

use anyhow::{Context as _, Result, bail};
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::cf::{Auth, Body, Cf};
use crate::envfile::{self, EnvFile};
use crate::names::DEPLOY_TOKEN;
use crate::paths::{self, tilde};
use crate::secret::Secret;
use crate::timefmt;

pub const TOKEN_DAYS: i64 = 90;
const ACCOUNT_SCOPE: &str = "com.cloudflare.api.account";
const ZONE_SCOPE: &str = "com.cloudflare.api.account.zone";

/// Account-level permissions: Worker script, R2, D1, Queues, tail, and reading the workers.dev subdomain.
pub const ACCOUNT_PERMISSIONS: &[&str] = &[
    "Workers Scripts Write",
    "Workers R2 Storage Write",
    "D1 Write",
    "Queues Write",
    "Workers Tail Read",
    "Account Settings Read",
];
/// Zone-level permissions on `CF_ZONE`, for the custom domain later (decision D2).
pub const ZONE_PERMISSIONS: &[&str] = &["Workers Routes Write", "DNS Write"];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PermissionGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Deserialize)]
struct Minted {
    id: String,
    value: String,
}

#[derive(Deserialize)]
struct TokenSummary {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct Verify {
    status: String,
}

/// Ids of the named groups that apply at `scope`. Fails listing every name it could not find.
pub fn select_groups(groups: &[PermissionGroup], names: &[&str], scope: &str) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    let mut missing = Vec::new();
    for name in names {
        match groups.iter().find(|g| g.name == *name && g.scopes.iter().any(|s| s == scope)) {
            Some(g) => ids.push(g.id.clone()),
            None => missing.push(*name),
        }
    }
    if !missing.is_empty() {
        bail!("Cloudflare has no permission group(s) {missing:?} at scope {scope}");
    }
    Ok(ids)
}

pub fn token_request(
    account: &str,
    zone: &str,
    account_ids: &[String],
    zone_ids: &[String],
    expires_on: &str,
) -> Value {
    let groups = |ids: &[String]| ids.iter().map(|id| json!({ "id": id })).collect::<Vec<_>>();
    json!({
        "name": DEPLOY_TOKEN,
        "policies": [
            {
                "effect": "allow",
                "resources": { format!("{ACCOUNT_SCOPE}.{account}"): "*" },
                "permission_groups": groups(account_ids),
            },
            {
                "effect": "allow",
                "resources": { format!("{ZONE_SCOPE}.{zone}"): "*" },
                "permission_groups": groups(zone_ids),
            },
        ],
        "expires_on": expires_on,
    })
}

pub fn run(rotate: bool) -> Result<()> {
    let out_path = paths::deploy_env()?;
    if out_path.exists() && !rotate {
        bail!("{} already exists; pass --rotate to mint a replacement and revoke the old token", tilde(&out_path));
    }

    let global = EnvFile::load(&paths::cloudflare_global_env()?)?;
    let account = global.plain("CF_ACCOUNT")?;
    let zone = global.plain("CF_ZONE")?;
    let auth = Auth::GlobalKey { email: global.secret("CF_EMAIL")?.clone(), key: global.secret("CF_KEY")?.clone() };
    let cf = Cf::new(auth, account.clone())?;

    let groups: Vec<PermissionGroup> = cf.get("/user/tokens/permission_groups")?;
    let account_ids = select_groups(&groups, ACCOUNT_PERMISSIONS, ACCOUNT_SCOPE)?;
    let zone_ids = select_groups(&groups, ZONE_PERMISSIONS, ZONE_SCOPE)?;
    let expires_on = timefmt::rfc3339(timefmt::now_unix() + TOKEN_DAYS * 86_400);
    let body = token_request(&account, &zone, &account_ids, &zone_ids, &expires_on);

    let minted: Minted = cf.mutate(DEPLOY_TOKEN, Method::POST, "/user/tokens", Body::Json(body))?;
    let token = Secret::new(minted.value);

    let written = envfile::render(
        "Cloudflare API token for nolabs (scoped). Written by `cargo xtask bootstrap-token`; never commit.",
        &[
            ("CF_API_TOKEN", token.expose()),
            ("CF_ACCOUNT_ID", &account),
            ("CF_TOKEN_ID", &minted.id),
            ("CF_TOKEN_EXPIRES_ON", &expires_on),
        ],
    )
    .and_then(|content| envfile::write_private(&out_path, &content));
    if let Err(e) = written {
        // Do not leave a live token nobody holds.
        let revoked: Result<Value> =
            cf.mutate(DEPLOY_TOKEN, Method::DELETE, &format!("/user/tokens/{}", minted.id), Body::Empty);
        return Err(e).context(match revoked {
            Ok(_) => "the new token was revoked again".to_owned(),
            Err(_) => format!("the new token {} could not be revoked; delete it in the dashboard", minted.id),
        });
    }

    let check = Cf::new(Auth::Token(token), account)?;
    let verify: Verify = check.get("/user/tokens/verify")?;
    if verify.status != "active" {
        bail!("new token {} reports status {:?}", minted.id, verify.status);
    }

    if rotate {
        let tokens: Vec<TokenSummary> = cf.get_all("/user/tokens")?;
        for old in tokens.iter().filter(|t| t.name == DEPLOY_TOKEN && t.id != minted.id) {
            let _: Value = cf.mutate(DEPLOY_TOKEN, Method::DELETE, &format!("/user/tokens/{}", old.id), Body::Empty)?;
            println!("revoked old {DEPLOY_TOKEN} token {}", old.id);
        }
    }

    println!("minted {DEPLOY_TOKEN} token {} (expires {expires_on}); stored in {}", minted.id, tilde(&out_path));
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn group(id: &str, name: &str, scope: &str) -> PermissionGroup {
        PermissionGroup { id: id.into(), name: name.into(), scopes: vec![scope.into()] }
    }

    fn catalogue() -> Vec<PermissionGroup> {
        let mut g: Vec<PermissionGroup> =
            ACCOUNT_PERMISSIONS.iter().enumerate().map(|(i, n)| group(&format!("a{i}"), n, ACCOUNT_SCOPE)).collect();
        g.extend(ZONE_PERMISSIONS.iter().enumerate().map(|(i, n)| group(&format!("z{i}"), n, ZONE_SCOPE)));
        // Same name at another scope must not be picked up.
        g.push(group("wrong", "DNS Write", "com.cloudflare.api.user"));
        g
    }

    #[test]
    fn selects_by_name_and_scope() {
        let g = catalogue();
        assert_eq!(
            select_groups(&g, ACCOUNT_PERMISSIONS, ACCOUNT_SCOPE).unwrap(),
            ["a0", "a1", "a2", "a3", "a4", "a5"]
        );
        assert_eq!(select_groups(&g, ZONE_PERMISSIONS, ZONE_SCOPE).unwrap(), ["z0", "z1"]);
    }

    #[test]
    fn missing_groups_are_all_listed() {
        let err = select_groups(&catalogue(), &["D1 Write", "Nope One", "Nope Two"], ACCOUNT_SCOPE).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Nope One") && msg.contains("Nope Two") && !msg.contains("D1 Write"), "{msg}");
        // An account-scope name asked for at zone scope is missing.
        assert!(select_groups(&catalogue(), &["D1 Write"], ZONE_SCOPE).is_err());
    }

    #[test]
    fn token_body_shape() {
        let body = token_request("acc1", "zone1", &["a".into(), "b".into()], &["z".into()], "2027-01-28T00:00:00Z");
        assert_eq!(body["name"], "nolabs-deploy");
        assert_eq!(body["expires_on"], "2027-01-28T00:00:00Z");
        let p = body["policies"].as_array().unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0]["resources"], json!({"com.cloudflare.api.account.acc1": "*"}));
        assert_eq!(p[0]["permission_groups"], json!([{"id":"a"},{"id":"b"}]));
        assert_eq!(p[1]["resources"], json!({"com.cloudflare.api.account.zone.zone1": "*"}));
        assert_eq!(p[1]["permission_groups"], json!([{"id":"z"}]));
        assert!(p.iter().all(|x| x["effect"] == "allow"));
    }
}
