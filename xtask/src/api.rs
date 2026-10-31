//! Client for the deployed nolabs HTTP API (`crates/core/src/proto/api.rs`), used by `smoke` and `status`. It
//! authenticates like the apps: `Authorization: Bearer <Supabase access token>`.

use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::{API_VERSION, API_VERSION_HEADER};
use reqwest::Method;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::cf::http_client;
use crate::secret::{Redactor, Secret};

pub struct Api {
    http: Client,
    base: String,
    token: Secret,
    redactor: Redactor,
}

/// Bytes of a GET with the response's content type.
pub struct Fetched {
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl Api {
    pub fn new(base: &str, access_token: &Secret) -> Result<Self> {
        let base = normalise_base(base)?;
        let redactor = Redactor::new().with(access_token);
        Ok(Self { http: http_client(Duration::from_secs(60))?, base, token: access_token.clone(), redactor })
    }

    pub fn base(&self) -> &str {
        &self.base
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
        self.fetch_bytes(path, true)
    }

    /// GET without the `Authorization` header, as a media element fetches (the path carries `?t=`).
    pub fn get_bytes_unauthenticated(&self, path: &str) -> Result<Fetched> {
        self.fetch_bytes(path, false)
    }

    fn fetch_bytes(&self, path: &str, with_token: bool) -> Result<Fetched> {
        let resp = self.send_as(Method::GET, path, with_token, |r| r)?;
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
        self.send_as(method, path, true, build)
    }

    fn send_as(
        &self,
        method: Method,
        path: &str,
        with_token: bool,
        build: impl FnOnce(RequestBuilder) -> RequestBuilder,
    ) -> Result<Response> {
        let mut req = self.http.request(method.clone(), format!("{}{path}", self.base));
        req = req.header(API_VERSION_HEADER, API_VERSION.to_string());
        if with_token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", self.token.expose()));
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
    fn rejects_bad_base_before_any_request() {
        let t = Secret::new("token");
        assert!(Api::new("http://evil.example", &t).is_err());
        let api = Api::new("https://a.b/", &t).unwrap();
        assert_eq!(api.base(), "https://a.b");
    }
}
