//! `GET /api/fetch?url=…`: fetch a web page for the app to import (the app parses it; browsers cannot read
//! cross-origin pages). Every hop of a redirect chain is validated again, the whole chain shares one 15 s deadline,
//! bodies are capped at 5 MB, only text content types pass, and no cookie or credential is ever sent.

pub mod url_guard;

use nolabs_core::proto::{ApiError, ErrorCode};
use url::Url;

use crate::app::App;
use crate::error::too_large;
use crate::ports::{HttpError, HttpRequest, Method, header_value};
use url_guard::validate_url;

pub const MAX_REDIRECTS: usize = 5;
pub const TIMEOUT_MS: i64 = 15_000;
pub const MAX_BYTES: u64 = 5 * 1024 * 1024;
pub const ALLOWED_TYPES: &[&str] = &["text/html", "application/xhtml+xml", "text/plain", "text/markdown"];
const USER_AGENT: &str = "Mozilla/5.0 (compatible; nolabs-reader/1; personal read-aloud app)";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub final_url: String,
    /// The upstream `Content-Type`, as received (media type is on the allowlist).
    pub upstream_type: String,
    pub body: Vec<u8>,
}

impl Fetched {
    /// What the Worker serves the bytes as. Never the upstream HTML type: a fetched page must not be able to render
    /// as a document on our origin. The charset is kept so the app can decode the text.
    pub fn content_type(&self) -> String {
        match charset(&self.upstream_type) {
            Some(cs) => format!("text/plain; charset={cs}"),
            None => "text/plain".to_owned(),
        }
    }
}

fn upstream(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::UpstreamFetch, message)
}

fn media_type(content_type: &str) -> String {
    content_type.split(';').next().unwrap_or_default().trim().to_ascii_lowercase()
}

fn charset(content_type: &str) -> Option<String> {
    content_type.split(';').skip(1).find_map(|p| {
        let (k, v) = p.split_once('=')?;
        let v = v.trim().trim_matches('"');
        let ok = !v.is_empty() && v.len() <= 40 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b));
        (k.trim().eq_ignore_ascii_case("charset") && ok).then(|| v.to_ascii_lowercase())
    })
}

