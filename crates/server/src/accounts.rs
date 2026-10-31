//! The user's own rows in Supabase, reached with the user's access token so row-level security decides what they may
//! see and change; the Worker holds no key that could do more (decision D10). [`Accounts`] is the port the services
//! use; [`Postgrest`] implements it over PostgREST, and the tests use an in-memory fake.

use async_trait::async_trait;
use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::VoiceState;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::unauthorized;
use crate::ports::{Http, HttpRequest, HttpResponse, Method};

/// What the Worker needs to know about a document row before touching its body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct DocumentAccess {
    pub ready: bool,
    pub body_bytes: u64,
}

/// A new `public.voices` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewVoice<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub transcript: &'a str,
}

#[async_trait(?Send)]
pub trait Accounts {
    /// The caller's document, or None when it does not exist or is someone else's (RLS hides it either way).
    async fn document(&self, token: &str, id: &str) -> Result<Option<DocumentAccess>, ApiError>;
    /// Mark the body uploaded; false when the row is gone.
    async fn set_document_ready(&self, token: &str, id: &str) -> Result<bool, ApiError>;
    /// Delete the row (progress and bookmarks cascade); false when there was none to delete.
    async fn delete_document(&self, token: &str, id: &str) -> Result<bool, ApiError>;
    async fn voice_exists(&self, token: &str, id: &str) -> Result<bool, ApiError>;
    async fn insert_voice(&self, token: &str, voice: &NewVoice<'_>) -> Result<(), ApiError>;
    /// Mirror the pipeline state onto the user's row; false when the row is gone.
    async fn set_voice_state(
        &self,
        token: &str,
        id: &str,
        state: VoiceState,
        ref_sha256: Option<&str>,
    ) -> Result<bool, ApiError>;
    async fn delete_voice(&self, token: &str, id: &str) -> Result<bool, ApiError>;
    /// The caller's daily GPU share in seconds for a deployment cap (`public.gpu_share`).
    async fn gpu_share(&self, token: &str, daily_cap_s: u32) -> Result<u32, ApiError>;
}

const TIMEOUT_MS: u32 = 10_000;
const MAX_BYTES: u64 = 256 * 1024;

/// [`Accounts`] over PostgREST (`<url>/rest/v1`), with the publishable key as `apikey` and the user's token as
/// `Authorization`.
pub struct Postgrest {
    pub http: Box<dyn Http>,
    pub url: String,
    pub anon_key: String,
}

impl Postgrest {
    fn request(&self, method: Method, path: &str, token: &str) -> HttpRequest {
        HttpRequest::new(method, format!("{}/rest/v1/{path}", self.url))
            .header("apikey", self.anon_key.clone())
            .header("authorization", format!("Bearer {token}"))
            .header("accept", "application/json")
            .timeout_ms(TIMEOUT_MS)
            .max_body_bytes(MAX_BYTES)
    }

    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, ApiError> {
        let what = format!("{} {}", req.method.as_str(), req.url.split('?').next().unwrap_or_default());
        let resp = self.http.send(req).await.map_err(|e| ApiError::internal(format!("supabase {what}: {e}")))?;
        match resp.status {
            200..=299 => Ok(resp),
            // PostgREST answers 401 for an expired or revoked token even when the Worker's check passed a second ago.
            401 => Err(unauthorized("the session is no longer valid; sign in again")),
            s => {
                let body: String = String::from_utf8_lossy(&resp.body).chars().take(200).collect();
                Err(ApiError::internal(format!("supabase {what}: HTTP {s}: {body}")))
            }
        }
    }

    async fn rows<T: for<'de> Deserialize<'de>>(&self, req: HttpRequest) -> Result<Vec<T>, ApiError> {
        let resp = self.send(req).await?;
        serde_json::from_slice(&resp.body).map_err(|e| ApiError::internal(format!("supabase rows: {e}")))
    }

