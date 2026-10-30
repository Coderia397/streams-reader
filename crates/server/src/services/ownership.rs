//! Does this user own that object? Supabase answers (row-level security hides other users' rows from the caller's
//! token), and positive answers are cached in D1 for [`CACHE_MS`] so a reading session does not query PostgREST per
//! request. Negative answers are never cached: a row created a second ago must work at once. Deleting an object
//! drops its cache entries, so a deleted document stops serving within one request, not five minutes.

use nolabs_core::proto::ApiError;

use crate::app::App;
use crate::auth::AuthUser;
use crate::repo::Owned;

pub const CACHE_MS: i64 = 5 * 60 * 1000;

impl App {
    /// Ok when the caller owns the document and its body is uploaded; 404 otherwise, 409 while not uploaded.
    pub(crate) async fn check_ready_document(&self, user: &AuthUser, id: &str) -> Result<(), ApiError> {
        let now = self.now_ms();
        let repo = self.repo();
        if repo.owner_cached(Owned::ReadyDocument, id, &user.id, now).await? {
            return Ok(());
        }
        match self.accounts.document(user.token()?, id).await? {
            Some(doc) if doc.ready => {
                repo.cache_owner(Owned::ReadyDocument, id, &user.id, now + CACHE_MS).await?;
                Ok(())
            }
            Some(_) => Err(crate::error::conflict("the document body has not been uploaded yet")),
            None => Err(ApiError::not_found("no such document")),
        }
    }

    /// Ok when the caller owns the voice in Supabase (the Worker's own row must say so too; see voices).
    pub(crate) async fn check_voice_owner(&self, user: &AuthUser, id: &str) -> Result<(), ApiError> {
        let now = self.now_ms();
        let repo = self.repo();
        if repo.owner_cached(Owned::Voice, id, &user.id, now).await? {
            return Ok(());
        }
        if self.accounts.voice_exists(user.token()?, id).await? {
            repo.cache_owner(Owned::Voice, id, &user.id, now + CACHE_MS).await?;
            Ok(())
        } else {
            Err(ApiError::not_found("no such voice"))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{TestEnv, run};
    use nolabs_core::proto::ErrorCode;

    const DOC: &str = "0123456789abcdef";

    #[test]
    fn positive_answers_are_cached_and_negative_ones_are_not() {
        let env = TestEnv::new();
        let alice = env.user("alice");
        let bob = env.user("bob");
        env.accounts.add_document("alice", DOC, 10, true);
        run(async {
            env.app.check_ready_document(&alice, DOC).await.unwrap();
            env.app.check_ready_document(&alice, DOC).await.unwrap();
            assert_eq!(env.accounts.calls(), 1, "second check served from D1");
            for _ in 0..2 {
                let e = env.app.check_ready_document(&bob, DOC).await.unwrap_err();
                assert_eq!(e.code, ErrorCode::NotFound);
            }
            assert_eq!(env.accounts.calls(), 3, "refusals are asked again every time");
        });
    }

    #[test]
    fn the_cache_expires() {
        let env = TestEnv::new();
        let alice = env.user("alice");
        env.accounts.add_document("alice", DOC, 10, true);
        run(env.app.check_ready_document(&alice, DOC)).unwrap();
        env.accounts.remove_document(DOC);
        env.advance(super::CACHE_MS - 1);
        run(env.app.check_ready_document(&alice, DOC)).unwrap();
        env.advance(1);
        assert_eq!(run(env.app.check_ready_document(&alice, DOC)).unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn documents_without_a_body_are_not_cached() {
        let env = TestEnv::new();
        let alice = env.user("alice");
        env.accounts.add_document("alice", DOC, 10, false);
        assert_eq!(run(env.app.check_ready_document(&alice, DOC)).unwrap_err().code, ErrorCode::Conflict);
        assert_eq!(env.db.scalar("SELECT count(*) FROM owner_cache"), serde_json::json!(0));
    }

    #[test]
    fn audio_token_callers_cannot_reach_supabase() {
        let env = TestEnv::new();
        let mut alice = env.user("alice");
        alice.access_token = None;
        assert_eq!(run(env.app.check_voice_owner(&alice, DOC)).unwrap_err().code, ErrorCode::Unauthorized);
    }
}
