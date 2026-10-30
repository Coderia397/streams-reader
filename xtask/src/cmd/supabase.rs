//! Supabase commands: `supabase-migrate` (Postgres migrations through the Management API, tracked in
//! `private.nolabs_migrations`), `supabase-auth` (sign-in settings and the two auth hooks), `supabase-owner` (create
//! or promote the admin account) and `invite` (an invite code for a new user).

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::envfile;
use crate::envfile::EnvFile;
use crate::migrations::{self, Applied};
use crate::names::{SITE_URL, send_email_hook_url};
use crate::paths::{self, tilde};
use crate::secret::Secret;
use crate::supabase::{Management, Project, sql_string};
use nolabs_core::brand;

pub const LEDGER_DDL: &str = "create schema if not exists private;\n\
     create table if not exists private.nolabs_migrations (\n  id integer primary key,\n  name text not null,\n  \
     sha256 text not null,\n  applied_at timestamptz not null default now()\n);\n\
     alter table private.nolabs_migrations enable row level security;\n\
     alter table private.nolabs_migrations force row level security;\n\
     revoke all on table private.nolabs_migrations from public, anon, authenticated;";
pub const LEDGER_SELECT: &str = "select id, name, sha256 from private.nolabs_migrations order by id;";
const LEDGER_EXISTS: &str = "select to_regclass('private.nolabs_migrations') is not null as present;";

/// Where Supabase may send people back after a magic link or OAuth: the site, local development servers, the Tauri
/// apps and their deep-link scheme. Built from the brand constants, so a rename changes them in one place.
pub fn redirect_urls() -> Vec<String> {
    let scheme = brand::APP_SCHEME;
    vec![
        format!("{SITE_URL}/**"),
        "http://localhost:*/**".to_owned(),
        "tauri://localhost".to_owned(),
        "tauri://localhost/**".to_owned(),
        format!("{scheme}://auth"),
        format!("{scheme}://auth/**"),
    ]
}
pub const BEFORE_USER_CREATED_HOOK: &str = "pg-functions://postgres/private/hook_before_user_created";
/// Sign-in codes: 6 digits (typed on a phone), valid 10 minutes (the emails say so).
pub const OTP_LENGTH: u32 = 6;
pub const OTP_SECONDS: u32 = 600;

/// One request body per migration: the file and its ledger row in one explicit transaction.
pub fn apply_sql(m: &migrations::Migration) -> String {
    format!(
        "begin;\n{}\ninsert into private.nolabs_migrations (id, name, sha256) values ({}, {}, {});\ncommit;",
        m.sql.trim_end(),
        m.id,
        sql_string(&m.name),
        sql_string(&m.sha256),
    )
}

pub fn migrate(dry_run: bool) -> Result<()> {
    let local = migrations::load_dir(&paths::supabase_migrations_dir())?;
    let mgmt = Management::from_env()?;
    let present = mgmt.query(LEDGER_EXISTS)?.first().and_then(|r| r.get("present")).and_then(Value::as_bool);
    if !dry_run {
        // Idempotent; also brings an older ledger table up to the RLS rule the check enforces.
        mgmt.query(LEDGER_DDL)?;
    }
    let applied: Vec<Applied> = if present == Some(true) || !dry_run {
        mgmt.query(LEDGER_SELECT)?
            .into_iter()
            .map(|r| serde_json::from_value(r).context("unexpected ledger row"))
            .collect::<Result<_>>()?
    } else {
        Vec::new()
    };
    let pending = migrations::plan(&local, &applied)?;
    println!("supabase: {} applied, {} pending, {} local", applied.len(), pending.len(), local.len());
    for m in pending {
        if dry_run {
            println!("would apply {} (sha256 {})", m.name, m.sha256.get(..12).unwrap_or(&m.sha256));
            continue;
        }
        mgmt.query(&apply_sql(m)).with_context(|| format!("applying {}", m.name))?;
        println!("applied {}", m.name);
    }
    if !dry_run {
        check_with(&mgmt)?;
    }
    Ok(())
}

