//! Supabase clients for xtask: the Management API (SQL, API keys, auth settings) with the personal access token in
//! `~/.config/streams/supabase.env`, and the project's own Auth Admin API with the service-role key fetched through
//! the Management API. The service-role key exists only in this process's memory: it is never written to disk and
//! never deployed (the Worker only ever holds the publishable key).
//!
//! The access token can reach every project of the account, including other apps' projects, so every request is
//! built from [`SUPABASE_REF`] and the project file must name the same ref; nothing here takes a ref from outside.

use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use reqwest::Method;
use reqwest::blocking::{Client, RequestBuilder};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::cf::http_client;
use crate::envfile::EnvFile;
use crate::names::SUPABASE_REF;
use crate::paths::{self, tilde};
use crate::secret::{Redactor, Secret};

const MANAGEMENT_API: &str = "https://api.supabase.com/v1";

/// The Management API scoped to this project.
pub struct Management {
    http: Client,
    token: Secret,
    redactor: Redactor,
    /// `https://<ref>.supabase.co`, from the project file.
    pub project_url: String,
}

/// The project's API keys this tool uses. `publishable` is public by design (it ships in apps).
pub struct Keys {
    pub publishable: Secret,
    pub service_role: Secret,
}

#[derive(Deserialize)]
struct ApiKey {
    #[serde(default)]
    name: String,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
}

impl Management {
    pub fn from_env() -> Result<Self> {
        let token = EnvFile::load(&paths::supabase_token_env()?)?.secret("SUPABASE_ACCESS_TOKEN")?.clone();
        let project_path = paths::supabase_project_env()?;
        let project = EnvFile::load(&project_path)?;
        let file_ref = project.plain("SUPABASE_PROJECT_REF")?;
        if file_ref != SUPABASE_REF {
            bail!("{} names project {file_ref}, but this tool only manages {SUPABASE_REF}", tilde(&project_path));
        }
        let project_url = project.plain("SUPABASE_URL")?.trim_end_matches('/').to_owned();
        if project_url != format!("https://{SUPABASE_REF}.supabase.co") {
            bail!("SUPABASE_URL in {} does not belong to project {SUPABASE_REF}", tilde(&project_path));
        }
        let redactor = Redactor::new().with(&token);
        Ok(Self { http: http_client(Duration::from_secs(120))?, token, redactor, project_url })
    }

    fn url(rest: &str) -> String {
        format!("{MANAGEMENT_API}/projects/{SUPABASE_REF}{rest}")
    }

    fn send(&self, method: Method, rest: &str, body: Option<&Value>) -> Result<Value> {
        let mut req = self.http.request(method.clone(), Self::url(rest)).bearer_auth(self.token.expose());
        if let Some(b) = body {
            req = req.json(b);
        }
        let resp =
            req.send().map_err(|e| anyhow::anyhow!("{method} {rest}: {}", self.redactor.redact(&e.to_string())))?;
        let status = resp.status();
        let text = self.redactor.redact(&resp.text().unwrap_or_default());
        if !status.is_success() {
            bail!("{method} {rest}: HTTP {}: {}", status.as_u16(), text.chars().take(500).collect::<String>());
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).with_context(|| format!("{method} {rest}: response is not JSON"))
    }

    /// Run SQL as the `postgres` role and return the rows of the last statement.
    pub fn query(&self, sql: &str) -> Result<Vec<Value>> {
        let rows = self.send(Method::POST, "/database/query", Some(&json!({ "query": sql })))?;
        match rows {
            Value::Array(rows) => Ok(rows),
            Value::Null => Ok(Vec::new()),
            other => bail!("unexpected SQL result shape: {}", other.to_string().chars().take(200).collect::<String>()),
        }
    }

    /// The project's advisor findings (`security` or `performance`).
    pub fn advisors(&self, kind: &str) -> Result<Vec<Value>> {
        let v = self.send(Method::GET, &format!("/advisors/{kind}"), None)?;
        Ok(v.get("lints").and_then(Value::as_array).cloned().unwrap_or_default())
    }

    pub fn auth_config(&self) -> Result<Value> {
        self.send(Method::GET, "/config/auth", None)
    }

    pub fn patch_auth_config(&self, patch: &Value) -> Result<Value> {
        self.send(Method::PATCH, "/config/auth", Some(patch))
    }

    /// The publishable key and the legacy service-role JWT (accepted by every Auth Admin endpoint).
    pub fn keys(&mut self) -> Result<Keys> {
        let list: Vec<ApiKey> = serde_json::from_value(self.send(Method::GET, "/api-keys?reveal=true", None)?)
            .context("unexpected api-keys shape")?;
        let find = |pred: &dyn Fn(&ApiKey) -> bool, what: &str| -> Result<Secret> {
            list.iter()
                .find(|k| pred(k))
                .and_then(|k| k.api_key.clone())
                .filter(|k| !k.is_empty() && !k.contains('·'))
                .map(Secret::new)
                .with_context(|| format!("project has no revealed {what} key"))
        };
        let publishable = find(&|k| k.kind.as_deref() == Some("publishable"), "publishable")?;
        let service_role = find(&|k| k.name == "service_role" && k.kind.as_deref() == Some("legacy"), "service_role")?;
        self.redactor.add(&service_role);
        Ok(Keys { publishable, service_role })
    }
}

