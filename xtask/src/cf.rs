//! Cloudflare API v4 client. Two ways to authenticate: the scoped deploy token (every command but one) and the Global
//! API Key, which only `bootstrap-token` constructs. Reads go through [`Cf::get`] and friends; every write goes
//! through [`Cf::mutate`], which runs the ownership guard on the named target before anything is sent. All text that
//! comes back from Cloudflare passes through the [`Redactor`] before it can appear in an error.

use std::time::Duration;

use anyhow::{Context as _, Result};
use reqwest::Method;
use reqwest::blocking::{Client, RequestBuilder, multipart::Form};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::envfile::EnvFile;
use crate::names::guard;
use crate::paths::{self, tilde};
use crate::secret::{Redactor, Secret};

const API: &str = "https://api.cloudflare.com/client/v4";
const MAX_PAGES: u32 = 50;

pub enum Auth {
    /// `Authorization: Bearer` with the scoped `nolabs-deploy` token.
    Token(Secret),
    /// `X-Auth-Email` + `X-Auth-Key`. Full account access: only `bootstrap-token` builds this.
    GlobalKey { email: Secret, key: Secret },
}

pub enum Body {
    Empty,
    Json(Value),
    Multipart(Form),
}

/// A non-success Cloudflare response, already redacted.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{method} {path}: HTTP {status}: {detail}")]
pub struct CfError {
    pub method: Method,
    pub path: String,
    pub status: u16,
    pub codes: Vec<i64>,
    pub detail: String,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(default)]
    success: bool,
    /// Some APIs (Queues) send `"errors": null` on success.
    #[serde(default, deserialize_with = "null_as_empty")]
    errors: Vec<CfMessage>,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    result_info: Option<ResultInfo>,
}

fn null_as_empty<'de, D, T>(de: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(de)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct CfMessage {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct ResultInfo {
    #[serde(default)]
    total_pages: Option<u32>,
}

pub struct Cf {
    http: Client,
    auth: Auth,
    redactor: Redactor,
    account: String,
}

impl Cf {
    pub fn new(auth: Auth, account: String) -> Result<Self> {
        let mut redactor = Redactor::new();
        match &auth {
            Auth::Token(t) => redactor.add(t),
            Auth::GlobalKey { email, key } => {
                redactor.add(key);
                redactor.add(email);
            }
        }
        Ok(Self { http: http_client(Duration::from_secs(300))?, auth, redactor, account })
    }

    /// The deploy-token client used by every command except `bootstrap-token`. Refuses to run without the token file.
    pub fn from_deploy_env() -> Result<Self> {
        let path = paths::deploy_env()?;
        if !path.exists() {
            anyhow::bail!("{} not found: run `cargo xtask bootstrap-token` first", tilde(&path));
        }
        let env = EnvFile::load(&path)?;
        let token = env.secret("CF_API_TOKEN")?.clone();
        let account = env.plain("CF_ACCOUNT_ID")?;
        if let Some(expires) = env.get("CF_TOKEN_EXPIRES_ON") {
            eprintln!("using deploy token from {} (expires {})", tilde(&path), expires.expose());
        }
        Self::new(Auth::Token(token), account)
    }

    /// `/accounts/{id}` + `rest`.
    pub fn acct(&self, rest: &str) -> String {
        format!("/accounts/{}{rest}", self.account)
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let env = self.send(Method::GET, path, Body::Empty)?;
        decode(path, env.result)
    }

