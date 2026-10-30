//! `POST /api/render` and `GET /api/render/status`. A render request never waits for audio: it looks up every item's
//! cache key, claims the misses in D1 (so concurrent requests never render a segment twice), groups the claimed items
//! into jobs, stores each job's full engine request and sends one queue message per job. The app then polls the
//! status route and fetches `/api/audio/{key}` once an item is ready.
//!
//! The cache is global (content-addressed, shared by every user), but GPU time is charged to the user whose request
//! claimed the miss: Ming jobs are admitted against the deployment's budget and the caller's daily share.

pub mod plan;

use std::collections::{HashMap, HashSet};

use nolabs_core::budget::{self, Admission, DAY_MS, GpuPolicy, GpuUse, Share};
use nolabs_core::cache_key::{self, sha256_hex};
use nolabs_core::proto::api::{Engine, RenderItemStatus, RenderRequest, RenderResponse, RenderState, VoiceState};
use nolabs_core::proto::engine::{CONTRACT_VERSION, EngineRequest, EngineVoice};
use nolabs_core::proto::{ApiError, ErrorCode};
use nolabs_core::voices::VoiceRef;

use crate::app::App;
use crate::auth::AuthUser;
use crate::config::audio_profile;
use crate::error::{conflict, to_json};
use crate::ports::{JobMessage, Stmt};
use crate::repo::{JobKind, NewClaim, NewJob, RenderRow, Repo};
use crate::services::gpu::GpuHistory;
use crate::services::gpu::max_chars_per_call;
use crate::services::voices::user_voice_id;
use plan::{PlannedItem, ResolvedVoice, kokoro_groups, ming_groups, plan_items};

pub const MAX_ITEMS: usize = 64;
pub const MAX_ITEM_CHARS: usize = 1000;
pub const MAX_STATUS_KEYS: usize = 100;

/// R2 key of the completion marker the Space writes for a job.
pub fn marker_key(job_id: &str) -> String {
    format!("jobs/{job_id}.json")
}

/// The error shown on items waiting for the GPU budget.
pub fn budget_error(retry_at_ms: i64, now_ms: i64) -> ApiError {
    ApiError::new(ErrorCode::GpuBudget, "the daily GPU budget is used up; this part renders when it frees up")
        .with_retry_after(crate::repo::seconds_until(retry_at_ms, now_ms))
}

/// A job about to be created.
struct PlannedJob {
    id: String,
    items: Vec<PlannedItem>,
    state: RenderState,
    retry_at_ms: Option<i64>,
    requested_s: u32,
}

/// Parse `keys=k1,k2,…` for the status route.
pub fn parse_status_keys(value: &str) -> Result<Vec<String>, ApiError> {
    let keys: Vec<String> = value.split(',').filter(|k| !k.is_empty()).map(str::to_owned).collect();
    if keys.is_empty() || keys.len() > MAX_STATUS_KEYS {
        return Err(ApiError::bad_request(format!("give 1 to {MAX_STATUS_KEYS} keys")));
    }
    if let Some(bad) = keys.iter().find(|k| !cache_key::is_key(k)) {
        return Err(ApiError::bad_request(format!("not a render key: {bad}")));
    }
    Ok(keys)
}

fn validate(req: &RenderRequest) -> Result<(), ApiError> {
    if req.items.is_empty() || req.items.len() > MAX_ITEMS {
        return Err(ApiError::bad_request(format!("give 1 to {MAX_ITEMS} items")));
    }
    for (i, item) in req.items.iter().enumerate() {
        let n = item.text.chars().count();
        if item.text.trim().is_empty() || n > MAX_ITEM_CHARS {
            return Err(ApiError::bad_request(format!("item {i}: text must have 1 to {MAX_ITEM_CHARS} characters")));
        }
    }
    Ok(())
}

impl App {
    /// Engine voice and fingerprint for a catalogue id. User voices must be the caller's and prepared.
    pub(crate) async fn resolve_voice(&self, caller: &AuthUser, id: &str) -> Result<ResolvedVoice, ApiError> {
        let voice = VoiceRef::parse(id).ok_or_else(|| ApiError::bad_request(format!("unknown voice {id}")))?;
        let engine = voice.engine();
        match voice {
            VoiceRef::Kokoro(preset) | VoiceRef::MingPreset(preset) => Ok(ResolvedVoice {
                catalog_id: id.to_owned(),
                engine,
                fingerprint: preset.clone(),
                voice: EngineVoice::Preset { id: preset },
                is_user: false,
            }),
            VoiceRef::User(user) => {
                let row = self.owned_voice(caller, &user).await?;
                let (Some(ref_key), Some(transcript), Some(sha)) = (row.ref_key, row.ref_transcript, row.ref_sha256)
                else {
                    return Err(conflict("this voice is not ready yet"));
                };
                if row.state != VoiceState::Ready {
                    return Err(conflict("this voice is not ready yet"));
                }
                Ok(ResolvedVoice {
                    catalog_id: user_voice_id(&user),
                    engine,
                    fingerprint: format!("{sha}:{}", sha256_hex(transcript.as_bytes())),
                    voice: EngineVoice::Reference { id: user, ref_key, transcript },
                    is_user: true,
                })
            }
        }
    }

