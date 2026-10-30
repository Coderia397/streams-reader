//! Request guards that need no storage: the same-origin check on writes (with `SameSite=Strict` cookies this blocks
//! cross-site request forgery even from same-site subdomains) and the API version check.

use nolabs_core::proto::api::API_VERSION;
use nolabs_core::proto::{ApiError, ErrorCode};

use crate::error::forbidden;

/// Allow safe methods; for everything else require `Sec-Fetch-Site: same-origin`, or, from browsers that do not send
/// it, an `Origin` equal to the request's own origin. A present `Sec-Fetch-Site` with any other value is refused.
pub fn check_same_origin(
    method: &str,
    sec_fetch_site: Option<&str>,
    origin: Option<&str>,
    own_origin: &str,
) -> Result<(), ApiError> {
    if method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD") {
        return Ok(());
    }
    match sec_fetch_site {
        Some(site) if site.eq_ignore_ascii_case("same-origin") => Ok(()),
        Some(_) => Err(forbidden("cross-origin request refused")),
        None if origin.is_some_and(|o| o.eq_ignore_ascii_case(own_origin)) => Ok(()),
        None => Err(forbidden("cross-origin request refused")),
    }
}

/// The app sends `x-nolabs-api: 1`; anything else means the installed app is older or newer than the Worker.
pub fn check_api_version(header: Option<&str>) -> Result<(), ApiError> {
    match header.map(str::trim) {
        Some(v) if v == API_VERSION.to_string() => Ok(()),
        _ => Err(ApiError::new(
            ErrorCode::UpgradeRequired,
            format!("this app version does not match the server (API {API_VERSION}); reload to update"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "https://nolabs.example.dev";

    #[test]
    fn same_origin_table() {
        let cases: &[(&str, Option<&str>, Option<&str>, bool)] = &[
            ("GET", Some("cross-site"), None, true),
            ("HEAD", None, Some("https://evil.dev"), true),
            ("POST", Some("same-origin"), None, true),
            ("POST", Some("same-origin"), Some("https://evil.dev"), true),
            ("PUT", Some("same-site"), Some(ME), false),
            ("DELETE", Some("cross-site"), None, false),
            ("POST", Some("none"), None, false),
            ("POST", None, Some(ME), true),
            ("POST", None, Some("https://nolabs.example.dev.evil.dev"), false),
            ("POST", None, Some("null"), false),
            ("POST", None, None, false),
        ];
        for (method, site, origin, ok) in cases {
            assert_eq!(check_same_origin(method, *site, *origin, ME).is_ok(), *ok, "{method} {site:?} {origin:?}");
        }
        assert_eq!(check_same_origin("POST", None, None, ME).unwrap_err().code, ErrorCode::Forbidden);
    }

    #[test]
    fn version_header_must_match() {
        assert!(check_api_version(Some("1")).is_ok());
        assert!(check_api_version(Some(" 1 ")).is_ok());
        let e = check_api_version(Some("2")).unwrap_err();
        assert_eq!((e.code, e.status()), (ErrorCode::UpgradeRequired, 409));
        assert!(check_api_version(None).is_err());
    }
}
