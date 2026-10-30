//! The API route table ([`nolabs_core::proto::api`]) as a pure function of method and path, plus which guards each
//! route needs. The Worker dispatches on [`Route`]; keeping the table here lets it be tested natively.

use nolabs_core::cache_key;
use nolabs_core::ids;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Login,
    Logout,
    Session,
    Library,
    CreateDocument,
    GetDocument {
        id: String,
    },
    DeleteDocument {
        id: String,
    },
    PutDocumentBody {
        id: String,
    },
    GetDocumentBody {
        id: String,
    },
    /// `beacon` is true for the POST form used by `navigator.sendBeacon`, which cannot set headers.
    PutProgress {
        id: String,
        beacon: bool,
    },
    Voices,
    CreateVoice,
    PutVoiceReference {
        id: String,
    },
    PrepareVoice {
        id: String,
    },
    DeleteVoice {
        id: String,
    },
    Render,
    RenderStatus,
    Audio {
        key: String,
    },
    Fetch,
    Status,
}

impl Route {
    /// Match an `/api/…` request. Ids and keys are validated here, so handlers get well-formed values; anything
    /// else (unknown path, wrong method, malformed id) is None and answers 404.
    pub fn parse(method: &str, path: &str) -> Option<Route> {
        let rest = path.strip_prefix("/api/")?;
        let parts: Vec<&str> = rest.split('/').collect();
        let id = |s: &str| ids::is_id(s).then(|| s.to_owned());
        // The app addresses user voices by their catalogue id (`user:<id>`, colon raw or percent-encoded).
        let voice = |s: &str| {
            let bare =
                s.strip_prefix("user:").or_else(|| s.strip_prefix("user%3A")).or_else(|| s.strip_prefix("user%3a"));
            id(bare.unwrap_or(s))
        };
        let route = match (method, parts.as_slice()) {
            ("POST", ["session"]) => Route::Login,
            ("DELETE", ["session"]) => Route::Logout,
            ("GET", ["session"]) => Route::Session,
            ("GET", ["library"]) => Route::Library,
            ("POST", ["documents"]) => Route::CreateDocument,
            ("GET", ["documents", d]) => Route::GetDocument { id: id(d)? },
            ("DELETE", ["documents", d]) => Route::DeleteDocument { id: id(d)? },
            ("PUT", ["documents", d, "body"]) => Route::PutDocumentBody { id: id(d)? },
            ("GET", ["documents", d, "body"]) => Route::GetDocumentBody { id: id(d)? },
            ("PUT", ["documents", d, "progress"]) => Route::PutProgress { id: id(d)?, beacon: false },
            ("POST", ["documents", d, "progress"]) => Route::PutProgress { id: id(d)?, beacon: true },
            ("GET", ["voices"]) => Route::Voices,
            ("POST", ["voices"]) => Route::CreateVoice,
            ("PUT", ["voices", v, "reference"]) => Route::PutVoiceReference { id: voice(v)? },
            ("POST", ["voices", v, "prepare"]) => Route::PrepareVoice { id: voice(v)? },
            ("DELETE", ["voices", v]) => Route::DeleteVoice { id: voice(v)? },
            ("POST", ["render"]) => Route::Render,
            ("GET", ["render", "status"]) => Route::RenderStatus,
            ("GET" | "HEAD", ["audio", k]) => {
                let key = k.strip_suffix(".webm").unwrap_or(k);
                cache_key::is_key(key).then(|| Route::Audio { key: key.to_owned() })?
            }
            ("GET", ["fetch"]) => Route::Fetch,
            ("GET", ["status"]) => Route::Status,
            _ => return None,
        };
        Some(route)
    }

    /// Everything except logging in (and out) needs a valid session.
    pub fn needs_session(&self) -> bool {
        !matches!(self, Route::Login | Route::Logout)
    }

    /// The version header is required except where the browser, not the app's fetch wrapper, makes the request:
    /// login (an upgraded app must be able to log in), audio (media element / MSE fetches) and beacons.
    pub fn needs_version_header(&self) -> bool {
        !matches!(self, Route::Login | Route::Audio { .. } | Route::PutProgress { beacon: true, .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0123456789abcdef";

    #[test]
    fn route_table() {
        let key = "a".repeat(64);
        let cases = [
            ("POST", "/api/session".to_owned(), Some(Route::Login)),
            ("DELETE", "/api/session".to_owned(), Some(Route::Logout)),
            ("GET", "/api/library".to_owned(), Some(Route::Library)),
            ("PUT", format!("/api/documents/{ID}/body"), Some(Route::PutDocumentBody { id: ID.into() })),
            ("POST", format!("/api/documents/{ID}/progress"), Some(Route::PutProgress { id: ID.into(), beacon: true })),
            ("PUT", format!("/api/documents/{ID}/progress"), Some(Route::PutProgress { id: ID.into(), beacon: false })),
            ("POST", format!("/api/voices/{ID}/prepare"), Some(Route::PrepareVoice { id: ID.into() })),
            ("POST", format!("/api/voices/user:{ID}/prepare"), Some(Route::PrepareVoice { id: ID.into() })),
            ("PUT", format!("/api/voices/user%3A{ID}/reference"), Some(Route::PutVoiceReference { id: ID.into() })),
            ("DELETE", format!("/api/voices/user:{ID}"), Some(Route::DeleteVoice { id: ID.into() })),
            ("DELETE", "/api/voices/user:nope".to_owned(), None),
            ("GET", "/api/render/status".to_owned(), Some(Route::RenderStatus)),
            ("GET", format!("/api/audio/{key}"), Some(Route::Audio { key: key.clone() })),
            ("GET", format!("/api/audio/{key}.webm"), Some(Route::Audio { key: key.clone() })),
            ("GET", "/api/audio/ABC".to_owned(), None),
            ("GET", "/api/documents/../x".to_owned(), None),
            ("GET", "/api/documents/UPPERCASE0000000".to_owned(), None),
            ("PATCH", "/api/library".to_owned(), None),
            ("GET", "/api/nothing".to_owned(), None),
            ("GET", "/library".to_owned(), None),
            ("GET", "/api/library/".to_owned(), None),
        ];
        for (m, p, want) in cases {
            assert_eq!(Route::parse(m, &p), want, "{m} {p}");
        }
    }

    #[test]
    fn guard_exemptions() {
        assert!(!Route::Login.needs_session());
        assert!(!Route::Login.needs_version_header());
        assert!(Route::Logout.needs_version_header());
        assert!(Route::Session.needs_session());
        assert!(!Route::Audio { key: String::new() }.needs_version_header());
        assert!(Route::Audio { key: String::new() }.needs_session());
        assert!(!Route::PutProgress { id: ID.into(), beacon: true }.needs_version_header());
        assert!(Route::PutProgress { id: ID.into(), beacon: false }.needs_version_header());
        assert!(Route::Render.needs_version_header());
    }
}