impl App {
    pub async fn fetch_page(&self, raw_url: &str) -> Result<Fetched, ApiError> {
        let mut url: Url = validate_url(raw_url)?;
        let deadline = self.now_ms() + TIMEOUT_MS;
        for _ in 0..=MAX_REDIRECTS {
            let left = deadline - self.now_ms();
            if left <= 0 {
                return Err(upstream("the page took too long to load"));
            }
            let req = HttpRequest::new(Method::Get, url.as_str())
                .header("user-agent", USER_AGENT)
                .header("accept", "text/html,application/xhtml+xml,text/plain;q=0.9,text/markdown;q=0.9")
                .timeout_ms(u32::try_from(left).unwrap_or(u32::MAX))
                .max_body_bytes(MAX_BYTES);
            let res = self.http.send(req).await.map_err(|e| match e {
                HttpError::Timeout => upstream("the page took too long to load"),
                HttpError::TooLarge => too_large("the page is larger than 5 MB"),
                HttpError::Network(m) => upstream(format!("could not load the page: {m}")),
            })?;
            if matches!(res.status, 301 | 302 | 303 | 307 | 308) {
                let location = res.header("location").ok_or_else(|| upstream("redirect without a location"))?;
                let next = url.join(location).map_err(|_| upstream("redirect to an invalid address"))?;
                url = validate_url(next.as_str())?;
                continue;
            }
            if !(200..300).contains(&res.status) {
                return Err(upstream(format!("the site answered HTTP {}", res.status)));
            }
            let content_type = header_value(&res.headers, "content-type").unwrap_or_default().to_owned();
            if !ALLOWED_TYPES.contains(&media_type(&content_type).as_str()) {
                return Err(ApiError::bad_request(format!(
                    "unsupported content type {:?}; only web pages and text can be imported",
                    media_type(&content_type)
                )));
            }
            return Ok(Fetched { final_url: url.to_string(), upstream_type: content_type, body: res.body });
        }
        Err(upstream("too many redirects"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fakes::response;
    use crate::testing::{TestEnv, run};

    #[test]
    fn follows_redirects_and_revalidates_each_hop() {
        let env = TestEnv::new();
        env.http.set(|req| {
            assert!(req.headers.iter().all(|(k, _)| k != "cookie" && k != "authorization"));
            Ok(match req.url.as_str() {
                "https://a.example/start" => response(301, &[("location", "/next")], ""),
                "https://a.example/next" => response(302, &[("location", "https://b.example/page")], ""),
                "https://b.example/page" => {
                    response(200, &[("content-type", "text/html; charset=ISO-8859-1")], "<p>hi")
                }
                other => panic!("unexpected {other}"),
            })
        });
        let f = run(env.app.fetch_page("https://a.example/start#x")).unwrap();
        assert_eq!(f.final_url, "https://b.example/page");
        assert_eq!(f.body, b"<p>hi");
        assert_eq!(f.content_type(), "text/plain; charset=iso-8859-1");
        assert_eq!(env.http.requests.borrow().len(), 3);
    }

    #[test]
    fn redirect_into_a_private_network_is_refused() {
        let env = TestEnv::new();
        env.http.set(|_| Ok(response(302, &[("location", "http://169.254.169.254/latest")], "")));
        let e = run(env.app.fetch_page("https://a.example/")).unwrap_err();
        assert_eq!(e.code, ErrorCode::BadRequest);
        assert_eq!(env.http.requests.borrow().len(), 1);
    }

    #[test]
    fn redirect_loops_stop_after_five_hops() {
        let env = TestEnv::new();
        env.http.set(|_| Ok(response(307, &[("location", "https://a.example/again")], "")));
        let e = run(env.app.fetch_page("https://a.example/")).unwrap_err();
        assert_eq!((e.code, e.message.as_str()), (ErrorCode::UpstreamFetch, "too many redirects"));
        assert_eq!(env.http.requests.borrow().len(), MAX_REDIRECTS + 1);
    }

    #[test]
    fn content_type_allowlist_size_cap_and_errors() {
        let env = TestEnv::new();
        env.http.set(|req| {
            Ok(match req.url.as_str() {
                "https://a.example/pdf" => response(200, &[("content-type", "application/pdf")], "%PDF"),
                "https://a.example/none" => response(200, &[], "x"),
                "https://a.example/md" => response(200, &[("content-type", "text/markdown")], "# hi"),
                "https://a.example/big" => response(200, &[("content-type", "text/html")], vec![b'x'; 6 << 20]),
                "https://a.example/gone" => response(404, &[], ""),
                _ => return Err(HttpError::Timeout),
            })
        });
        let code = |u: &str| run(env.app.fetch_page(u)).unwrap_err().code;
        assert_eq!(code("https://a.example/pdf"), ErrorCode::BadRequest);
        assert_eq!(code("https://a.example/none"), ErrorCode::BadRequest);
        assert_eq!(code("https://a.example/big"), ErrorCode::PayloadTooLarge);
        assert_eq!(code("https://a.example/gone"), ErrorCode::UpstreamFetch);
        assert_eq!(code("https://a.example/slow"), ErrorCode::UpstreamFetch);
        let md = run(env.app.fetch_page("https://a.example/md")).unwrap();
        assert_eq!(md.content_type(), "text/plain");
        let limits = env.http.requests.borrow().iter().map(|r| (r.timeout_ms, r.max_body_bytes)).collect::<Vec<_>>();
        assert!(limits.iter().all(|&(t, m)| t <= 15_000 && m == MAX_BYTES));
    }

    #[test]
    fn charset_parsing() {
        assert_eq!(charset("text/html; charset=\"UTF-8\""), Some("utf-8".into()));
        assert_eq!(charset("text/html;charset=utf-8;x=y"), Some("utf-8".into()));
        assert_eq!(charset("text/html; charset=a b"), None);
        assert_eq!(charset("text/html"), None);
    }
}
