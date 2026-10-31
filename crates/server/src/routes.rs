//! The API route table ([`nolabs_core::proto::api`]) as a pure function of method and path, plus which guards each
//! route needs. The Worker dispatches on [`Route`]; keeping the table here lets it be tested natively.

use nolabs_core::cache_key;
use nolabs_core::ids;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Session,
    AudioToken,
    DeleteDocument { id: String },
    PutDocumentBody { id: String },
    GetDocumentBody { id: String },
    Voices,
    CreateVoice,
    PutVoiceReference { id: String },
    PrepareVoice { id: String },
    DeleteVoice { id: String },
    Render,
    RenderStatus,
    Audio { key: String },
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
            ("GET", ["session"]) => Route::Session,
            ("POST", ["audio-token"]) => Route::AudioToken,
            ("DELETE", ["documents", d]) => Route::DeleteDocument { id: id(d)? },
            ("PUT", ["documents", d, "body"]) => Route::PutDocumentBody { id: id(d)? },
            ("GET", ["documents", d, "body"]) => Route::GetDocumentBody { id: id(d)? },
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

    /// Audio accepts an audio-URL token instead of the access token (media elements cannot send headers); every
    /// other route needs a Supabase access token.
    pub fn accepts_audio_token(&self) -> bool {
        matches!(self, Route::Audio { .. })
    }

    /// The version header is required except where the browser, not the app's fetch wrapper, makes the request:
    /// audio (media element / MSE fetches).
    pub fn needs_version_header(&self) -> bool {
        !matches!(self, Route::Audio { .. })
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
            ("GET", "/api/session".to_owned(), Some(Route::Session)),
            ("POST", "/api/audio-token".to_owned(), Some(Route::AudioToken)),
            ("PUT", format!("/api/documents/{ID}/body"), Some(Route::PutDocumentBody { id: ID.into() })),
            ("GET", format!("/api/documents/{ID}/body"), Some(Route::GetDocumentBody { id: ID.into() })),
            ("DELETE", format!("/api/documents/{ID}"), Some(Route::DeleteDocument { id: ID.into() })),
            // Moved to Supabase (PostgREST), no longer Worker routes.
            ("POST", "/api/session".to_owned(), None),
            ("GET", "/api/library".to_owned(), None),
            ("POST", "/api/documents".to_owned(), None),
            ("GET", format!("/api/documents/{ID}"), None),
            ("PUT", format!("/api/documents/{ID}/progress"), None),
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
            ("PATCH", "/api/status".to_owned(), None),
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
        let audio = Route::Audio { key: String::new() };
        assert!(!audio.needs_version_header() && audio.accepts_audio_token());
        for r in [Route::Session, Route::AudioToken, Route::Render, Route::GetDocumentBody { id: ID.into() }] {
            assert!(r.needs_version_header() && !r.accepts_audio_token(), "{r:?}");
        }
    }
}
