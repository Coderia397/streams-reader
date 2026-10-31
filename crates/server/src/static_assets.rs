//! Which files of the two static builds the Worker embeds, and their content type and cache policy. The landing site
//! (`crates/site/dist`) is served at `/`, the web app (`crates/web/dist`) under [`APP_PREFIX`]. The Worker's `build.rs`
//! includes this file with `#[path]` (it has no dependencies for that reason), so the policy is computed once at build
//! time and unit-tested here. The site's rules follow `crates/site/HEADERS.md`.

/// Where the web app lives; the landing site owns everything else.
pub const APP_PREFIX: &str = "/app/";

/// Hashed names never change their content.
pub const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Revalidated with the ETag on every use, so a deploy is picked up at once (HTML, `sw.js`, the manifest).
pub const REVALIDATE: &str = "no-cache";
/// HTML pages of the site: always fresh after a deploy, 304s keep it cheap.
pub const PAGE: &str = "public, max-age=0, must-revalidate";
/// Responses that must never be stored (404 pages).
pub const NO_STORE: &str = "no-store";
const THIRTY_DAYS: &str = "public, max-age=2592000";
const SEVEN_DAYS: &str = "public, max-age=604800";
const ONE_DAY: &str = "public, max-age=86400";
const ONE_HOUR: &str = "public, max-age=3600";

/// True for a URL path served by the web app.
pub fn is_app(path: &str) -> bool {
    path.starts_with(APP_PREFIX)
}

/// The last path segment.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Lowercase extension of the last path segment, or "".
pub fn extension(path: &str) -> String {
    file_name(path).rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

/// Whether a file of a build directory (path relative to it, `/` separators) is published. Hidden files and folders
/// are build bookkeeping (`.site-gen`), except `.well-known`.
pub fn is_published(relative: &str) -> bool {
    relative.split('/').all(|part| !part.starts_with('.') || part == ".well-known")
}

pub fn content_type(path: &str) -> &'static str {
    match extension(path).as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "wasm" => "application/wasm",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "xml" => "application/xml; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "" if file_name(path).eq_ignore_ascii_case("LICENSE") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// True when the file name carries a content hash: trunk's `name-0123456789abcdef.js` and `name-…_bg.wasm`, the
/// site's `site.b6aceb238d.js`. A hash part is at least 8 hex digits with at least one digit, so words are not.
pub fn is_hashed(path: &str) -> bool {
    let name = file_name(path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.split(['-', '_', '.']).any(|part| {
        part.len() >= 8 && part.bytes().all(|b| b.is_ascii_hexdigit()) && part.bytes().any(|b| b.is_ascii_digit())
    })
}

/// The Cache-Control of a file served at this URL path (successful responses; 404s use [`NO_STORE`]).
pub fn cache_control(path: &str) -> &'static str {
    if is_app(path) { app_cache_control(path) } else { site_cache_control(path) }
}

/// The app: hashed files are immutable; everything else (index.html, sw.js, the manifest, icons, fonts, vendored
/// pdf.js) is revalidated with its ETag, so the service worker and a deploy never disagree for long.
fn app_cache_control(path: &str) -> &'static str {
    let name = file_name(path);
    let always_fresh = name == "index.html" || name == "sw.js" || name.starts_with("manifest");
    if !always_fresh && is_hashed(path) { IMMUTABLE } else { REVALIDATE }
}

/// The site, per `crates/site/HEADERS.md`.
fn site_cache_control(path: &str) -> &'static str {
    let name = file_name(path);
    if extension(path) == "html" {
        return PAGE;
    }
    if path.starts_with("/.well-known/") || is_indexnow_key(path) {
        return ONE_DAY;
    }
    if matches!(path, "/robots.txt" | "/sitemap.xml" | "/llms.txt") {
        return ONE_HOUR;
    }
    // Images of the auth emails keep their names for as long as sent mail points at them.
    if path == "/favicon.ico" || path.starts_with("/assets/brand/") || path.starts_with("/email/") {
        return SEVEN_DAYS;
    }
    if path.starts_with("/assets/fonts/") || path.starts_with("/assets/demo/") {
        return THIRTY_DAYS;
    }
    if path.starts_with("/assets/") {
        // Stylesheet and script carry a hash; covers, previews, spots and the hero film keep stable names and are
        // re-rendered rarely, so they get the brand files' week.
        return if is_hashed(name) { IMMUTABLE } else { SEVEN_DAYS };
    }
    PAGE
}

