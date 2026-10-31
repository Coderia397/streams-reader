//! The voice catalogue (built-in presets plus the caller's own voices) and the clone flow: create the row, stream
//! the recording to R2, then queue a CPU job that trims it into a reference clip (`prepare_reference` on the Space).
//!
//! A user voice has two halves. The Supabase row (`public.voices`, owner-only) is the user's record and the
//! authority on ownership: a voice deleted there is gone. The D1 row holds what the render pipeline needs (R2 keys,
//! the kept transcript span) and the owner's id. The queue consumer finishes preparations without any user token, so
//! the Supabase `state` is brought up to date the next time the owner lists their voices ([`App::voice_catalog`]).

use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::{CreateVoice, Engine, RenderState, Tier, VoiceCatalog, VoiceInfo, VoiceState};
use nolabs_core::proto::engine::{CONTRACT_VERSION, PrepareReferenceRequest};
use nolabs_core::voices::{DEFAULT_VOICE, builtin_catalog};

use crate::accounts::NewVoice;
use crate::app::App;
use crate::auth::AuthUser;
use crate::error::{conflict, to_json, too_large};
use crate::ports::JobMessage;
use crate::repo::{JobKind, NewJob, Owned, Repo, VoiceRow};
use crate::services::documents::{UploadTarget, check_length};
use crate::services::ownership::CACHE_MS;

/// A 60 s mono WAV at 48 kHz/16-bit is 5.8 MB; leave room for longer takes.
pub const MAX_REFERENCE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TRANSCRIPT_CHARS: usize = 4000;

pub fn raw_key(id: &str) -> String {
    format!("voices/{id}/raw.wav")
}

pub fn ref_key(id: &str) -> String {
    format!("voices/{id}/ref.wav")
}

/// Catalogue id of a user voice.
pub fn user_voice_id(id: &str) -> String {
    format!("user:{id}")
}

fn voice_info(v: &VoiceRow) -> VoiceInfo {
    let description = match (&v.state, &v.error) {
        (VoiceState::Failed, Some(e)) => format!("Preparation failed: {e}"),
        _ => "Your own voice.".to_owned(),
    };
    VoiceInfo {
        id: user_voice_id(&v.id),
        engine: Engine::Ming,
        tier: Tier::Expressive,
        kind: v.kind,
        name: v.name.clone(),
        gender: None,
        accent: None,
        description,
        featured: true,
        state: v.state,
        preview_key: None,
    }
}

impl App {
    /// Presets plus the caller's voices. Voices whose pipeline state changed since the Supabase row was last
    /// written are mirrored now; a voice whose Supabase row is gone was deleted by the user elsewhere and is
    /// cleaned up here.
    pub async fn voice_catalog(&self, user: &AuthUser) -> Result<VoiceCatalog, ApiError> {
        let mut voices = builtin_catalog();
        for v in self.repo().list_voices(&user.id).await? {
            if v.synced_state != Some(v.state) {
                let token = user.token()?;
                if !self.accounts.set_voice_state(token, &v.id, v.state, v.ref_sha256.as_deref()).await? {
                    self.purge_voice(&v.id).await?;
                    continue;
                }
                self.repo().set_voice_synced(&v.id, v.state).await?;
            }
            voices.push(voice_info(&v));
        }
        Ok(VoiceCatalog { voices, default_voice: DEFAULT_VOICE.to_owned() })
    }

