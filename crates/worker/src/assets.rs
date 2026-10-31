//! The landing site and the web app, embedded at build time (see `build.rs`) and served by the routing table of
//! `nolabs_server::static_routes`: ETags with 304s, HEAD, single byte ranges (Safari plays video only from servers
//! that answer them), redirects that keep the query, and the localised 404 page.

use nolabs_server::services::range::{ByteRange, parse_range};
use nolabs_server::static_assets::NO_STORE;
use nolabs_server::static_routes::{Resolved, etag_matches, resolve};
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
    ASSETS.binary_search_by(|a| a.path.cmp(path)).ok().map(|i| &ASSETS[i])
}

/// The parts of a GET or HEAD that static files depend on.
pub struct StaticRequest<'a> {
    pub path: &'a str,
    /// The query string without `?`, kept on redirects.
    pub query: Option<&'a str>,
    pub head: bool,
    pub if_none_match: Option<&'a str>,
    pub range: Option<&'a str>,
}

pub fn serve(req: &StaticRequest<'_>) -> Response {
    match resolve(req.path, |p| find(p).is_some()) {
        Resolved::File { path, status } => match find(&path) {
            Some(asset) if status == 200 => file(asset, req),
            Some(asset) => page(asset, status, req.head),
            None => not_found(None, req.head),
        },
        Resolved::Redirect { location, permanent } => {
            let location = match req.query.filter(|q| !q.is_empty()) {
                Some(q) => format!("{location}?{q}"),
                None => location,
            };
            let (status, cache) = if permanent { (301, "public, max-age=3600") } else { (302, NO_STORE) };
            build(status, &[("location", &location), ("cache-control", cache)], None)
        }
        Resolved::NotFound { page } => not_found(page.as_deref().and_then(find), req.head),
    }
}

/// A file with 200, 206, 304 or 416.
fn file(asset: &'static Asset, req: &StaticRequest<'_>) -> Response {
    let base = [
        ("content-type", asset.content_type),
        ("etag", asset.etag),
        ("cache-control", asset.cache_control),
        ("accept-ranges", "bytes"),
    ];
    if etag_matches(req.if_none_match, asset.etag) {
        return build(304, &base[1..3], None);
    }
    let total = asset.body.len() as u64;
    let range = if req.head { ByteRange::Full } else { parse_range(req.range, total) };
    let content_range = range.content_range(total).unwrap_or_default();
    let (status, body) = match range {
        ByteRange::Full => (200, asset.body),
        ByteRange::Partial { start, end } => (206, slice(asset.body, start, end)),
        ByteRange::Unsatisfiable => (416, &[][..]),
    };
    let len = body.len().to_string();
    let mut headers = base.to_vec();
    headers.push(("content-length", &len));
    if !content_range.is_empty() {
        headers.push(("content-range", &content_range));
    }
    build(status, &headers, (!req.head).then(|| body.to_vec()))
}

fn slice(body: &'static [u8], start: u64, end: u64) -> &'static [u8] {
    let (start, end) = (usize::try_from(start).unwrap_or(usize::MAX), usize::try_from(end).unwrap_or(usize::MAX));
    body.get(start..=end).unwrap_or_default()
}

/// An HTML page sent with an error status (the 404 page); never stored.
fn page(asset: &'static Asset, status: u16, head: bool) -> Response {
    let len = asset.body.len().to_string();
    let headers = [("content-type", asset.content_type), ("cache-control", NO_STORE), ("content-length", len.as_str())];
    build(status, &headers, (!head).then(|| asset.body.to_vec()))
}

fn not_found(page_asset: Option<&'static Asset>, head: bool) -> Response {
    match page_asset {
        Some(asset) => page(asset, 404, head),
        None => build(
            404,
            &[("content-type", "text/plain; charset=utf-8"), ("cache-control", NO_STORE)],
            (!head).then(|| b"Not found".to_vec()),
        ),
    }
}
