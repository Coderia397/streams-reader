//! The routing table of the embedded static files (everything except `/api`, `/hooks` and `/updates`): the landing
//! site at `/` per `crates/site/HEADERS.md`, the web app under `/app/` with its single-page fallback. Pure over a
//! `has(path)` lookup so the Worker resolves against its embedded table and the tests against a list.

use crate::static_assets::APP_PREFIX;

/// What a GET or HEAD for a static path gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// Serve this file with this status (200, or 404 for a 404 page requested by name).
    File { path: String, status: u16 },
    /// Redirect to this path; the caller appends the query string. Permanent ones (canonical slashes) are 301 and
    /// cacheable, the rest (auth links, which carry one-time tokens) 302 and never stored.
    Redirect { location: String, permanent: bool },
    /// 404, with the localised 404 page when one exists.
    NotFound { page: Option<String> },
}

/// Resolve a request path against the files `has` reports.
pub fn resolve(path: &str, has: impl Fn(&str) -> bool) -> Resolved {
    if path == "/app" {
        return moved(APP_PREFIX);
    }
    // Sign-in emails link to `<site>/auth/confirm` (crates/mail); the app's routes are under /app.
    if path.starts_with("/auth/") {
        return Resolved::Redirect {
            location: format!("{}{}", APP_PREFIX.trim_end_matches('/'), path),
            permanent: false,
        };
    }
    if path.starts_with(APP_PREFIX) { resolve_app(path, &has) } else { resolve_site(path, &has) }
}

fn moved(location: impl Into<String>) -> Resolved {
    Resolved::Redirect { location: location.into(), permanent: true }
}

fn file(path: impl Into<String>) -> Resolved {
    let path = path.into();
    let status = if path.ends_with("/404.html") { 404 } else { 200 };
    Resolved::File { path, status }
}

fn has_extension(path: &str) -> bool {
    path.rsplit('/').next().is_some_and(|last| last.contains('.'))
}

/// The app: exact files, then `index.html` for every path without an extension (client-side routes such as
/// `/app/read/<id>`); a missing file with an extension (a stale hashed asset) is a 404.
fn resolve_app(path: &str, has: &impl Fn(&str) -> bool) -> Resolved {
    let index = format!("{APP_PREFIX}index.html");
    if path.ends_with('/') && path.len() == APP_PREFIX.len() {
        return file(index);
    }
    if has(path) {
        return file(path);
    }
    if has_extension(path) { not_found("/", has) } else { file(index) }
}

/// A language root (`/ar/`) is the folder holding a localised 404 page; its canonical form keeps the slash. Every
/// other page (`/download`, `/ar/download`) is canonical without one.
fn is_language_root(dir: &str, has: &impl Fn(&str) -> bool) -> bool {
    dir.len() > 1 && !dir[1..].contains('/') && has(&format!("{dir}/404.html"))
}

fn resolve_site(path: &str, has: &impl Fn(&str) -> bool) -> Resolved {
    if path == "/" {
        return if has("/index.html") { file("/index.html") } else { not_found(path, has) };
    }
    if !path.starts_with('/') || path.contains("//") {
        return not_found("/", has);
    }
    if !path.ends_with('/') && has(path) {
        return file(path);
    }
    if let Some(dir) = path.strip_suffix('/') {
        let index = format!("{path}index.html");
        return match (has(&index), is_language_root(dir, has)) {
            (true, true) => file(index),
            (true, false) => moved(dir),
            (false, _) => not_found(path, has),
        };
    }
    if !has_extension(path) && has(&format!("{path}/index.html")) {
        return if is_language_root(path, has) {
            moved(format!("{path}/"))
        } else {
            file(format!("{path}/index.html"))
        };
    }
    not_found(path, has)
}

/// Whether an `If-None-Match` header names this ETag (weak or strong; `*` matches any file).
pub fn etag_matches(if_none_match: Option<&str>, etag: &str) -> bool {
    if_none_match
        .is_some_and(|v| v.split(',').map(|t| t.trim()).any(|t| t == "*" || t.strip_prefix("W/").unwrap_or(t) == etag))
}