    pub async fn create_voice(&self, user: &AuthUser, req: CreateVoice) -> Result<VoiceInfo, ApiError> {
        let name = req.name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return Err(ApiError::bad_request("name must have 1 to 60 characters"));
        }
        let transcript = req.transcript.trim();
        if transcript.is_empty() || transcript.chars().count() > MAX_TRANSCRIPT_CHARS {
            return Err(ApiError::bad_request(format!("transcript must have 1 to {MAX_TRANSCRIPT_CHARS} characters")));
        }
        if req.reference_bytes == 0 {
            return Err(ApiError::bad_request("reference_bytes must be positive"));
        }
        if req.reference_bytes > MAX_REFERENCE_BYTES {
            return Err(too_large(format!("recordings are limited to {MAX_REFERENCE_BYTES} bytes")));
        }
        let id = self.new_id()?;
        // Supabase first: if the user may not create it there, nothing is left behind in D1.
        self.accounts.insert_voice(user.token()?, &NewVoice { id: &id, name, transcript }).await?;
        let now = self.now_ms();
        let repo = self.repo();
        repo.insert_voice(&id, &user.id, name, transcript, req.reference_bytes, now).await?;
        repo.cache_owner(Owned::Voice, &id, &user.id, now + CACHE_MS).await?;
        let row =
            repo.get_voice(&id, &user.id).await?.ok_or_else(|| ApiError::internal("voice vanished after insert"))?;
        Ok(voice_info(&row))
    }

    /// The caller's voice: the D1 row must name them and the Supabase row must still exist (RLS: theirs).
    pub(crate) async fn owned_voice(&self, user: &AuthUser, id: &str) -> Result<VoiceRow, ApiError> {
        let row = self.repo().get_voice(id, &user.id).await?.ok_or_else(|| ApiError::not_found("no such voice"))?;
        self.check_voice_owner(user, id).await?;
        Ok(row)
    }

    pub async fn begin_voice_reference(
        &self,
        user: &AuthUser,
        id: &str,
        content_length: Option<u64>,
    ) -> Result<UploadTarget, ApiError> {
        let voice = self.owned_voice(user, id).await?;
        if voice.state == VoiceState::Preparing {
            return Err(conflict("this voice is being prepared"));
        }
        check_length(content_length, voice.reference_bytes)?;
        Ok(UploadTarget { object_key: raw_key(id), length: voice.reference_bytes })
    }

    pub async fn finish_voice_reference(&self, user: &AuthUser, id: &str) -> Result<(), ApiError> {
        self.owned_voice(user, id).await?;
        if self.repo().set_voice_raw_uploaded(id, self.now_ms()).await? {
            Ok(())
        } else {
            Err(conflict("the voice was deleted or is being prepared"))
        }
    }

    /// Queue preparation of the uploaded recording.
    pub async fn prepare_voice(&self, user: &AuthUser, id: &str) -> Result<VoiceInfo, ApiError> {
        let voice = self.owned_voice(user, id).await?;
        if !voice.raw_uploaded {
            return Err(conflict("upload the recording first"));
        }
        if voice.state == VoiceState::Preparing {
            return Ok(voice_info(&voice));
        }
        let request = PrepareReferenceRequest {
            v: CONTRACT_VERSION,
            raw_key: raw_key(id),
            ref_key: ref_key(id),
            transcript: voice.transcript.clone(),
        };
        let job = NewJob {
            id: self.new_id()?,
            kind: JobKind::Prepare,
            // CPU work: follows the Kokoro (CPU) retry policy of `nolabs_core::job`.
            engine: Engine::Kokoro,
            voice: user_voice_id(id),
            state: RenderState::Queued,
            request_json: to_json(&request)?,
            chars: 0,
            requested_s: 0,
            retry_at_ms: None,
            error: None,
            user_id: user.id.clone(),
            gpu_share_s: 0,
        };
        let now = self.now_ms();
        let repo = self.repo();
        repo.batch(&[Repo::set_voice_preparing_stmt(id, now), Repo::insert_job_stmt(&job, now)]).await?;
        self.queue.send(&JobMessage { job_id: job.id.clone() }, 0).await?;
        if self.accounts.set_voice_state(user.token()?, id, VoiceState::Preparing, None).await? {
            repo.set_voice_synced(id, VoiceState::Preparing).await?;
        }
        Ok(voice_info(&self.owned_voice(user, id).await?))
    }

    /// Delete the voice everywhere: the Supabase row, then (when the caller owned it) the pipeline row, its jobs,
    /// its rendered segments (audio and sidecars) and its recordings.
    pub async fn delete_voice(&self, user: &AuthUser, id: &str) -> Result<(), ApiError> {
        let removed_remote = self.accounts.delete_voice(user.token()?, id).await?;
        let owned_locally = self.repo().get_voice(id, &user.id).await?.is_some();
        if !removed_remote && !owned_locally {
            return Err(ApiError::not_found("no such voice"));
        }
        if owned_locally {
            self.purge_voice(id).await?;
        }
        self.repo().forget_owner(Owned::Voice, id).await?;
        Ok(())
    }

    /// Remove the pipeline half of a voice: D1 rows and every R2 object it produced.
    async fn purge_voice(&self, id: &str) -> Result<(), ApiError> {
        let voice_ref = user_voice_id(id);
        let repo = self.repo();
        let mut keys: Vec<String> = Vec::new();
        for object in repo.render_objects_by_voice(&voice_ref).await? {
            keys.push(format!("{object}.json"));
            keys.push(object);
        }
        repo.delete_voice_cascade(id, &voice_ref).await?;
        keys.extend(self.blob.list_prefix(&format!("voices/{id}/")).await?);
        keys.push(format!("previews/{id}.webm"));
        self.blob.delete_many(&keys).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestEnv, run};
    use nolabs_core::proto::ErrorCode;

    fn create(env: &TestEnv, user: &AuthUser) -> String {
        let v = run(env.app.create_voice(
            user,
            CreateVoice { name: " Me ".into(), transcript: "Line one.\nLine two.".into(), reference_bytes: 100 },
        ))
        .unwrap();
        v.id.strip_prefix("user:").unwrap().to_owned()
    }

    #[test]
    fn catalog_lists_presets_then_only_the_callers_voices() {
        let env = TestEnv::new();
        let (alice, bob) = (env.user("alice"), env.user("bob"));
        let id = create(&env, &alice);
        assert_eq!(env.accounts.voice(&id).unwrap().owner, "alice");
        let cat = run(env.app.voice_catalog(&alice)).unwrap();
        assert_eq!(cat.default_voice, "kokoro:af_heart");
        assert_eq!(cat.voices.len(), builtin_catalog().len() + 1);
        let last = cat.voices.last().unwrap();
        assert_eq!((last.name.as_str(), last.state, last.engine), ("Me", VoiceState::AwaitingReference, Engine::Ming));
        assert_eq!(run(env.app.voice_catalog(&bob)).unwrap().voices.len(), builtin_catalog().len());
    }

    #[test]
    fn clone_flow_upload_then_prepare_queues_a_cpu_job_for_the_owner() {
        let env = TestEnv::new();
        let alice = env.user("alice");
        let id = create(&env, &alice);
        run(async {
            assert_eq!(env.app.prepare_voice(&alice, &id).await.unwrap_err().code, ErrorCode::Conflict);
            assert!(env.app.begin_voice_reference(&alice, &id, Some(99)).await.is_err());
            let t = env.app.begin_voice_reference(&alice, &id, Some(100)).await.unwrap();
            assert_eq!(t.object_key, format!("voices/{id}/raw.wav"));
            env.app.finish_voice_reference(&alice, &id).await.unwrap();
            let info = env.app.prepare_voice(&alice, &id).await.unwrap();
            assert_eq!(info.state, VoiceState::Preparing);
            assert_eq!(env.accounts.voice(&id).unwrap().state, VoiceState::Preparing, "mirrored to Supabase");
            let sent = env.queue.take();
            assert_eq!(sent.len(), 1);
            let job = env.app.repo().get_job(&sent[0].0.job_id).await.unwrap().unwrap();
            assert_eq!((job.kind, job.engine, job.state), (JobKind::Prepare, Engine::Kokoro, RenderState::Queued));
            assert_eq!(job.user_id, "alice");
            let req: PrepareReferenceRequest = serde_json::from_str(&job.request_json).unwrap();
            assert_eq!(req.ref_key, format!("voices/{id}/ref.wav"));
            env.app.prepare_voice(&alice, &id).await.unwrap();
            assert!(env.queue.take().is_empty(), "preparing twice is idempotent");
            let e = env.app.begin_voice_reference(&alice, &id, Some(100)).await.unwrap_err();
            assert_eq!(e.code, ErrorCode::Conflict);
        });
    }

    #[test]
    fn another_users_voice_is_not_found_on_every_route() {
        let env = TestEnv::new();
        let (alice, bob) = (env.user("alice"), env.user("bob"));
        let id = create(&env, &alice);
        run(async {
            for e in [
                env.app.begin_voice_reference(&bob, &id, Some(100)).await.unwrap_err(),
                env.app.finish_voice_reference(&bob, &id).await.unwrap_err(),
                env.app.prepare_voice(&bob, &id).await.unwrap_err(),
                env.app.delete_voice(&bob, &id).await.unwrap_err(),
            ] {
                assert_eq!(e.code, ErrorCode::NotFound);
            }
        });
        assert!(env.accounts.voice(&id).is_some());
    }

    #[test]
    fn ready_state_reaches_supabase_on_the_next_listing() {
        let env = TestEnv::new();
        let alice = env.user("alice");
        let id = create(&env, &alice);
        let sha = "c".repeat(64);
        env.db.exec(&format!("UPDATE voices SET state = 'ready', ref_sha256 = '{sha}' WHERE id = '{id}'"));
        run(env.app.voice_catalog(&alice)).unwrap();
        let remote = env.accounts.voice(&id).unwrap();
        assert_eq!((remote.state, remote.ref_sha256.as_deref()), (VoiceState::Ready, Some(sha.as_str())));
        let calls = env.accounts.calls();
        run(env.app.voice_catalog(&alice)).unwrap();
        assert_eq!(env.accounts.calls(), calls, "nothing to mirror the second time");
    }

    #[test]
    fn a_voice_deleted_in_supabase_is_cleaned_up_here() {
        let env = TestEnv::new();
        let alice = env.user("alice");
        let id = create(&env, &alice);
        env.db.exec(&format!("UPDATE voices SET state = 'ready' WHERE id = '{id}'"));
        env.accounts.state.borrow_mut().voices.remove(&id);
        let cat = run(env.app.voice_catalog(&alice)).unwrap();
        assert_eq!(cat.voices.len(), builtin_catalog().len());
        assert_eq!(env.db.scalar("SELECT count(*) FROM voices"), serde_json::json!(0));
    }

    #[test]
    fn validation() {
        let env = TestEnv::new();
        let alice = env.user("alice");
        let bad = |name: &str, transcript: &str, bytes| CreateVoice {
            name: name.into(),
            transcript: transcript.into(),
            reference_bytes: bytes,
        };
        run(async {
            assert!(env.app.create_voice(&alice, bad("", "x", 1)).await.is_err());
            assert!(env.app.create_voice(&alice, bad("a", " ", 1)).await.is_err());
            assert!(env.app.create_voice(&alice, bad("a", "x", 0)).await.is_err());
            let e = env.app.create_voice(&alice, bad("a", "x", MAX_REFERENCE_BYTES + 1)).await.unwrap_err();
            assert_eq!(e.code, ErrorCode::PayloadTooLarge);
        });
        assert_eq!(env.accounts.calls(), 0, "nothing reaches Supabase before validation passes");
    }

    #[test]
    fn delete_removes_rows_recordings_and_rendered_audio() {
        let env = TestEnv::new();
        let alice = env.user("alice");
        let id = create(&env, &alice);
        env.db.exec(&format!(
            "INSERT INTO renders (key, engine, voice, state, object_key, chars, created_at_ms, updated_at_ms) VALUES \
             ('k1', 'ming', 'user:{id}', 'ready', 'audio/ming/k1.webm', 5, 0, 0), \
             ('k2', 'ming', 'ming:US_Emma', 'ready', 'audio/ming/k2.webm', 5, 0, 0); \
             INSERT INTO jobs (id, kind, engine, voice, state, request_json, created_at_ms, updated_at_ms) VALUES \
             ('j1', 'render', 'ming', 'user:{id}', 'queued', '{{}}', 0, 0)"
        ));
        for k in [
            "audio/ming/k1.webm",
            "audio/ming/k1.webm.json",
            "audio/ming/k2.webm",
            &raw_key(&id),
            &ref_key(&id),
            &format!("previews/{id}.webm"),
        ] {
            env.blob.insert(k, "x");
        }
        run(env.app.delete_voice(&alice, &id)).unwrap();
        assert_eq!(env.blob.keys(), vec!["audio/ming/k2.webm".to_owned()]);
        assert_eq!(env.db.scalar("SELECT count(*) FROM renders"), serde_json::json!(1));
        assert_eq!(env.db.scalar("SELECT count(*) FROM jobs"), serde_json::json!(0));
        assert!(env.accounts.voice(&id).is_none());
        assert_eq!(run(env.app.delete_voice(&alice, &id)).unwrap_err().code, ErrorCode::NotFound);
    }
}