    /// Run a write that asks for the changed rows back and count them.
    async fn changed(&self, req: HttpRequest) -> Result<bool, ApiError> {
        let rows: Vec<serde_json::Value> = self.rows(req.header("prefer", "return=representation")).await?;
        Ok(!rows.is_empty())
    }
}

/// `id=eq.<id>` for an id that was validated by the route table (16 chars of Crockford base32, nothing to escape).
fn by_id(id: &str) -> String {
    format!("id=eq.{id}")
}

fn json_body(value: &serde_json::Value) -> Vec<u8> {
    value.to_string().into_bytes()
}

#[async_trait(?Send)]
impl Accounts for Postgrest {
    async fn document(&self, token: &str, id: &str) -> Result<Option<DocumentAccess>, ApiError> {
        let req = self.request(Method::Get, &format!("documents?select=ready,body_bytes&{}", by_id(id)), token);
        Ok(self.rows::<DocumentAccess>(req).await?.into_iter().next())
    }

    async fn set_document_ready(&self, token: &str, id: &str) -> Result<bool, ApiError> {
        let req = self
            .request(Method::Patch, &format!("documents?{}&select=id", by_id(id)), token)
            .json_body(json_body(&json!({ "ready": true })));
        self.changed(req).await
    }

    async fn delete_document(&self, token: &str, id: &str) -> Result<bool, ApiError> {
        self.changed(self.request(Method::Delete, &format!("documents?{}&select=id", by_id(id)), token)).await
    }

    async fn voice_exists(&self, token: &str, id: &str) -> Result<bool, ApiError> {
        let req = self.request(Method::Get, &format!("voices?select=id&{}", by_id(id)), token);
        Ok(!self.rows::<serde_json::Value>(req).await?.is_empty())
    }

    async fn insert_voice(&self, token: &str, voice: &NewVoice<'_>) -> Result<(), ApiError> {
        let body = serde_json::to_value(voice).map_err(|e| ApiError::internal(e.to_string()))?;
        let req = self.request(Method::Post, "voices", token).json_body(json_body(&body));
        self.send(req).await.map(drop)
    }

    async fn set_voice_state(
        &self,
        token: &str,
        id: &str,
        state: VoiceState,
        ref_sha256: Option<&str>,
    ) -> Result<bool, ApiError> {
        let mut patch = json!({ "state": crate::repo::enum_text(&state) });
        if let Some(sha) = ref_sha256 {
            patch["ref_sha256"] = sha.into();
        }
        let req =
            self.request(Method::Patch, &format!("voices?{}&select=id", by_id(id)), token).json_body(json_body(&patch));
        self.changed(req).await
    }

    async fn delete_voice(&self, token: &str, id: &str) -> Result<bool, ApiError> {
        self.changed(self.request(Method::Delete, &format!("voices?{}&select=id", by_id(id)), token)).await
    }

