//! The signed-in user's rows in Supabase, through PostgREST (`<SUPABASE_URL>/rest/v1`). Every call sends the
//! publishable key as `apikey` and the access token as `Authorization: Bearer`; row level security limits each user
//! to their own rows. A 401 refreshes the session once and retries, like the Worker client.

pub mod bookmarks;
pub mod library;
pub mod profile;
pub mod rpc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::api::ClientError;
use crate::api::client::{js_error_text, read_text, signal_auth_lost};
use crate::api::error::parse_error_body;
use crate::auth;
use crate::config::{SUPABASE_KEY, SUPABASE_URL};
use crate::util::dom;

/// `https://<project>.supabase.co/rest/v1/<path>`.
pub fn url(path: &str) -> String {
    format!("{SUPABASE_URL}/rest/v1/{path}")
}

fn browser(e: JsValue) -> ClientError {
    ClientError::Browser(js_error_text(e))
}

/// Request for PostgREST. `prefer` is the `Prefer` header (`return=representation`, `resolution=merge-duplicates`).
pub(crate) fn request_init(
    method: &str,
    body: Option<&str>,
    prefer: Option<&str>,
) -> Result<web_sys::RequestInit, ClientError> {
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    init.set_credentials(web_sys::RequestCredentials::Omit);
    let headers = web_sys::Headers::new().map_err(browser)?;
    headers.set("apikey", SUPABASE_KEY).map_err(browser)?;
    // The publishable key is not a JWT, so anonymous calls send no Authorization header at all.
    if let Some(token) = auth::access_token() {
        headers.set("authorization", &format!("Bearer {token}")).map_err(browser)?;
    }
    if let Some(p) = prefer {
        headers.set("prefer", p).map_err(browser)?;
    }
    if let Some(b) = body {
        headers.set("content-type", "application/json").map_err(browser)?;
        init.set_body(&JsValue::from_str(b));
    }
    init.set_headers(&headers);
    Ok(init)
}

async fn fetch_once(
    method: &str,
    path: &str,
    body: Option<&str>,
    prefer: Option<&str>,
) -> Result<web_sys::Response, ClientError> {
    let window = dom::window().ok_or_else(|| ClientError::Browser("no window".into()))?;
    let init = request_init(method, body, prefer)?;
    let resp = JsFuture::from(window.fetch_with_str_and_init(&url(path), &init))
        .await
        .map_err(|e| ClientError::Network(js_error_text(e)))?;
    resp.dyn_into().map_err(|_| ClientError::Decode("not a Response".into()))
}

/// Sends a request and returns the body text of a 2xx answer.
pub async fn send(method: &str, path: &str, body: Option<&str>, prefer: Option<&str>) -> Result<String, ClientError> {
    let mut resp = fetch_once(method, path, body, prefer).await?;
    if resp.status() == 401 && auth::access_token().is_some() && auth::refresh_now().await.is_ok() {
        resp = fetch_once(method, path, body, prefer).await?;
    }
    let text = read_text(&resp).await.unwrap_or_default();
    if resp.ok() {
        return Ok(text);
    }
    if resp.status() == 401 {
        signal_auth_lost();
    }
    Err(ClientError::Api(parse_error_body(resp.status(), &text)))
}

pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, ClientError> {
    let text = send("GET", path, None, None).await?;
    serde_json::from_str(&text).map_err(|e| ClientError::Decode(e.to_string()))
}

pub fn to_json<T: Serialize>(value: &T) -> Result<String, ClientError> {
    serde_json::to_string(value).map_err(|e| ClientError::Decode(e.to_string()))
}
