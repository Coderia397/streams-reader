//! Content type and cache policy of the embedded web app files. The Worker's `build.rs` includes this file with
//! `#[path]` (it has no dependencies for that reason), so the policy is unit-tested here once.

/// Lowercase extension of the last path segment, or "".
pub fn extension(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

pub fn content_type(path: &str) -> &'static str {
    match extension(path).as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "wasm" => "application/wasm",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        _ => "application/octet-stream",
    }
}

/// True when the file name carries a content hash (trunk: `name-0123456789abcdef.js`, `name-…_bg.wasm`).
pub fn is_hashed(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.split('.').next().unwrap_or(name);
    stem.split(['-', '_']).any(|part| part.len() >= 8 && part.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Hashed files never change under their name; everything else (index.html, sw.js, the manifest, icons) is
/// revalidated with its ETag on every load so a deploy is picked up at once.
pub fn cache_control(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    let always_fresh = name == "index.html" || name == "sw.js" || name.starts_with("manifest");
    if !always_fresh && is_hashed(path) { "public, max-age=31536000, immutable" } else { "no-cache" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_table() {
        let cases = [
            ("/index.html", "text/html; charset=utf-8", "no-cache"),
            ("/sw.js", "text/javascript; charset=utf-8", "no-cache"),
            ("/manifest.webmanifest", "application/manifest+json", "no-cache"),
            (
                "/nolabs-web-3f2a9c1d0b8e7f6a.js",
                "text/javascript; charset=utf-8",
                "public, max-age=31536000, immutable",
            ),
            ("/nolabs-web-3f2a9c1d0b8e7f6a_bg.wasm", "application/wasm", "public, max-age=31536000, immutable"),
            ("/style-0a1b2c3d.css", "text/css; charset=utf-8", "public, max-age=31536000, immutable"),
            ("/icons/icon-192.png", "image/png", "no-cache"),
            ("/fonts/Inter.WOFF2", "font/woff2", "no-cache"),
            ("/README", "application/octet-stream", "no-cache"),
        ];
        for (path, ct, cc) in cases {
            assert_eq!((content_type(path), cache_control(path)), (ct, cc), "{path}");
        }
        assert!(!is_hashed("/application.js"));
        assert!(is_hashed("/a/b/app-deadbeef.js"));
    }
}
