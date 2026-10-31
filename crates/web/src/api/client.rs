//! The one place that calls `fetch` for `/api`. Every request carries the API version header and, when signed in,
//! `Authorization: Bearer <access token>`; error bodies become [`ClientError::Api`]. A 401 refreshes the session
//! once and retries; when that fails the auth-lost hook runs (the app shows sign-in). 409 `upgrade_required` reloads
//! the page to pick up the new build.

use std::cell::RefCell;
use std::rc::Rc;

use nolabs_core::proto::ErrorCode;
use nolabs_core::proto::api::{API_VERSION, API_VERSION_HEADER};
use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use super::error::{ClientError, parse_error_body};
use crate::auth;
use crate::util::dom;

thread_local! {
    static AUTH_LOST: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Registers what to do when a call stays 401 after a refresh (set once by the app shell, inside the router).
pub fn on_auth_lost(hook: Rc<dyn Fn()>) {
    AUTH_LOST.with(|h| *h.borrow_mut() = Some(hook));
}

/// Runs the auth-lost hook (a call stayed 401 after a refresh).
pub(crate) fn signal_auth_lost() {
    if let Some(hook) = AUTH_LOST.with(|h| h.borrow().clone()) {
        hook();
    }
}

#[derive(Clone)]
pub enum Body {
    Empty,
    Json(String),
    Bytes { data: js_sys::Uint8Array, content_type: &'static str },
}

impl Body {
    pub fn json<T: Serialize>(value: &T) -> Result<Body, ClientError> {
        serde_json::to_string(value).map(Body::Json).map_err(|e| ClientError::Decode(e.to_string()))
    }
}

fn js_err(e: JsValue) -> String {
    e.as_string()
        .or_else(|| js_sys::Reflect::get(&e, &JsValue::from_str("message")).ok().and_then(|m| m.as_string()))
        .unwrap_or_else(|| "unknown error".to_owned())
}

fn browser(e: JsValue) -> ClientError {
    ClientError::Browser(js_err(e))
}

/// Builds the request: version header, bearer token, body.
pub(crate) fn request_init(method: &str, body: &Body) -> Result<web_sys::RequestInit, ClientError> {
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    // Bearer tokens only: no cookies go to the Worker (D24).
    init.set_credentials(web_sys::RequestCredentials::Omit);
    let headers = web_sys::Headers::new().map_err(browser)?;
    headers.set(API_VERSION_HEADER, &API_VERSION.to_string()).map_err(browser)?;
    if let Some(token) = auth::access_token() {
        headers.set("authorization", &format!("Bearer {token}")).map_err(browser)?;
    }
    match body {
        Body::Empty => {}
        Body::Json(text) => {
            headers.set("content-type", "application/json").map_err(browser)?;
            init.set_body(&JsValue::from_str(text));
        }
        Body::Bytes { data, content_type } => {
            headers.set("content-type", content_type).map_err(browser)?;
            init.set_body(data);
        }
    }
    init.set_headers(&headers);
    Ok(init)
}

async fn fetch_once(method: &str, path: &str, body: &Body) -> Result<web_sys::Response, ClientError> {
    let window = dom::window().ok_or_else(|| ClientError::Browser("no window".into()))?;
    let init = request_init(method, body)?;
    let resp = JsFuture::from(window.fetch_with_str_and_init(path, &init))
        .await
        .map_err(|e| ClientError::Network(js_err(e)))?;
    resp.dyn_into().map_err(|_| ClientError::Decode("not a Response".into()))
}

/// Sends a request and returns the response when it is 2xx.
pub async fn send(method: &str, path: &str, body: Body) -> Result<web_sys::Response, ClientError> {
    let mut resp = fetch_once(method, path, &body).await?;
    if resp.status() == 401 && auth::refresh_now().await.is_ok() {
        resp = fetch_once(method, path, &body).await?;
    }
    if resp.ok() {
        return Ok(resp);
    }
    let status = resp.status();
    let text = read_text(&resp).await.unwrap_or_default();
    let err = parse_error_body(status, &text);
    match err.code {
        ErrorCode::Unauthorized => signal_auth_lost(),
        ErrorCode::UpgradeRequired => dom::reload(),
        _ => {}
    }
    Err(ClientError::Api(err))
}

pub async fn read_text(resp: &web_sys::Response) -> Result<String, ClientError> {
    let promise = resp.text().map_err(|e| ClientError::Decode(js_err(e)))?;
    let value = JsFuture::from(promise).await.map_err(|e| ClientError::Network(js_err(e)))?;
    value.as_string().ok_or_else(|| ClientError::Decode("body is not text".into()))
}

pub async fn read_bytes(resp: &web_sys::Response) -> Result<js_sys::Uint8Array, ClientError> {
    let promise = resp.array_buffer().map_err(|e| ClientError::Decode(js_err(e)))?;
    let value = JsFuture::from(promise).await.map_err(|e| ClientError::Network(js_err(e)))?;
    Ok(js_sys::Uint8Array::new(&value))
}

pub async fn read_json<T: DeserializeOwned>(resp: &web_sys::Response) -> Result<T, ClientError> {
    let text = read_text(resp).await?;
    serde_json::from_str(&text).map_err(|e| ClientError::Decode(e.to_string()))
}

pub async fn get_json<T: DeserializeOwned>(path: &str) -> Result<T, ClientError> {
    let resp = send("GET", path, Body::Empty).await?;
    read_json(&resp).await
}

pub async fn send_json<B: Serialize, T: DeserializeOwned>(
    method: &str,
    path: &str,
    body: &B,
) -> Result<T, ClientError> {
    let resp = send(method, path, Body::json(body)?).await?;
    read_json(&resp).await
}

/// For routes answering 204.
pub async fn send_no_content(method: &str, path: &str, body: Body) -> Result<(), ClientError> {
    send(method, path, body).await.map(|_| ())
}

/// Percent-encodes one URL query or path component (RFC 3986 unreserved characters pass through).
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Percent-encodes one URL path segment; like [`encode_component`] but keeps `:` and `@` (RFC 3986 `pchar`), so voice
/// ids such as `user:0123456789abcdef` stay readable in `/api/voices/{id}`.
pub fn encode_segment(s: &str) -> String {
    encode_component(s).replace("%3A", ":").replace("%40", "@")
}

pub(crate) fn js_error_text(e: JsValue) -> String {
    js_err(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_components() {
        assert_eq!(encode_component("https://a.b/c?d=e f"), "https%3A%2F%2Fa.b%2Fc%3Fd%3De%20f");
        assert_eq!(encode_component("abc-_.~"), "abc-_.~");
        assert_eq!(encode_component("é"), "%C3%A9");
        assert_eq!(encode_segment("user:0123456789abcdef"), "user:0123456789abcdef");
        assert_eq!(encode_segment("a/b c"), "a%2Fb%20c");
    }
}
