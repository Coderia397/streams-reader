//! Document bodies. The metadata row lives in Supabase (the app creates it with `ready = false` and the exact
//! `body_bytes`); the body lives in R2 at `docs/<user_id>/<id>/body.json`. An upload is two-phase: the Worker checks
//! the row with the caller's token, streams the PUT body to R2 with the declared length, then sets `ready`. A crash in
//! between leaves a row that is not ready, which the app can upload to again.

use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::MAX_BODY_BYTES;

use crate::app::App;
use crate::auth::AuthUser;
use crate::error::{conflict, too_large};
use crate::repo::Owned;

/// Where the Worker must stream an upload, and the exact length it must have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadTarget {
    pub object_key: String,
    pub length: u64,
}

/// Bodies sit under the owner's id, so one user's keys can never name another's object.
pub fn body_key(user_id: &str, id: &str) -> String {
    format!("docs/{user_id}/{id}/body.json")
}

/// Check that a streamed upload declares exactly the expected length (R2 needs a known length up front).
pub fn check_length(content_length: Option<u64>, expected: u64) -> Result<(), ApiError> {
    match content_length {
        Some(n) if n == expected => Ok(()),
        Some(n) => {
            Err(ApiError::bad_request(format!("Content-Length {n} does not match the declared {expected} bytes")))
        }
        None => Err(ApiError::bad_request("Content-Length is required")),
    }
}

impl App {
    /// Validate a body upload before the Worker streams it.
    pub async fn begin_document_body(
        &self,
        user: &AuthUser,
        id: &str,
        content_length: Option<u64>,
    ) -> Result<UploadTarget, ApiError> {
        let doc =
            self.accounts.document(user.token()?, id).await?.ok_or_else(|| ApiError::not_found("no such document"))?;
        if doc.ready {
            return Err(conflict("the body of this document was already uploaded"));
        }
        if doc.body_bytes > MAX_BODY_BYTES {
            return Err(too_large(format!("documents are limited to {MAX_BODY_BYTES} bytes")));
        }
        check_length(content_length, doc.body_bytes)?;
        Ok(UploadTarget { object_key: body_key(&user.id, id), length: doc.body_bytes })
    }

    /// Mark the body uploaded after R2 accepted it.
    pub async fn finish_document_body(&self, user: &AuthUser, id: &str) -> Result<(), ApiError> {
        if self.accounts.set_document_ready(user.token()?, id).await? {
            Ok(())
        } else {
            Err(ApiError::not_found("document deleted during upload"))
        }
    }

    /// R2 key of a ready document's body that the caller owns.
    pub async fn document_body_key(&self, user: &AuthUser, id: &str) -> Result<String, ApiError> {
        self.check_ready_document(user, id).await?;
        Ok(body_key(&user.id, id))
    }

    /// Delete the Supabase row first (the document disappears from every device at once; progress and bookmarks
    /// cascade), then every object under the owner's `docs/<user_id>/<id>/`.
    pub async fn delete_document(&self, user: &AuthUser, id: &str) -> Result<(), ApiError> {
        if !self.accounts.delete_document(user.token()?, id).await? {
            return Err(ApiError::not_found("no such document"));
        }
        self.repo().forget_owner(Owned::ReadyDocument, id).await?;
        let keys = self.blob.list_prefix(&format!("docs/{}/{id}/", user.id)).await?;
        self.blob.delete_many(&keys).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestEnv, run};
    use nolabs_core::proto::ErrorCode;

    const DOC: &str = "0123456789abcdef";

    #[test]
    fn upload_then_read_back() {
        let env = TestEnv::new();
        let alice = env.user("alice");
        env.accounts.add_document("alice", DOC, 42, false);
        run(async {
            assert_eq!(env.app.document_body_key(&alice, DOC).await.unwrap_err().code, ErrorCode::Conflict);
            assert_eq!(env.app.begin_document_body(&alice, DOC, None).await.unwrap_err().code, ErrorCode::BadRequest);
            assert!(env.app.begin_document_body(&alice, DOC, Some(41)).await.is_err());
            let target = env.app.begin_document_body(&alice, DOC, Some(42)).await.unwrap();
            assert_eq!(target, UploadTarget { object_key: format!("docs/alice/{DOC}/body.json"), length: 42 });
            env.app.finish_document_body(&alice, DOC).await.unwrap();
            assert_eq!(env.app.document_body_key(&alice, DOC).await.unwrap(), target.object_key);
            let e = env.app.begin_document_body(&alice, DOC, Some(42)).await.unwrap_err();
            assert_eq!(e.code, ErrorCode::Conflict);
        });
    }

    #[test]
    fn other_users_documents_are_not_found() {
        let env = TestEnv::new();
        let bob = env.user("bob");
        env.accounts.add_document("alice", DOC, 42, true);
        run(async {
            assert_eq!(env.app.begin_document_body(&bob, DOC, Some(42)).await.unwrap_err().code, ErrorCode::NotFound);
            assert_eq!(env.app.document_body_key(&bob, DOC).await.unwrap_err().code, ErrorCode::NotFound);
            assert_eq!(env.app.delete_document(&bob, DOC).await.unwrap_err().code, ErrorCode::NotFound);
            assert_eq!(env.app.finish_document_body(&bob, DOC).await.unwrap_err().code, ErrorCode::NotFound);
        });
    }

    #[test]
    fn delete_removes_row_objects_and_cached_ownership() {
        let env = TestEnv::new();
        let alice = env.user("alice");
        env.accounts.add_document("alice", DOC, 1, true);
        env.blob.insert(&body_key("alice", DOC), "{}");
        env.blob.insert("docs/alice/other0000000000/body.json", "{}");
        run(async {
            env.app.document_body_key(&alice, DOC).await.unwrap();
            env.app.delete_document(&alice, DOC).await.unwrap();
            assert_eq!(env.blob.keys(), vec!["docs/alice/other0000000000/body.json".to_owned()]);
            assert_eq!(env.app.document_body_key(&alice, DOC).await.unwrap_err().code, ErrorCode::NotFound);
            assert_eq!(env.app.delete_document(&alice, DOC).await.unwrap_err().code, ErrorCode::NotFound);
        });
    }

    #[test]
    fn oversized_rows_are_refused_before_streaming() {
        let env = TestEnv::new();
        let alice = env.user("alice");
        env.accounts.add_document("alice", DOC, MAX_BODY_BYTES + 1, false);
        let e = run(env.app.begin_document_body(&alice, DOC, Some(MAX_BODY_BYTES + 1))).unwrap_err();
        assert_eq!(e.code, ErrorCode::PayloadTooLarge);
    }
}
