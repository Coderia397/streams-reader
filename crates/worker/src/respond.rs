//! Response helpers: JSON bodies, the API error body, and the security headers every response carries.

use nolabs_core::proto::ApiError;
use serde::Serialize;
use worker::{Headers, Response};

/// Only our own origin for scripts (plus wasm compilation), styles, media and fetches, and the Supabase project
/// for the app's direct PostgREST, Auth and Realtime (WebSocket) calls (decision D10).
pub fn csp(supabase_url: Option<&str>) -> String {
    let supabase = supabase_url
        .filter(|u| u.starts_with("https://"))
        .map(|u| format!(" {u} {}", u.replacen("https://", "wss://", 1)))
        .unwrap_or_default();
    format!(
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; \
         img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self'{supabase}; worker-src 'self'; \
         manifest-src 'self'; font-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"
    )
}

/// Convert a binding error into a 500.
pub fn internal(e: worker::Error) -> ApiError {
    ApiError::internal(e.to_string())
}

/// A response with these headers and an empty or fixed body; header errors cannot occur for our static names.
pub fn build(status: u16, headers: &[(&str, &str)], body: Option<Vec<u8>>) -> Response {
    let h = Headers::new();
    for (k, v) in headers {
        let _ = h.set(k, v);
    }
    let builder = Response::builder().with_status(status).with_headers(h);
    match body {
        Some(bytes) => builder.fixed(bytes),
        None => builder.empty(),
    }
}

pub fn json<T: Serialize>(status: u16, value: &T) -> Response {
    match serde_json::to_vec(value) {
        Ok(body) => build(status, &[("content-type", "application/json"), ("cache-control", "no-store")], Some(body)),
        Err(e) => error(&ApiError::internal(format!("serialize response: {e}"))),
    }
}

pub fn no_content() -> Response {
    build(204, &[("cache-control", "no-store")], None)
}

pub fn error(e: &ApiError) -> Response {
    let mut resp = json(e.status(), e);
    if let Some(s) = e.retry_after_s {
        let _ = resp.headers_mut().set("retry-after", &s.to_string());
    }
    resp
}

/// Add the security headers to any response (API, assets and errors alike).
pub fn secure(mut resp: Response, csp: &str) -> Response {
    let h = resp.headers_mut();
    for (k, v) in [
        ("content-security-policy", csp),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("permissions-policy", "microphone=(self)"),
    ] {
        let _ = h.set(k, v);
    }
    resp
}
