//! The `nolabs-render` queue consumer: one message `{job_id}` per job. The queue never retries on its own
//! (`max_retries = 0`); this module decides retries through [`nolabs_core::job::apply`] and re-sends with a delay.
//!
//! GPU (Ming) jobs are guarded three ways because each call may spend one of the day's few ZeroGPU runs: the Space's
//! completion marker `jobs/<id>.json` is checked before any call (a finished job is applied, never resubmitted), the
//! budget is checked again right before the call, and a ledger row is written before the call starts.

use std::collections::HashMap;

use nolabs_core::engine_error::EngineError;
use nolabs_core::job::{self, Event};
use nolabs_core::proto::api::{Engine, RenderState};
use nolabs_core::proto::engine::{EngineItemResult, EngineRequest, EngineResponse, PrepareReferenceRequest};
use nolabs_core::proto::{ApiError, ErrorCode};

use crate::app::App;
use crate::ports::{JobMessage, Stmt};
use crate::repo::{JobKind, JobRow, JobUpdate, Repo, seconds_until};
use crate::services::gpu::admit_whole;
use crate::services::render::{budget_error, marker_key};

/// What happened to a job; the Worker logs it and acknowledges the message either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Every item rendered (or the reference was prepared).
    Done,
    /// Rendered, but these items failed.
    PartlyFailed {
        failed: usize,
    },
    /// Already finished per the Space's marker; applied without calling the Space.
    AppliedMarker,
    /// Sent back to the queue with this delay.
    Retrying {
        delay_s: u32,
    },
    /// Waiting for the GPU budget until this time.
    Deferred {
        retry_at_ms: i64,
    },
    Failed {
        error: String,
    },
    /// Nothing to do (unknown job, duplicate message, already started elsewhere).
    Skipped {
        reason: &'static str,
    },
}

/// How long a failed segment keeps its failure before a new render request may claim it again.
pub fn failed_cooldown_ms(engine: Engine) -> i64 {
    match engine {
        Engine::Kokoro => 60_000,
        // Each retry may cost a GPU run.
        Engine::Ming => 60 * 60 * 1000,
    }
}

/// Whether a failed call may have used GPU time. Every other failure happens before ZeroGPU allocates a GPU (the
/// proxy, the queue, contract validation or ZeroGPU's own admission refused it), so it costs no run.
pub fn may_have_used_gpu(error: &EngineError) -> bool {
    matches!(error, EngineError::Internal(_))
}

/// Cooldown for segments of a failed job: the engine's default, but short when the GPU never ran.
fn cooldown_after(engine: Engine, error: &EngineError) -> i64 {
    if engine == Engine::Ming && !may_have_used_gpu(error) {
        failed_cooldown_ms(Engine::Kokoro)
    } else {
        failed_cooldown_ms(engine)
    }
}

fn machine_error(e: String) -> ApiError {
    ApiError::internal(format!("job state machine: {e}"))
}

fn delay_s(retry_at_ms: Option<i64>, now_ms: i64) -> u32 {
    retry_at_ms.map_or(0, |at| u32::try_from(seconds_until(at, now_ms)).unwrap_or(u32::MAX))
}

impl App {
    pub async fn handle_job(&self, message: &JobMessage) -> Result<Outcome, ApiError> {
        let Some(job) = self.repo().get_job(&message.job_id).await? else {
            return Ok(Outcome::Skipped { reason: "unknown job" });
        };
        if job.state != RenderState::Queued {
            return Ok(Outcome::Skipped { reason: "job is not queued" });
        }
        match job.kind {
            JobKind::Render => self.run_render(job).await,
            JobKind::Prepare => self.run_prepare(job).await,
        }
    }

    /// The Space's completion marker for a job, if it wrote one.
    pub(crate) async fn read_marker(&self, job_id: &str) -> Result<Option<EngineResponse>, ApiError> {
        let Some(bytes) = self.blob.get(&marker_key(job_id)).await? else {
            return Ok(None);
        };
        Ok(serde_json::from_slice::<EngineResponse>(&bytes).ok().filter(|r| r.job_id == job_id))
    }