/// Each query returns the rows that break a rule; an empty answer means the rule holds. These are the defects found
/// in an earlier project of the owner's (definer functions callable anonymously, tables without RLS).
pub const CHECKS: &[(&str, &str)] = &[
    (
        "tables without row-level security enabled and forced",
        "select n.nspname || '.' || c.relname as problem from pg_class c join pg_namespace n on n.oid = c.relnamespace \
         where c.relkind in ('r', 'p') and n.nspname in ('public', 'private') \
         and not (c.relrowsecurity and c.relforcerowsecurity)",
    ),
    (
        "SECURITY DEFINER functions in the exposed public schema",
        "select p.oid::regprocedure::text as problem from pg_proc p join pg_namespace n on n.oid = p.pronamespace \
         where n.nspname = 'public' and p.prosecdef",
    ),
    (
        "functions anon may execute beyond the sign-in RPCs",
        "select p.oid::regprocedure::text as problem from pg_proc p join pg_namespace n on n.oid = p.pronamespace \
         where n.nspname in ('public', 'private') and has_function_privilege('anon', p.oid, 'execute') \
         and p.proname not in ('invite_status', 'reserve_invite', 'normalise_invite')",
    ),
    (
        "private functions authenticated may execute beyond what the public wrappers need",
        "select p.oid::regprocedure::text as problem from pg_proc p join pg_namespace n on n.oid = p.pronamespace \
         where n.nspname = 'private' and has_function_privilege('authenticated', p.oid, 'execute') \
         and p.proname not in ('invite_status', 'reserve_invite', 'normalise_invite', 'gpu_share_of')",
    ),
    (
        "tables or views anon can read or write",
        "select n.nspname || '.' || c.relname as problem from pg_class c join pg_namespace n on n.oid = c.relnamespace \
         where c.relkind in ('r', 'p', 'v') and n.nspname in ('public', 'private') \
         and has_table_privilege('anon', c.oid, 'select, insert, update, delete')",
    ),
];

/// Advisor findings accepted on purpose, with the reason printed next to them.
pub const ACCEPTED_LINTS: &[(&str, &str)] = &[(
    "auth_leaked_password_protection",
    "HaveIBeenPwned checks need the Pro plan; no account can hold a password (trigger private.no_passwords)",
)];

pub fn check() -> Result<()> {
    check_with(&Management::from_env()?)
}

/// Run [`CHECKS`] and both advisors; fail on any broken rule or any advisor warning not in [`ACCEPTED_LINTS`].
fn check_with(mgmt: &Management) -> Result<()> {
    let mut failures = Vec::new();
    for (rule, sql) in CHECKS {
        let names: Vec<String> = mgmt
            .query(sql)?
            .iter()
            .filter_map(|r| r.get("problem").and_then(Value::as_str))
            .map(str::to_owned)
            .collect();
        if names.is_empty() {
            println!("ok    no {rule}");
        } else {
            failures.push(format!("{rule}: {}", names.join(", ")));
        }
    }
    for kind in ["security", "performance"] {
        for lint in mgmt.advisors(kind)? {
            let name = lint.get("name").and_then(Value::as_str).unwrap_or("?");
            let level = lint.get("level").and_then(Value::as_str).unwrap_or("?");
            let detail: String = lint.get("detail").and_then(Value::as_str).unwrap_or("").chars().take(160).collect();
            if let Some((_, why)) = ACCEPTED_LINTS.iter().find(|(n, _)| *n == name) {
                println!("note  {kind} {level} {name}: accepted ({why})");
            } else if level == "INFO" {
                println!("info  {kind} {name}: {detail}");
            } else {
                failures.push(format!("{kind} advisor {level} {name}: {detail}"));
            }
        }
    }
    if !failures.is_empty() {
        bail!("supabase check failed:\n  {}", failures.join("\n  "));
    }
    println!("supabase check passed");
    Ok(())
}