/// The project's own HTTP APIs (Auth, PostgREST) for one caller identity.
pub struct Project {
    http: Client,
    pub url: String,
    pub publishable: Secret,
    redactor: Redactor,
}

/// A signed-in session of a test user.
pub struct Session {
    pub user_id: String,
    pub access_token: Secret,
}

impl Project {
    pub fn new(url: &str, publishable: Secret) -> Result<Self> {
        Ok(Self {
            http: http_client(Duration::from_secs(60))?,
            url: url.to_owned(),
            publishable,
            redactor: Redactor::new(),
        })
    }

    pub fn redact(&mut self, secret: &Secret) {
        self.redactor.add(secret);
    }

    fn request(&self, method: Method, path: &str, bearer: &Secret, apikey: &Secret) -> RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.url))
            .header("apikey", apikey.expose())
            .bearer_auth(bearer.expose())
    }

    fn finish(&self, what: &str, req: RequestBuilder) -> Result<Value> {
        let resp = req.send().map_err(|e| anyhow::anyhow!("{what}: {}", self.redactor.redact(&e.to_string())))?;
        let status = resp.status();
        let text = self.redactor.redact(&resp.text().unwrap_or_default());
        if !status.is_success() {
            bail!("{what}: HTTP {}: {}", status.as_u16(), text.chars().take(400).collect::<String>());
        }
        if text.trim().is_empty() { Ok(Value::Null) } else { serde_json::from_str(&text).context(what.to_owned()) }
    }

    /// Create a confirmed user through the Admin API. The sign-up hook still runs, so `metadata` needs an invite.
    pub fn admin_create_user(
        &self,
        service: &Secret,
        email: &str,
        password: Option<&Secret>,
        metadata: &Value,
    ) -> Result<String> {
        let mut body = json!({ "email": email, "email_confirm": true, "user_metadata": metadata });
        if let Some(p) = password {
            body["password"] = Value::String(p.expose().to_owned());
        }
        let user = self
            .finish("create user", self.request(Method::POST, "/auth/v1/admin/users", service, service).json(&body))?;
        user.get("id").and_then(Value::as_str).map(str::to_owned).context("created user has no id")
    }

    pub fn admin_delete_user(&self, service: &Secret, user_id: &str) -> Result<()> {
        self.finish(
            "delete user",
            self.request(Method::DELETE, &format!("/auth/v1/admin/users/{user_id}"), service, service),
        )
        .map(drop)
    }

    /// The id of the user with this email, if any (first 1000 users; this project is small by design).
    pub fn admin_find_user(&self, service: &Secret, email: &str) -> Result<Option<String>> {
        let list = self
            .finish("list users", self.request(Method::GET, "/auth/v1/admin/users?per_page=1000", service, service))?;
        let users = list.get("users").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(users
            .iter()
            .find(|u| u.get("email").and_then(Value::as_str).is_some_and(|e| e.eq_ignore_ascii_case(email)))
            .and_then(|u| u.get("id").and_then(Value::as_str))
            .map(str::to_owned))
    }

    /// A session without any email or password (accounts cannot hold passwords): the Admin API makes a magic-link
    /// token, which is verified at once like the link in an email would be. No email is sent.
    pub fn magic_link_session(&mut self, service: &Secret, email: &str) -> Result<Session> {
        let link = self.finish(
            "generate link",
            self.request(Method::POST, "/auth/v1/admin/generate_link", service, service)
                .json(&json!({ "type": "magiclink", "email": email })),
        )?;
        let hash = link
            .get("hashed_token")
            .or_else(|| link.pointer("/properties/hashed_token"))
            .and_then(Value::as_str)
            .context("generate_link returned no hashed_token")?;
        let hash = Secret::new(hash);
        self.redactor.add(&hash);
        let anon = self.publishable.clone();
        let req = self
            .http
            .post(format!("{}/auth/v1/verify", self.url))
            .header("apikey", anon.expose())
            .json(&json!({ "type": "magiclink", "token_hash": hash.expose() }));
        let session = self.finish("verify magic link", req)?;
        let access_token = session.get("access_token").and_then(Value::as_str).context("no access_token")?;
        let user_id = session.pointer("/user/id").and_then(Value::as_str).context("no user id")?.to_owned();
        let access_token = Secret::new(access_token);
        self.redactor.add(&access_token);
        Ok(Session { user_id, access_token })
    }

    /// A PostgREST call as the signed-in user.
    pub fn rest(&self, session: &Session, method: Method, path: &str, body: Option<&Value>) -> Result<Value> {
        let mut req = self
            .request(method.clone(), &format!("/rest/v1/{path}"), &session.access_token, &self.publishable)
            .header("prefer", "return=representation");
        if let Some(b) = body {
            req = req.json(b);
        }
        self.finish(&format!("{method} /rest/v1/{path}"), req)
    }
}

/// `'text'` as a SQL literal.
pub fn sql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn management_urls_always_name_our_project() {
        assert_eq!(
            Management::url("/database/query"),
            format!("{MANAGEMENT_API}/projects/{SUPABASE_REF}/database/query")
        );
    }

    #[test]
    fn sql_strings_are_escaped() {
        assert_eq!(sql_string("o'neil"), "'o''neil'");
    }
}