    pub async fn render(&self, user: &AuthUser, req: RenderRequest) -> Result<RenderResponse, ApiError> {
        validate(&req)?;
        let voice = self.resolve_voice(user, &req.voice).await?;
        let planned = plan_items(&self.config, &voice, &req.items);
        let now = self.now_ms();
        let repo = self.repo();

        // Unique items in request order.
        let mut seen = HashSet::new();
        let unique: Vec<&PlannedItem> = planned.iter().filter(|p| seen.insert(p.key.clone())).collect();
        let keys: Vec<String> = unique.iter().map(|p| p.key.clone()).collect();
        let existing: HashMap<String, RenderRow> =
            repo.select_renders(&keys).await?.into_iter().map(|r| (r.key.clone(), r)).collect();

        let misses: Vec<&PlannedItem> = unique
            .iter()
            .copied()
            .filter(|p| match existing.get(&p.key) {
                None => true,
                Some(r) => r.state == RenderState::Failed && r.retry_at_ms.is_none_or(|at| at <= now),
            })
            .collect();
        let claims: Vec<NewClaim> = misses
            .iter()
            .map(|p| NewClaim {
                key: p.key.clone(),
                engine: voice.engine,
                voice: voice.catalog_id.clone(),
                object_key: p.item.object_key.clone(),
                chars: u32::try_from(p.chars).unwrap_or(u32::MAX),
            })
            .collect();
        let claimed: HashSet<String> = repo.claim_renders(&claims, now).await?.into_iter().collect();
        let to_render: Vec<PlannedItem> = misses.into_iter().filter(|p| claimed.contains(&p.key)).cloned().collect();

        let (jobs, share_s) = match voice.engine {
            Engine::Kokoro => (self.kokoro_jobs(to_render, &req)?, 0),
            Engine::Ming if to_render.is_empty() => (Vec::new(), 0),
            Engine::Ming => {
                let share_s = self.accounts.gpu_share(user.token()?, self.config.gpu.daily_cap_s).await?;
                (self.ming_jobs(to_render, now, &user.id, share_s).await?, share_s)
            }
        };
        let mut fresh: HashMap<String, RenderItemStatus> = HashMap::new();
        let mut stmts: Vec<Stmt> = Vec::new();
        for job in &jobs {
            let error = job.retry_at_ms.filter(|_| job.state == RenderState::Deferred).map(|at| budget_error(at, now));
            stmts.push(Repo::insert_job_stmt(&self.new_job_row(&voice, job, &user.id, share_s)?, now));
            for item in &job.items {
                stmts.push(Repo::assign_render_stmt(
                    &item.key,
                    &job.id,
                    job.state,
                    job.retry_at_ms,
                    error.as_ref(),
                    now,
                ));
                let status =
                    RenderItemStatus { key: item.key.clone(), state: job.state, audio: None, error: error.clone() };
                fresh.insert(item.key.clone(), status);
            }
        }
        repo.batch(&stmts).await?;
        for job in jobs.iter().filter(|j| j.state == RenderState::Queued) {
            self.queue.send(&JobMessage { job_id: job.id.clone() }, 0).await?;
        }

        let items = planned
            .iter()
            .map(|p| match (fresh.get(&p.key), existing.get(&p.key)) {
                (Some(s), _) => s.clone(),
                (None, Some(row)) => row.status(now),
                // Claimed by a concurrent request between our lookup and our claim.
                (None, None) => {
                    RenderItemStatus { key: p.key.clone(), state: RenderState::Queued, audio: None, error: None }
                }
            })
            .collect();
        Ok(RenderResponse { items })
    }

    fn kokoro_jobs(&self, items: Vec<PlannedItem>, req: &RenderRequest) -> Result<Vec<PlannedJob>, ApiError> {
        kokoro_groups(items.len(), req.priority)
            .into_iter()
            .map(|range| {
                Ok(PlannedJob {
                    id: self.new_id()?,
                    items: items[range].to_vec(),
                    state: RenderState::Queued,
                    retry_at_ms: None,
                    requested_s: 0,
                })
            })
            .collect()
    }