    async fn gpu_share(&self, token: &str, daily_cap_s: u32) -> Result<u32, ApiError> {
        let req = self
            .request(Method::Post, "rpc/gpu_share", token)
            .json_body(json_body(&json!({ "daily_cap_s": daily_cap_s })));
        let resp = self.send(req).await?;
        // A user without a profile row gets null: no share.
        let share: Option<i64> =
            serde_json::from_slice(&resp.body).map_err(|e| ApiError::internal(format!("gpu_share: {e}")))?;
        Ok(share.map_or(0, |s| u32::try_from(s.max(0)).unwrap_or(0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::HttpError;
    use crate::testing::fakes::{FakeHttp, response};
    use crate::testing::run;
    use nolabs_core::proto::ErrorCode;

    fn client(http: &FakeHttp) -> Postgrest {
        Postgrest {
            http: Box::new(http.clone()),
            url: "https://ref.supabase.co".into(),
            anon_key: "sb_publishable_x".into(),
        }
    }

    const ID: &str = "0123456789abcdef";

    #[test]
    fn reads_send_the_users_token_and_the_publishable_key() {
        let http = FakeHttp::default();
        http.set(|_| Ok(response(200, &[], r#"[{"ready":true,"body_bytes":42}]"#)));
        let got = run(client(&http).document("tok", ID)).unwrap();
        assert_eq!(got, Some(DocumentAccess { ready: true, body_bytes: 42 }));
        let req = http.requests.borrow()[0].clone();
        assert_eq!(req.url, format!("https://ref.supabase.co/rest/v1/documents?select=ready,body_bytes&id=eq.{ID}"));
        assert_eq!(crate::ports::header_value(&req.headers, "authorization"), Some("Bearer tok"));
        assert_eq!(crate::ports::header_value(&req.headers, "apikey"), Some("sb_publishable_x"));
    }

    #[test]
    fn rows_hidden_by_rls_read_as_missing() {
        let http = FakeHttp::default();
        http.set(|_| Ok(response(200, &[], "[]")));
        let c = client(&http);
        assert_eq!(run(c.document("tok", ID)).unwrap(), None);
        assert!(!run(c.set_document_ready("tok", ID)).unwrap());
        assert!(!run(c.delete_voice("tok", ID)).unwrap());
        assert!(!run(c.voice_exists("tok", ID)).unwrap());
        let methods: Vec<String> =
            http.urls().iter().map(|u| u.split(' ').next().unwrap_or_default().to_owned()).collect();
        assert_eq!(methods, ["GET", "PATCH", "DELETE", "GET"]);
        let patch = http.requests.borrow()[1].clone();
        assert_eq!(crate::ports::header_value(&patch.headers, "prefer"), Some("return=representation"));
        assert_eq!(patch.body.as_deref(), Some(br#"{"ready":true}"#.as_slice()));
    }

    #[test]
    fn voice_writes_and_gpu_share() {
        let http = FakeHttp::default();
        http.set(|req| match req.url.as_str() {
            u if u.ends_with("rpc/gpu_share") => Ok(response(200, &[], "60")),
            u if u.ends_with("/voices") => Ok(response(201, &[], "")),
            _ => Ok(response(200, &[], r#"[{"id":"x"}]"#)),
        });
        let c = client(&http);
        run(c.insert_voice("tok", &NewVoice { id: ID, name: "Me", transcript: "Hi." })).unwrap();
        assert!(run(c.set_voice_state("tok", ID, VoiceState::Ready, Some(&"a".repeat(64)))).unwrap());
        assert_eq!(run(c.gpu_share("tok", 280)).unwrap(), 60);
        let reqs = http.requests.borrow();
        let insert: serde_json::Value = serde_json::from_slice(reqs[0].body.as_deref().unwrap()).unwrap();
        assert_eq!(insert, json!({"id": ID, "name": "Me", "transcript": "Hi."}));
        let patch: serde_json::Value = serde_json::from_slice(reqs[1].body.as_deref().unwrap()).unwrap();
        assert_eq!(patch["state"], "ready");
        assert_eq!(reqs[2].body.as_deref(), Some(br#"{"daily_cap_s":280}"#.as_slice()));
    }

    #[test]
    fn errors_map_to_api_errors() {
        let http = FakeHttp::default();
        http.set(|_| Ok(response(401, &[], r#"{"message":"JWT expired"}"#)));
        assert_eq!(run(client(&http).document("tok", ID)).unwrap_err().code, ErrorCode::Unauthorized);
        http.set(|_| Ok(response(500, &[], "boom")));
        assert_eq!(run(client(&http).document("tok", ID)).unwrap_err().code, ErrorCode::Internal);
        http.set(|_| Err(HttpError::Timeout));
        assert_eq!(run(client(&http).gpu_share("tok", 280)).unwrap_err().code, ErrorCode::Internal);
        http.set(|_| Ok(response(200, &[], "null")));
        assert_eq!(run(client(&http).gpu_share("tok", 280)).unwrap(), 0);
    }
}
