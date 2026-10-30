//! The web app, embedded at build time (see `build.rs`). Paths without a file extension fall back to `index.html` so
//! client-side routes (`/read/<id>`, `/voices`) load the app.

use worker::Response;

use crate::respond::build;

pub struct Asset {
    pub path: &'static str,
    pub content_type: &'static str,
    pub etag: &'static str,
    pub cache_control: &'static str,
    pub body: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

fn find(path: &str) -> Option<&'static Asset> {
    ASSETS.iter().find(|a| a.path == path)
}

/// The asset for a request path, applying the index and SPA fallbacks.
fn lookup(path: &str) -> Option<&'static Asset> {
    let path = if path == "/" { "/index.html" } else { path };
    if let Some(asset) = find(path) {
        return Some(asset);
    }
    let last = path.rsplit('/').next().unwrap_or_default();
    if last.contains('.') { None } else { find("/index.html") }
}

/// Serve a GET or HEAD for a non-API path.
pub fn serve(path: &str, head: bool, if_none_match: Option<&str>) -> Response {
    let Some(asset) = lookup(path) else {
        return build(
            404,
            &[("content-type", "text/plain; charset=utf-8"), ("cache-control", "no-cache")],
            Some(b"Not found".to_vec()),
        );
    };
    let headers = [("content-type", asset.content_type), ("etag", asset.etag), ("cache-control", asset.cache_control)];
    if if_none_match.is_some_and(|v| v.split(',').any(|t| t.trim().trim_start_matches("W/") == asset.etag)) {
        return build(304, &headers[1..], None);
    }
    let len = asset.body.len().to_string();
    let mut all = headers.to_vec();
    all.push(("content-length", &len));
    build(200, &all, if head { None } else { Some(asset.body.to_vec()) })
}