    /// Move a queued job to Rendering (compare-and-set) and mirror that on its items. False when another consumer
    /// already took it.
    async fn start(&self, job: &JobRow, requested_s: u32, now: i64) -> Result<Option<JobRow>, ApiError> {
        let next = job::apply(&job.machine(), &Event::Start { now_ms: now }).map_err(machine_error)?;
        let update = JobUpdate { id: &job.id, from: RenderState::Queued, to: &next, requested_s, error: None };
        let repo = self.repo();
        if !repo.transition_job(&update, now).await? {
            return Ok(None);
        }
        let mut stmts = vec![Repo::renders_for_job_stmt(&job.id, RenderState::Rendering, None, None, now)];
        if job.engine == Engine::Ming && job.kind == JobKind::Render {
            stmts.push(Repo::ledger_insert_stmt(&job.id, next.attempts, now, requested_s));
        }
        repo.batch(&stmts).await?;
        Ok(Some(JobRow {
            state: next.state,
            attempts: next.attempts,
            lease_until_ms: next.lease_until_ms,
            retry_at_ms: next.retry_at_ms,
            requested_s,
            ..job.clone()
        }))
    }

    async fn run_render(&self, job: JobRow) -> Result<Outcome, ApiError> {
        let now = self.now_ms();
        let request: EngineRequest = serde_json::from_str(&job.request_json)
            .map_err(|e| ApiError::internal(format!("stored engine request: {e}")))?;
        let mut requested_s = job.requested_s;
        if job.engine == Engine::Ming {
            if let Some(response) = self.read_marker(&job.id).await? {
                self.apply_response(&job, &request, &response).await?;
                return Ok(Outcome::AppliedMarker);
            }
            let chars: Vec<usize> = request.items.iter().map(|i| i.text.chars().count()).collect();
            let history = self.gpu_history(now, Some(&job.id)).await?;
            match admit_whole(&self.config.gpu, &history, now, &chars) {
                Ok(request_s) => requested_s = request_s,
                Err(retry_at_ms) => {
                    self.defer(&job, retry_at_ms, now).await?;
                    return Ok(Outcome::Deferred { retry_at_ms });
                }
            }
        }
        let Some(started) = self.start(&job, requested_s, now).await? else {
            return Ok(Outcome::Skipped { reason: "job was started by another consumer" });
        };
        match self.space().render(job.engine, &job.request_json).await {
            Ok(response) => self.apply_response(&started, &request, &response).await,
            Err(error) => self.fail(&started, error).await,
        }
    }

    /// A queued job that no longer fits the budget waits (there is no state-machine event for this: the job never
    /// started, so it simply changes from Queued to Deferred).
    async fn defer(&self, job: &JobRow, retry_at_ms: i64, now: i64) -> Result<(), ApiError> {
        let to = job::Row { state: RenderState::Deferred, retry_at_ms: Some(retry_at_ms), ..job.machine() };
        let error = budget_error(retry_at_ms, now);
        let update = JobUpdate {
            id: &job.id,
            from: RenderState::Queued,
            to: &to,
            requested_s: job.requested_s,
            error: Some(&error),
        };
        self.repo()
            .batch(&[
                Repo::transition_job_stmt(&update, now),
                Repo::renders_for_job_stmt(&job.id, RenderState::Deferred, Some(retry_at_ms), Some(&error), now),
            ])
            .await?;
        Ok(())
    }

    /// Record an engine response: ready rows for successful items, failed rows for the rest, the job done and the GPU
    /// time used. Works from any job state, so a late marker can complete a job that was given up on.
    pub(crate) async fn apply_response(
        &self,
        job: &JobRow,
        request: &EngineRequest,
        response: &EngineResponse,
    ) -> Result<Outcome, ApiError> {
        let now = self.now_ms();
        let results: HashMap<&str, &EngineItemResult> = response.items.iter().map(|r| (r.key.as_str(), r)).collect();
        let retry_at = Some(now + failed_cooldown_ms(job.engine));
        let mut stmts: Vec<Stmt> = Vec::new();
        let mut failed = 0;
        for item in &request.items {
            match results.get(item.key.as_str()) {
                Some(r) if r.ok => {
                    let mut r = (*r).clone();
                    if r.object_key.is_empty() {
                        r.object_key.clone_from(&item.object_key);
                    }
                    stmts.push(Repo::render_ready_stmt(&job.id, &r, now));
                }
                other => {
                    failed += 1;
                    let message = other.and_then(|r| r.error.as_ref()).map_or_else(
                        || "missing from the engine response".to_owned(),
                        |e| format!("{}: {}", e.code, e.message),
                    );
                    let error = ApiError::new(ErrorCode::EngineFailed, message);
                    stmts.push(Repo::render_failed_stmt(&job.id, &item.key, &error, retry_at, now));
                }
            }
        }
        let done = match job::apply(&job.machine(), &Event::Done) {
            Ok(row) => row,
            // Completing from a state other than Rendering (a marker found later): finish it directly.
            Err(_) => job::Row { state: RenderState::Ready, lease_until_ms: None, retry_at_ms: None, ..job.machine() },
        };
        let update = JobUpdate { id: &job.id, from: job.state, to: &done, requested_s: job.requested_s, error: None };
        stmts.push(Repo::transition_job_stmt(&update, now));
        if job.engine == Engine::Ming {
            let used_s = u32::try_from(response.gpu_ms.div_ceil(1000)).unwrap_or(u32::MAX);
            stmts.push(Repo::ledger_used_stmt(&job.id, job.attempts, used_s));
        }
        self.repo().batch(&stmts).await?;
        Ok(if failed == 0 { Outcome::Done } else { Outcome::PartlyFailed { failed } })
    }