/// The 404 page of the path's language when it starts with a language prefix, else the default one.
fn not_found(path: &str, has: &impl Fn(&str) -> bool) -> Resolved {
    let first = path.trim_start_matches('/').split('/').next().unwrap_or_default();
    let localised = format!("/{first}/404.html");
    let page = if !first.is_empty() && is_language_root(&format!("/{first}"), has) {
        Some(localised)
    } else {
        has("/404.html").then(|| "/404.html".to_owned())
    };
    Resolved::NotFound { page }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: &[&str] = &[
        "/index.html",
        "/404.html",
        "/robots.txt",
        "/.well-known/security.txt",
        "/assets/site.b6aceb238d.js",
        "/download/index.html",
        "/privacy/index.html",
        "/ar/index.html",
        "/ar/404.html",
        "/ar/download/index.html",
        "/zh/index.html",
        "/zh/404.html",
        "/app/index.html",
        "/app/sw.js",
        "/app/nolabs-web-3f2a9c1d0b8e7f6a_bg.wasm",
        "/app/import/loader-0a1b2c3d4e5f6a7b.js",
    ];

    fn route(path: &str) -> Resolved {
        resolve(path, |p| FILES.contains(&p))
    }

    fn ok(path: &str) -> Resolved {
        Resolved::File { path: path.into(), status: 200 }
    }

    fn redirect(location: &str) -> Resolved {
        Resolved::Redirect { location: location.into(), permanent: true }
    }

    fn missing(page: &str) -> Resolved {
        Resolved::NotFound { page: Some(page.into()) }
    }

    #[test]
    fn site_routes_follow_headers_md() {
        let table = [
            ("/", ok("/index.html")),
            ("/index.html", ok("/index.html")),
            ("/robots.txt", ok("/robots.txt")),
            ("/.well-known/security.txt", ok("/.well-known/security.txt")),
            ("/assets/site.b6aceb238d.js", ok("/assets/site.b6aceb238d.js")),
            ("/download", ok("/download/index.html")),
            ("/download/", redirect("/download")),
            ("/ar/", ok("/ar/index.html")),
            ("/ar", redirect("/ar/")),
            ("/ar/download", ok("/ar/download/index.html")),
            ("/ar/download/", redirect("/ar/download")),
            ("/zh/", ok("/zh/index.html")),
            ("/404.html", Resolved::File { path: "/404.html".into(), status: 404 }),
        ];
        for (path, want) in table {
            assert_eq!(route(path), want, "{path}");
        }
    }

    #[test]
    fn unknown_site_paths_get_the_right_404_page() {
        let table = [
            ("/nope", missing("/404.html")),
            ("/nope/", missing("/404.html")),
            ("/assets/missing.css", missing("/404.html")),
            ("/ar/nope", missing("/ar/404.html")),
            ("/ar/download/extra", missing("/ar/404.html")),
            ("/zh/missing.png", missing("/zh/404.html")),
            ("/de/", missing("/404.html")),
            ("//evil.example", missing("/404.html")),
            ("/download/index", missing("/404.html")),
        ];
        for (path, want) in table {
            assert_eq!(route(path), want, "{path}");
        }
    }

    #[test]
    fn app_routes_fall_back_to_the_shell() {
        let table = [
            ("/app", redirect("/app/")),
            ("/app/", ok("/app/index.html")),
            ("/app/sw.js", ok("/app/sw.js")),
            ("/app/import/loader-0a1b2c3d4e5f6a7b.js", ok("/app/import/loader-0a1b2c3d4e5f6a7b.js")),
            ("/app/read/0f3a", ok("/app/index.html")),
            ("/app/voices", ok("/app/index.html")),
            ("/app/settings/", ok("/app/index.html")),
            ("/app/nolabs-web-0000000000000000_bg.wasm", missing("/404.html")),
        ];
        for (path, want) in table {
            assert_eq!(route(path), want, "{path}");
        }
    }

    #[test]
    fn email_auth_links_reach_the_app_without_caching() {
        let table = [
            ("/auth/confirm", Resolved::Redirect { location: "/app/auth/confirm".into(), permanent: false }),
            ("/auth/callback", Resolved::Redirect { location: "/app/auth/callback".into(), permanent: false }),
            ("/auth", missing("/404.html")),
        ];
        for (path, want) in table {
            assert_eq!(route(path), want, "{path}");
        }
    }

    #[test]
    fn etags_match_weak_strong_lists_and_star() {
        let etag = "\"abc\"";
        for header in ["\"abc\"", "W/\"abc\"", "\"x\", \"abc\"", "*"] {
            assert!(etag_matches(Some(header), etag), "{header}");
        }
        for header in ["\"abcd\"", "abc", ""] {
            assert!(!etag_matches(Some(header), etag), "{header}");
        }
        assert!(!etag_matches(None, etag));
    }

    #[test]
    fn empty_build_has_no_404_page() {
        assert_eq!(resolve("/x", |_| false), Resolved::NotFound { page: None });
        assert_eq!(resolve("/", |_| false), Resolved::NotFound { page: None });
    }
}