    /// Like [`Cf::get`], but a 404 is `Ok(None)`.
    pub fn get_opt<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>> {
        match self.send(Method::GET, path, Body::Empty) {
            Ok(env) => decode(path, env.result).map(Some),
            Err(e) if e.downcast_ref::<CfError>().is_some_and(|c| c.status == 404) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Collects every page of a page-numbered list endpoint. `path` must not carry `page`/`per_page` already.
    pub fn get_all<T: DeserializeOwned>(&self, path: &str) -> Result<Vec<T>> {
        let sep = if path.contains('?') { '&' } else { '?' };
        let mut out = Vec::new();
        for page in 1..=MAX_PAGES {
            let paged = format!("{path}{sep}page={page}&per_page=100");
            let env = self.send(Method::GET, &paged, Body::Empty)?;
            let total = env.result_info.as_ref().and_then(|i| i.total_pages).unwrap_or(1);
            let items: Vec<T> = decode(path, env.result)?;
            let empty = items.is_empty();
            out.extend(items);
            if empty || page >= total {
                return Ok(out);
            }
        }
        anyhow::bail!("GET {path}: more than {MAX_PAGES} pages")
    }

    /// A write. `target` is the name of the resource being changed; the guard panics unless this project owns it.
    pub fn mutate<T: DeserializeOwned>(&self, target: &str, method: Method, path: &str, body: Body) -> Result<T> {
        guard(target);
        let env = self.send(method, path, body)?;
        decode(path, env.result)
    }

    fn send(&self, method: Method, path: &str, body: Body) -> Result<Envelope> {
        let url = format!("{API}{path}");
        let mut req: RequestBuilder = self.http.request(method.clone(), &url);
        req = match &self.auth {
            Auth::Token(t) => req.bearer_auth(t.expose()),
            Auth::GlobalKey { email, key } => {
                req.header("X-Auth-Email", email.expose()).header("X-Auth-Key", key.expose())
            }
        };
        req = match body {
            Body::Empty => req,
            Body::Json(v) => req.json(&v),
            Body::Multipart(form) => req.multipart(form),
        };
        let resp =
            req.send().map_err(|e| anyhow::anyhow!("{method} {path}: {}", self.redactor.redact(&e.to_string())))?;
        let status = resp.status().as_u16();
        let text = resp.text().map_err(|e| anyhow::anyhow!("{method} {path}: reading body: {e}"))?;
        let text = self.redactor.redact(&text);
        let env: Envelope = match serde_json::from_str(&text) {
            Ok(env) => env,
            Err(_) => {
                return Err(
                    CfError { method, path: path.to_owned(), status, codes: vec![], detail: snippet(&text) }.into()
                );
            }
        };
        if !(200..300).contains(&status) || !env.success {
            let codes = env.errors.iter().map(|e| e.code).collect();
            let detail = if env.errors.is_empty() {
                snippet(&text)
            } else {
                env.errors.iter().map(|e| format!("[{}] {}", e.code, e.message)).collect::<Vec<_>>().join("; ")
            };
            return Err(CfError { method, path: path.to_owned(), status, codes, detail }.into());
        }
        Ok(env)
    }
}

fn decode<T: DeserializeOwned>(path: &str, result: Value) -> Result<T> {
    serde_json::from_value(result).with_context(|| format!("{path}: unexpected response shape"))
}

fn snippet(text: &str) -> String {
    let t: String = text.chars().take(300).collect();
    if t.is_empty() { "(empty body)".to_owned() } else { t }
}

pub fn http_client(timeout: Duration) -> Result<Client> {
    Client::builder()
        .user_agent(concat!("nolabs-xtask/", env!("CARGO_PKG_VERSION")))
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(20))
        .build()
        .context("cannot build HTTP client")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn envelope_parses_errors_and_paging() {
        let env: Envelope = serde_json::from_str(
            r#"{"success":false,"errors":[{"code":10006,"message":"bucket not found"}],"messages":[],"result":null,
                "result_info":{"page":1,"total_pages":3}}"#,
        )
        .unwrap();
        assert!(!env.success);
        assert_eq!(env.errors[0].code, 10006);
        assert_eq!(env.result_info.unwrap().total_pages, Some(3));
    }

    #[test]
    fn null_result_decodes_to_unit_and_value() {
        decode::<()>("/x", Value::Null).unwrap();
        assert_eq!(decode::<Value>("/x", Value::Null).unwrap(), Value::Null);
        assert!(decode::<Vec<String>>("/x", serde_json::json!({"a":1})).is_err());
    }

    #[test]
    fn acct_builds_account_paths() {
        let cf = Cf::new(Auth::Token(Secret::new("t")), "abc".into()).unwrap();
        assert_eq!(cf.acct("/d1/database"), "/accounts/abc/d1/database");
    }

    #[test]
    #[should_panic(expected = "refusing to modify \"streams-api\"")]
    fn mutate_guards_before_sending() {
        // The guard fires before any network I/O, so this test never touches the network.
        let cf = Cf::new(Auth::Token(Secret::new("t")), "abc".into()).unwrap();
        let _: Result<Value> =
            cf.mutate("streams-api", Method::PUT, "/accounts/abc/workers/scripts/streams-api", Body::Empty);
    }

    #[test]
    fn snippet_truncates() {
        assert_eq!(snippet(""), "(empty body)");
        assert_eq!(snippet(&"x".repeat(1000)).len(), 300);
    }
}
