//! Request guards that need no storage: the API version check and CORS.
//!
//! The passphrase cookie needed a same-origin check against cross-site request forgery. Bearer tokens are never sent
//! by the browser on its own, so a forged cross-site request carries no credentials; CORS can therefore open the API
//! to the native apps' origins without weakening it. The allowlist still keeps other websites from reading responses.

use nolabs_core::brand;
use nolabs_core::proto::api::{API_VERSION, API_VERSION_HEADER};
use nolabs_core::proto::{ApiError, ErrorCode};

/// The app sends `x-nolabs-api: <API_VERSION>`; anything else means the installed app is older or newer than the
/// Worker.
pub fn check_api_version(header: Option<&str>) -> Result<(), ApiError> {
    match header.map(str::trim) {
        Some(v) if v == API_VERSION.to_string() => Ok(()),
        _ => Err(ApiError::new(
            ErrorCode::UpgradeRequired,
            format!("this app version does not match the server (API {API_VERSION}); update the app"),
        )),
    }
}

/// Origins allowed to call the API from a browser engine: the site, the Tauri apps (macOS/Linux use
/// `tauri://localhost`, Windows and Android `http(s)://tauri.localhost`) and local development servers.
pub fn cors_allows(origin: &str, site_url: &str) -> bool {
    let origin = origin.trim();
    if origin.eq_ignore_ascii_case(site_url) || origin.eq_ignore_ascii_case(brand::SITE_URL) {
        return true;
    }
    if matches!(origin, "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost") {
        return true;
    }
    ["http://localhost:", "http://127.0.0.1:"].iter().any(|prefix| {
        origin.strip_prefix(prefix).is_some_and(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// Headers for an allowed origin, on preflights and on actual responses.
pub fn cors_headers(origin: &str) -> Vec<(&'static str, String)> {
    vec![
        ("access-control-allow-origin", origin.to_owned()),
        ("vary", "Origin".to_owned()),
        ("access-control-allow-methods", "GET, HEAD, POST, PUT, DELETE".to_owned()),
        ("access-control-allow-headers", format!("authorization, content-type, range, {API_VERSION_HEADER}")),
        (
            "access-control-expose-headers",
            "content-length, content-range, accept-ranges, retry-after, x-final-url, x-upstream-content-type"
                .to_owned(),
        ),
        ("access-control-max-age", "86400".to_owned()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITE: &str = "https://listen.streams.watch";

    #[test]
    fn version_header_must_match() {
        assert!(check_api_version(Some("2")).is_ok());
        assert!(check_api_version(Some(" 2 ")).is_ok());
        let e = check_api_version(Some("1")).unwrap_err();
        assert_eq!((e.code, e.status()), (ErrorCode::UpgradeRequired, 409));
        assert!(check_api_version(None).is_err());
    }

    #[test]
    fn cors_allowlist() {
        for ok in [
            SITE,
            "tauri://localhost",
            "http://tauri.localhost",
            "https://tauri.localhost",
            "http://localhost:8080",
            "http://127.0.0.1:1420",
        ] {
            assert!(cors_allows(ok, SITE), "{ok}");
        }
        assert!(cors_allows("https://nolabs-api.x.workers.dev", "https://nolabs-api.x.workers.dev"));
        for bad in [
            "https://evil.dev",
            "https://listen.streams.watch.evil.dev",
            "http://listen.streams.watch",
            "http://localhost",
            "http://localhost:80x",
            "http://localhost.evil.dev:80",
            "null",
            "",
        ] {
            assert!(!cors_allows(bad, SITE), "{bad}");
        }
    }

    #[test]
    fn cors_headers_echo_the_origin_and_allow_the_auth_header() {
        let h = cors_headers("tauri://localhost");
        let get = |k: &str| h.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("access-control-allow-origin"), Some("tauri://localhost"));
        assert!(get("access-control-allow-headers").unwrap().contains("authorization"));
        assert!(get("access-control-allow-credentials").is_none(), "no cookies are involved");
    }
}