/// A new Standard Webhooks secret: `v1,whsec_` + base64 of 32 random bytes.
pub fn generate_hook_secret() -> Result<Secret> {
    use base64::Engine as _;
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no OS randomness: {e}"))?;
    let secret = Secret::new(format!("v1,whsec_{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
    zeroize::Zeroize::zeroize(&mut bytes);
    Ok(secret)
}

/// The Send Email hook secret from the project file, generated and appended to it on first use.
pub fn hook_secret() -> Result<Secret> {
    let path = paths::supabase_project_env()?;
    if let Some(s) = EnvFile::load(&path)?.get("SEND_EMAIL_HOOK_SECRET") {
        return Ok(s.clone());
    }
    let secret = generate_hook_secret()?;
    envfile::append_private(&path, "SEND_EMAIL_HOOK_SECRET", secret.expose())?;
    println!("generated SEND_EMAIL_HOOK_SECRET in {}", tilde(&path));
    Ok(secret)
}

/// The auth settings this app needs, as a Management API PATCH body.
pub fn auth_settings(hook_secret: &Secret) -> Value {
    json!({
        "site_url": SITE_URL,
        "uri_allow_list": redirect_urls().join(","),
        "external_email_enabled": true,
        "disable_signup": false,
        "mailer_autoconfirm": false,
        "mailer_otp_length": OTP_LENGTH,
        "mailer_otp_exp": OTP_SECONDS,
        "rate_limit_email_sent": 30,
        "hook_send_email_enabled": true,
        "hook_send_email_uri": send_email_hook_url(),
        "hook_send_email_secrets": hook_secret.expose(),
        "hook_before_user_created_enabled": true,
        "hook_before_user_created_uri": BEFORE_USER_CREATED_HOOK,
        // Automatic linking (same verified email → same account) is always on in Supabase; manual linking lets a
        // signed-in user attach an identity with another email, such as an Apple private-relay address.
        "security_manual_linking_enabled": true,
    })
}

pub fn auth() -> Result<()> {
    let mgmt = Management::from_env()?;
    let secret = hook_secret()?;
    mgmt.patch_auth_config(&auth_settings(&secret))?;
    let now = mgmt.auth_config()?;
    let mut problems = Vec::new();
    for (key, want) in auth_settings(&secret).as_object().into_iter().flatten() {
        if key.ends_with("_secrets") {
            continue;
        }
        if now.get(key) != Some(want) {
            problems.push(format!("{key}: wanted {want}, project has {}", now.get(key).unwrap_or(&Value::Null)));
        }
    }
    if !problems.is_empty() {
        bail!("auth settings did not stick:\n  {}", problems.join("\n  "));
    }
    let hook = send_email_hook_url();
    println!("auth: site {SITE_URL}, {OTP_LENGTH}-digit codes for {} min, send-email hook {hook}", OTP_SECONDS / 60);
    println!("auth: sign-ups need an invite (before-user-created hook {BEFORE_USER_CREATED_HOOK})");
    Ok(())
}

/// Insert an invite created by the first admin; returns the code.
/// With `email`, only that address can use it, and OAuth sign-ups (Google, Microsoft) with that verified address
/// are accepted without typing the code.
pub fn create_invite(mgmt: &Management, days: u32, note: Option<&str>, email: Option<&str>) -> Result<String> {
    if !(1..=90).contains(&days) {
        bail!("--days must be between 1 and 90");
    }
    let note = note.map_or_else(|| "null".to_owned(), sql_string);
    let email = email.map(|e| e.trim().to_lowercase()).map_or_else(|| "null".to_owned(), |e| sql_string(&e));
    let sql = format!(
        "insert into public.invites (created_by, note, email, expires_at) values (\
         (select id from public.profiles where is_admin order by created_at limit 1), {note}, {email}, \
         now() + make_interval(days => {days})) returning code;"
    );
    mgmt.query(&sql)?
        .first()
        .and_then(|r| r.get("code"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .context("no invite code returned")
}

pub fn delete_invite(mgmt: &Management, code: &str) -> Result<()> {
    mgmt.query(&format!("delete from public.invites where code = {};", sql_string(code))).map(drop)
}

pub fn invite(days: u32, note: Option<String>, email: Option<String>) -> Result<()> {
    let mgmt = Management::from_env()?;
    let code = create_invite(&mgmt, days, note.as_deref(), email.as_deref())?;
    println!("invite code: {code} (valid {days} days, works once)");
    match email {
        Some(e) => println!("bound to {e}: that address can also sign up with Google or Microsoft without the code"),
        None => println!("the new user enters it on the sign-in screen with their email"),
    }
    Ok(())
}

/// Create the admin account (confirmed, signs in with email codes) or promote an existing one, and give it the rest
/// of the GPU budget (`daily_gpu_s = null`).
pub fn owner(email: &str) -> Result<()> {
    if !email.contains('@') || email.chars().any(char::is_whitespace) {
        bail!("not an email address");
    }
    let mut mgmt = Management::from_env()?;
    let keys = mgmt.keys()?;
    let project = Project::new(&mgmt.project_url, keys.publishable.clone())?;
    let user_id = match project.admin_find_user(&keys.service_role, email)? {
        Some(id) => {
            println!("account exists: {id}");
            id
        }
        None => {
            let code = create_invite(&mgmt, 1, Some("owner account"), Some(email))?;
            let id = project.admin_create_user(&keys.service_role, email, None, &json!({ "invite_code": code }))?;
            println!("account created: {id}");
            id
        }
    };
    let id = sql_string(&user_id);
    // The owner may always create an account again (for example through OAuth after deleting it).
    mgmt.query(&format!(
        "insert into private.allowed_emails (email) values ({}) on conflict do nothing;",
        sql_string(&email.trim().to_lowercase())
    ))?;
    mgmt.query(&format!(
        "update public.profiles set is_admin = true where id = {id}::uuid; \
         insert into public.budget_shares (user_id, daily_gpu_s) values ({id}::uuid, null) \
         on conflict (user_id) do update set daily_gpu_s = null;"
    ))?;
    println!("admin: yes, GPU share: the rest of the daily cap");
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use nolabs_core::cache_key::sha256_hex;

    #[test]
    fn migration_and_ledger_row_share_one_transaction() {
        let m = migrations::Migration {
            id: 3,
            name: "0003_x.sql".into(),
            sha256: sha256_hex(b"select 1;"),
            sql: "select 1;\n\n".into(),
        };
        let sql = apply_sql(&m);
        assert!(sql.starts_with("begin;\nselect 1;\ninsert into private.nolabs_migrations"), "{sql}");
        assert!(sql.contains(&format!("values (3, '0003_x.sql', '{}')", m.sha256)));
        assert!(sql.ends_with("commit;"));
    }

    #[test]
    fn hook_secrets_have_the_standard_webhooks_shape() {
        use base64::Engine as _;
        let s = generate_hook_secret().unwrap();
        let b64 = s.expose().strip_prefix("v1,whsec_").unwrap();
        assert_eq!(base64::engine::general_purpose::STANDARD.decode(b64).unwrap().len(), 32);
        assert_ne!(s, generate_hook_secret().unwrap());
    }

    #[test]
    fn auth_settings_cover_codes_redirects_and_hooks() {
        let v = auth_settings(&Secret::new("v1,whsec_AAAA"));
        assert_eq!(v["mailer_otp_length"], 6);
        assert_eq!(v["site_url"], "https://listen.streams.watch");
        let list = v["uri_allow_list"].as_str().unwrap();
        for url in ["https://listen.streams.watch/**", "http://localhost:*/**", "tauri://localhost", "nolabs://auth"] {
            assert!(list.split(',').any(|u| u == url), "{url}");
        }
        assert_eq!(v["hook_send_email_uri"], "https://listen.streams.watch/hooks/auth-email");
        assert_eq!(v["hook_before_user_created_uri"], BEFORE_USER_CREATED_HOOK);
    }

    #[test]
    fn supabase_migrations_are_well_named() {
        let local = migrations::load_dir(&paths::supabase_migrations_dir()).unwrap();
        assert!(local.first().is_some_and(|m| m.id == 1));
    }
}