/// The IndexNow key file: `/<32 hex>.txt` at the root.
fn is_indexnow_key(path: &str) -> bool {
    path.strip_prefix('/')
        .and_then(|p| p.strip_suffix(".txt"))
        .is_some_and(|key| !key.contains('/') && key.len() >= 8 && key.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_policy() {
        let cases = [
            ("/app/index.html", "text/html; charset=utf-8", REVALIDATE),
            ("/app/sw.js", "text/javascript; charset=utf-8", REVALIDATE),
            ("/app/manifest.webmanifest", "application/manifest+json", REVALIDATE),
            ("/app/nolabs-web-3f2a9c1d0b8e7f6a.js", "text/javascript; charset=utf-8", IMMUTABLE),
            ("/app/nolabs-web-3f2a9c1d0b8e7f6a_bg.wasm", "application/wasm", IMMUTABLE),
            ("/app/boot-ec96cf5b543af34c.js", "text/javascript; charset=utf-8", IMMUTABLE),
            ("/app/import/loader-0a1b2c3d4e5f6a7b.js", "text/javascript; charset=utf-8", IMMUTABLE),
            ("/app/main-0a1b2c3d.css", "text/css; charset=utf-8", IMMUTABLE),
            ("/app/assets/brand/icons/icon-192.png", "image/png", REVALIDATE),
            ("/app/assets/fonts/Inter.WOFF2", "font/woff2", REVALIDATE),
            ("/app/assets/vendor/pdfjs/pdf.worker.min.mjs", "text/javascript; charset=utf-8", REVALIDATE),
            ("/app/assets/vendor/pdfjs/LICENSE", "text/plain; charset=utf-8", REVALIDATE),
            ("/app/README", "application/octet-stream", REVALIDATE),
        ];
        for (path, ct, cc) in cases {
            assert!(is_app(path), "{path}");
            assert_eq!((content_type(path), cache_control(path)), (ct, cc), "{path}");
        }
    }

    #[test]
    fn site_policy_follows_headers_md() {
        let cases = [
            ("/index.html", "text/html; charset=utf-8", PAGE),
            ("/ar/download/index.html", "text/html; charset=utf-8", PAGE),
            ("/404.html", "text/html; charset=utf-8", PAGE),
            ("/assets/site.b6aceb238d.js", "text/javascript; charset=utf-8", IMMUTABLE),
            ("/assets/landing.67319f46ad.css", "text/css; charset=utf-8", IMMUTABLE),
            ("/assets/fonts/inter-latin.woff2", "font/woff2", THIRTY_DAYS),
            ("/assets/demo/af_heart.mp3", "audio/mpeg", THIRTY_DAYS),
            ("/assets/demo/article-bm_george.json", "application/json", THIRTY_DAYS),
            ("/assets/brand/web/og-image.png", "image/png", SEVEN_DAYS),
            ("/favicon.ico", "image/x-icon", SEVEN_DAYS),
            ("/email/card-en.png", "image/png", SEVEN_DAYS),
            ("/email/logo-on-light.png", "image/png", SEVEN_DAYS),
            ("/assets/media/hero-loop.mp4", "video/mp4", SEVEN_DAYS),
            ("/assets/media/hero-loop.poster.avif", "image/avif", SEVEN_DAYS),
            ("/assets/previews/moby-dick.webm", "video/webm", SEVEN_DAYS),
            ("/robots.txt", "text/plain; charset=utf-8", ONE_HOUR),
            ("/sitemap.xml", "application/xml; charset=utf-8", ONE_HOUR),
            ("/llms.txt", "text/plain; charset=utf-8", ONE_HOUR),
            ("/.well-known/security.txt", "text/plain; charset=utf-8", ONE_DAY),
            ("/5f1c2a9e7b4d4c3fa8e06d1b92c7e4a1.txt", "text/plain; charset=utf-8", ONE_DAY),
        ];
        for (path, ct, cc) in cases {
            assert!(!is_app(path), "{path}");
            assert_eq!((content_type(path), cache_control(path)), (ct, cc), "{path}");
        }
    }

    #[test]
    fn hashes_are_recognised_and_words_are_not() {
        for hashed in ["/a/b/app-1eadbeef.js", "/site.b6aceb238d.js", "/x-0123456789abcdef_bg.wasm"] {
            assert!(is_hashed(hashed), "{hashed}");
        }
        for plain in ["/application.js", "/deadbeefcafe.js", "/hero-loop.mp4", "/sw.js", "/a1b2c3.js"] {
            assert!(!is_hashed(plain), "{plain}");
        }
    }

    #[test]
    fn hidden_files_are_not_published_except_well_known() {
        assert!(is_published("index.html"));
        assert!(is_published(".well-known/security.txt"));
        assert!(!is_published(".site-gen"));
        assert!(!is_published("assets/.DS_Store"));
        assert!(!is_published(".tmp/x.js"));
    }
}