    /// Apply an engine failure to a started job: deferred (budget), re-queued with a delay (transient CPU errors) or
    /// failed.
    async fn fail(&self, job: &JobRow, error: EngineError) -> Result<Outcome, ApiError> {
        let now = self.now_ms();
        let next =
            job::apply(&job.machine(), &Event::Fail { error: error.clone(), now_ms: now }).map_err(machine_error)?;
        let api_error = match next.state {
            RenderState::Deferred => budget_error(next.retry_at_ms.unwrap_or(now), now),
            _ => error.to_api(),
        };
        let update = JobUpdate {
            id: &job.id,
            from: RenderState::Rendering,
            to: &next,
            requested_s: job.requested_s,
            error: Some(&api_error),
        };
        let (item_state, item_retry, item_error) = match next.state {
            RenderState::Queued => (RenderState::Queued, None, None),
            RenderState::Deferred => (RenderState::Deferred, next.retry_at_ms, Some(&api_error)),
            _ => (RenderState::Failed, Some(now + cooldown_after(job.engine, &error)), Some(&api_error)),
        };
        let mut stmts = vec![Repo::transition_job_stmt(&update, now)];
        if job.engine == Engine::Ming && job.kind == JobKind::Render && !may_have_used_gpu(&error) {
            // The call was refused before reaching a GPU: it spent neither seconds nor a run.
            stmts.push(Repo::ledger_delete_stmt(&job.id, job.attempts));
        }
        match job.kind {
            JobKind::Render => {
                stmts.push(Repo::renders_for_job_stmt(&job.id, item_state, item_retry, item_error, now));
            }
            JobKind::Prepare if next.state == RenderState::Failed => {
                let voice = job.voice.strip_prefix("user:").unwrap_or(&job.voice).to_owned();
                self.repo().set_voice_failed(&voice, &error.to_string(), now).await?;
            }
            JobKind::Prepare => {}
        }
        self.repo().batch(&stmts).await?;
        match next.state {
            RenderState::Queued => {
                let delay = delay_s(next.retry_at_ms, now);
                self.queue.send(&JobMessage { job_id: job.id.clone() }, delay).await?;
                Ok(Outcome::Retrying { delay_s: delay })
            }
            RenderState::Deferred => Ok(Outcome::Deferred { retry_at_ms: next.retry_at_ms.unwrap_or(now) }),
            _ => Ok(Outcome::Failed { error: error.to_string() }),
        }
    }

    async fn run_prepare(&self, job: JobRow) -> Result<Outcome, ApiError> {
        let now = self.now_ms();
        let request: PrepareReferenceRequest = serde_json::from_str(&job.request_json)
            .map_err(|e| ApiError::internal(format!("stored prepare request: {e}")))?;
        let Some(started) = self.start(&job, 0, now).await? else {
            return Ok(Outcome::Skipped { reason: "job was started by another consumer" });
        };
        match self.space().prepare_reference(&request).await {
            Ok(prepared) => {
                let voice = job.voice.strip_prefix("user:").unwrap_or(&job.voice);
                let repo = self.repo();
                repo.set_voice_ready(voice, &prepared, self.now_ms()).await?;
                let done = job::apply(&started.machine(), &Event::Done).map_err(machine_error)?;
                let update =
                    JobUpdate { id: &job.id, from: RenderState::Rendering, to: &done, requested_s: 0, error: None };
                repo.transition_job(&update, self.now_ms()).await?;
                Ok(Outcome::Done)
            }
            Err(error) => self.fail(&started, error).await,
        }
    }
}

#[cfg(test)]
mod tests;