    /// Admit Ming jobs in order against the GPU budget and the user's share; once one does not fit, it and every
    /// later job wait.
    async fn ming_jobs(
        &self,
        items: Vec<PlannedItem>,
        now: i64,
        user_id: &str,
        share_s: u32,
    ) -> Result<Vec<PlannedJob>, ApiError> {
        let policy = self.config.gpu;
        let mut history: GpuHistory = self.gpu_history(now, user_id, None).await?;
        let chars: Vec<usize> = items.iter().map(|p| p.chars).collect();
        let mut jobs = Vec::new();
        let mut blocked_until: Option<i64> = None;
        for range in ming_groups(&chars, max_chars_per_call(&policy)) {
            let group = &items[range.clone()];
            let group_chars = &chars[range];
            let deferred = |items: &[PlannedItem], until: i64| -> Result<PlannedJob, ApiError> {
                let total: usize = items.iter().map(|p| p.chars).sum();
                Ok(PlannedJob {
                    id: self.new_id()?,
                    items: items.to_vec(),
                    state: RenderState::Deferred,
                    retry_at_ms: Some(until),
                    requested_s: budget::estimate_seconds(&policy, total),
                })
            };
            if let Some(until) = blocked_until {
                jobs.push(deferred(group, until)?);
                continue;
            }
            let queued = |items: &[PlannedItem], request_s: u32| -> Result<PlannedJob, ApiError> {
                Ok(PlannedJob {
                    id: self.new_id()?,
                    items: items.to_vec(),
                    state: RenderState::Queued,
                    retry_at_ms: None,
                    requested_s: request_s,
                })
            };
            let share = Share { daily_s: share_s, history: &history.mine };
            match budget::admit_for_user(&policy, &history.all, share, now, group_chars) {
                Admission::Admit { request_s } => {
                    history.push(GpuUse { at_ms: now, requested_s: request_s, used_s: 0 });
                    jobs.push(queued(group, request_s)?);
                }
                Admission::Shrink { items: n, request_s } => {
                    let n = n.clamp(1, group.len());
                    history.push(GpuUse { at_ms: now, requested_s: request_s, used_s: 0 });
                    jobs.push(queued(&group[..n], request_s)?);
                    let mine = budget::view(&GpuPolicy { daily_cap_s: share_s, ..policy }, &history.mine, now);
                    let all = budget::view(&policy, &history.all, now);
                    let until = mine.resets_at_ms.into_iter().chain(all.resets_at_ms).min().unwrap_or(now + DAY_MS);
                    if n < group.len() {
                        jobs.push(deferred(&group[n..], until)?);
                    }
                    blocked_until = Some(until);
                }
                Admission::Defer { retry_at_ms } => {
                    jobs.push(deferred(group, retry_at_ms)?);
                    blocked_until = Some(retry_at_ms);
                }
            }
        }
        Ok(jobs)
    }

    fn new_job_row(
        &self,
        voice: &ResolvedVoice,
        job: &PlannedJob,
        user_id: &str,
        gpu_share_s: u32,
    ) -> Result<NewJob, ApiError> {
        let request = EngineRequest {
            v: CONTRACT_VERSION,
            job_id: job.id.clone(),
            engine: voice.engine,
            voice: voice.voice.clone(),
            items: job.items.iter().map(|p| p.item.clone()).collect(),
            audio: audio_profile(voice.engine),
            marker_key: marker_key(&job.id),
        };
        let chars: usize = job.items.iter().map(|p| p.chars).sum();
        Ok(NewJob {
            id: job.id.clone(),
            kind: JobKind::Render,
            engine: voice.engine,
            voice: voice.catalog_id.clone(),
            state: job.state,
            request_json: to_json(&request)?,
            chars: u32::try_from(chars).unwrap_or(u32::MAX),
            requested_s: job.requested_s,
            retry_at_ms: job.retry_at_ms,
            error: None,
            user_id: user_id.to_owned(),
            gpu_share_s,
        })
    }

    /// Current status of each key, in the order given; unknown keys come back failed with `not_found`.
    pub async fn render_status(&self, keys: &[String]) -> Result<RenderResponse, ApiError> {
        let now = self.now_ms();
        let rows: HashMap<String, RenderRow> =
            self.repo().select_renders(keys).await?.into_iter().map(|r| (r.key.clone(), r)).collect();
        let items = keys
            .iter()
            .map(|k| match rows.get(k) {
                Some(row) => row.status(now),
                None => RenderItemStatus {
                    key: k.clone(),
                    state: RenderState::Failed,
                    audio: None,
                    error: Some(ApiError::not_found("never requested; send it to /api/render")),
                },
            })
            .collect();
        Ok(RenderResponse { items })
    }
}

#[cfg(test)]
mod tests;
