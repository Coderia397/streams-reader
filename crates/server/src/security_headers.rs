//! The security headers of every Worker response, by surface. Both browser surfaces get a strict CSP without inline
//! scripts: the landing site needs nothing but its own files; the web app also compiles wasm and talks to the
//! Supabase project directly (PostgREST, Auth, Realtime over WebSocket; decision D10). The API keeps the app's
//! policy, which is harmless on JSON and media.

/// The Supabase project the apps use, for when the Worker var is missing or malformed.
pub const SUPABASE_PROJECT_URL: &str = "https://atxbmgfuxjtcwmoaldqy.supabase.co";

const HSTS: &str = "max-age=63072000; includeSubDomains";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// The landing site at `/`.
    Site,
    /// The web app under `/app/`.
    App,
    /// `/api`, `/hooks` and `/updates`: called by the apps and services, not navigated to.
    Api,
}

impl Surface {
    pub fn of(path: &str) -> Surface {
        let under = |prefix: &str| path == prefix || path.starts_with(&format!("{prefix}/"));
        if under("/api") || under("/hooks") || under("/updates") {
            Surface::Api
        } else if under("/app") {
            Surface::App
        } else {
            Surface::Site
        }
    }
}

/// `https://<ref>.supabase.co` (no trailing slash) from the Worker var, else the project's own.
fn supabase_origin(configured: Option<&str>) -> String {
    configured
        .map(|u| u.trim().trim_end_matches('/'))
        .filter(|u| u.starts_with("https://") && u.len() > "https://".len() && !u["https://".len()..].contains('/'))
        .unwrap_or(SUPABASE_PROJECT_URL)
        .to_owned()
}

pub fn csp(surface: Surface, supabase_url: Option<&str>) -> String {
    match surface {
        Surface::Site => "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; \
             font-src 'self'; media-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; \
             form-action 'self'; frame-ancestors 'none'; upgrade-insecure-requests"
            .to_owned(),
        Surface::App | Surface::Api => {
            let https = supabase_origin(supabase_url);
            let wss = https.replacen("https://", "wss://", 1);
            format!(
                "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; \
                 img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self' {https} {wss}; \
                 worker-src 'self'; manifest-src 'self'; font-src 'self'; object-src 'none'; base-uri 'none'; \
                 form-action 'self'; frame-ancestors 'none'; upgrade-insecure-requests"
            )
        }
    }
}

/// Every security header of a response on this surface.
pub fn headers(surface: Surface, supabase_url: Option<&str>) -> Vec<(&'static str, String)> {
    let (referrer, permissions) = match surface {
        // The site links out (GitHub, Hugging Face); origins are fine to share there.
        Surface::Site => {
            ("strict-origin-when-cross-origin", "camera=(), microphone=(), geolocation=(), payment=(), usb=()")
        }
        // The app records voice samples; nothing else is needed.
        Surface::App | Surface::Api => {
            ("no-referrer", "camera=(), microphone=(self), geolocation=(), payment=(), usb=()")
        }
    };
    vec![
        ("content-security-policy", csp(surface, supabase_url)),
        ("strict-transport-security", HSTS.to_owned()),
        ("x-content-type-options", "nosniff".to_owned()),
        ("x-frame-options", "DENY".to_owned()),
        ("referrer-policy", referrer.to_owned()),
        ("permissions-policy", permissions.to_owned()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directive(csp: &str, name: &str) -> Vec<String> {
        csp.split(';')
            .map(str::trim)
            .find_map(|d| d.strip_prefix(name).filter(|rest| rest.starts_with(' ')))
            .map(|rest| rest.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    #[test]
    fn surfaces_by_path() {
        for (path, want) in [
            ("/api", Surface::Api),
            ("/api/v1/session", Surface::Api),
            ("/hooks/auth-email", Surface::Api),
            ("/updates/desktop/latest.json", Surface::Api),
            ("/app", Surface::App),
            ("/app/", Surface::App),
            ("/app/read/1", Surface::App),
            ("/", Surface::Site),
            ("/ar/download", Surface::Site),
            ("/apps", Surface::Site),
            ("/apiary", Surface::Site),
            ("/updates-log", Surface::Site),
        ] {
            assert_eq!(Surface::of(path), want, "{path}");
        }
    }

    #[test]
    fn app_connects_to_the_supabase_project_over_https_and_wss() {
        for configured in [None, Some(""), Some("http://x.example"), Some(SUPABASE_PROJECT_URL)] {
            let policy = csp(Surface::App, configured);
            let connect = directive(&policy, "connect-src");
            assert_eq!(
                connect,
                ["'self'", "https://atxbmgfuxjtcwmoaldqy.supabase.co", "wss://atxbmgfuxjtcwmoaldqy.supabase.co"],
                "{configured:?}"
            );
        }
        let custom = csp(Surface::App, Some("https://other.supabase.co/"));
        assert_eq!(
            directive(&custom, "connect-src"),
            ["'self'", "https://other.supabase.co", "wss://other.supabase.co"]
        );
    }

    #[test]
    fn no_surface_allows_inline_or_foreign_scripts() {
        for surface in [Surface::Site, Surface::App, Surface::Api] {
            let policy = csp(surface, None);
            let script = directive(&policy, "script-src");
            assert_eq!(script.first().map(String::as_str), Some("'self'"), "{surface:?}");
            assert!(!script.iter().any(|s| s == "'unsafe-inline'" || s == "'unsafe-eval'" || s.contains(':')));
            assert_eq!(directive(&policy, "frame-ancestors"), ["'none'"]);
            assert_eq!(directive(&policy, "object-src"), ["'none'"]);
        }
        let site = csp(Surface::Site, None);
        assert_eq!(directive(&site, "style-src"), ["'self'"]);
        assert_eq!(directive(&site, "connect-src"), ["'self'"]);
    }

    #[test]
    fn every_surface_gets_the_full_header_set() {
        for surface in [Surface::Site, Surface::App, Surface::Api] {
            let h = headers(surface, None);
            let names: Vec<&str> = h.iter().map(|(k, _)| *k).collect();
            assert_eq!(
                names,
                [
                    "content-security-policy",
                    "strict-transport-security",
                    "x-content-type-options",
                    "x-frame-options",
                    "referrer-policy",
                    "permissions-policy"
                ]
            );
            let get = |name: &str| h.iter().find(|(k, _)| *k == name).map(|(_, v)| v.as_str()).unwrap_or_default();
            assert_eq!(get("x-content-type-options"), "nosniff");
            assert!(get("strict-transport-security").starts_with("max-age=63072000"));
            let mic = surface == Surface::Site;
            assert_eq!(get("permissions-policy").contains("microphone=()"), mic, "{surface:?}");
        }
    }
}
