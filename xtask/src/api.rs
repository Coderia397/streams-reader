//! Client for the deployed nolabs HTTP API (`crates/core/src/proto/api.rs`), used by `smoke` and `status`.
//!
//! The session cookie is handled by hand instead of a cookie store: `__Host-` cookies are `Secure`-only, which a
//! cookie store would refuse to send to an `http://localhost` base during local testing. Writes carry an `Origin`
//! header equal to the base URL because the Worker rejects cross-origin writes.

use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::{API_VERSION, API_VERSION_HEADER, LoginRequest, SESSION_COOKIE};
use reqwest::Method;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{CONTENT_TYPE, COOKIE, ORIGIN, SET_COOKIE};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::cf::http_client;
use crate::secret::{Redactor, Secret};

pub struct Api {
    http: Client,
    base: String,
    cookie: Option<Secret>,
    redactor: Redactor,
}

/// Bytes of a GET with the response's content type.
pub struct Fetched {
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl Api {
    pub fn new(base: &str) -> Result<Self> {
        let base = normalise_base(base)?;
        Ok(Self { http: http_client(Duration::from_secs(60))?, base, cookie: None, redactor: Redactor::new() })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// `POST /api/session`; keeps the session cookie for later calls.
    pub fn login(&mut self, passphrase: &Secret) -> Result<()> {
        self.redactor.add(passphrase);
        let body = LoginRequest { passphrase: passphrase.expose().to_owned() };
        let resp = self.send(Method::POST, "/api/session", |r| r.json(&body))?;
        let value = resp
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(|v| cookie_value(v, SESSION_COOKIE))
            .with_context(|| format!("login succeeded but no {SESSION_COOKIE} cookie was set"))?;
        let cookie = Secret::new(value);
        self.redactor.add(&cookie);
        self.cookie = Some(cookie);
        Ok(())
    }

    pub fn logout(&mut self) -> Result<()> {
        self.send(Method::DELETE, "/api/session", |r| r)?;
        self.cookie = None;
        Ok(())
    }

    pub fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        json_body(path, self.send(Method::GET, path, |r| r)?)
    }

    pub fn post_json<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        json_body(path, self.send(Method::POST, path, |r| r.json(body))?)
    }

    pub fn put_bytes(&self, path: &str, content_type: &str, bytes: Vec<u8>) -> Result<()> {
        self.send(Method::PUT, path, |r| r.header(CONTENT_TYPE, content_type).body(bytes)).map(drop)
    }

    pub fn delete(&self, path: &str) -> Result<()> {
        self.send(Method::DELETE, path, |r| r).map(drop)
    }

    pub fn get_bytes(&self, path: &str) -> Result<Fetched> {
        let resp = self.send(Method::GET, path, |r| r)?;
        let content_type =
            resp.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or_default().to_owned();
        let bytes = resp.bytes().with_context(|| format!("GET {path}: reading body"))?.to_vec();
        Ok(Fetched { content_type, bytes })
    }

    fn send(
        &self,
        method: Method,
        path: &str,
        build: impl FnOnce(RequestBuilder) -> RequestBuilder,
    ) -> Result<Response> {
        let mut req = self.http.request(method.clone(), format!("{}{path}", self.base));
        req = req.header(API_VERSION_HEADER, API_VERSION.to_string());
        if method != Method::GET && method != Method::HEAD {
            req = req.header(ORIGIN, &self.base);
        }
        if let Some(c) = &self.cookie {
            req = req.header(COOKIE, format!("{SESSION_COOKIE}={}", c.expose()));
        }
        let resp = build(req)
            .send()
            .map_err(|e| anyhow::anyhow!("{method} {path}: {}", self.redactor.redact(&e.to_string())))?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let text = self.redactor.redact(&resp.text().unwrap_or_default());
        match serde_json::from_str::<ApiError>(&text) {
            Ok(e) => bail!("{method} {path}: HTTP {}: {e}", status.as_u16()),
            Err(_) => {
                bail!("{method} {path}: HTTP {}: {}", status.as_u16(), text.chars().take(300).collect::<String>())
            }
        }
    }
}

fn json_body<T: DeserializeOwned>(path: &str, resp: Response) -> Result<T> {
    let bytes = resp.bytes().with_context(|| format!("{path}: reading body"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("{path}: unexpected response shape"))
}

/// `https://host[:port]` without a trailing slash or path. Plain `http` is only accepted for localhost.
pub fn normalise_base(base: &str) -> Result<String> {
    let trimmed = base.trim().trim_end_matches('/');
    let local = ["http://localhost", "http://127.0.0.1"]
        .iter()
        .any(|p| trimmed == *p || trimmed.strip_prefix(p).is_some_and(|rest| rest.starts_with(':')));
    if !(trimmed.starts_with("https://") || local) {
        bail!("base URL must be https:// (or http://localhost): {trimmed}");
    }
    let after_scheme = trimmed.split_once("://").map_or("", |(_, rest)| rest);
    if after_scheme.is_empty() || after_scheme.contains('/') {
        bail!("base URL must be an origin without a path: {trimmed}");
    }
    Ok(trimmed.to_owned())
}

/// The value of cookie `name` in one `Set-Cookie` header, ignoring empty values (a cleared cookie).
pub fn cookie_value(set_cookie: &str, name: &str) -> Option<String> {
    let first = set_cookie.split(';').next()?.trim();
    let (k, v) = first.split_once('=')?;
    (k.trim() == name && !v.trim().is_empty()).then(|| v.trim().to_owned())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn base_urls() {
        assert_eq!(normalise_base("https://nolabs-api.x.workers.dev/").unwrap(), "https://nolabs-api.x.workers.dev");
        assert_eq!(normalise_base("http://localhost:8787").unwrap(), "http://localhost:8787");
        assert_eq!(normalise_base("http://127.0.0.1").unwrap(), "http://127.0.0.1");
        for bad in ["http://example.com", "ftp://x", "https://", "https://x.dev/api", "http://localhost.evil.com", "x"]
        {
            assert!(normalise_base(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn session_cookie_extraction() {
        let h = "__Host-nolabs_s=abc.def; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=15552000";
        assert_eq!(cookie_value(h, SESSION_COOKIE).as_deref(), Some("abc.def"));
        assert_eq!(cookie_value("other=1; Path=/", SESSION_COOKIE), None);
        assert_eq!(cookie_value("__Host-nolabs_s=; Max-Age=0", SESSION_COOKIE), None);
        assert_eq!(cookie_value("garbage", SESSION_COOKIE), None);
    }

    #[test]
    fn rejects_bad_base_before_any_request() {
        assert!(Api::new("http://evil.example").is_err());
        let api = Api::new("https://a.b/").unwrap();
        assert_eq!(api.base(), "https://a.b");
    }
}
