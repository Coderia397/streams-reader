//! The 30-minute cron: keep the Space awake (restart it when Hugging Face put it to sleep, paused it or it crashed),
//! recover jobs whose consumer died, release deferred GPU jobs when the budget allows, re-send queue messages that
//! may have been lost, and prune old rows. Each step runs even when an earlier one failed.

use nolabs_core::engine_error::EngineError;
use nolabs_core::job::{self, Event};
use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::{Engine, RenderState, SpaceStatus};
use nolabs_core::proto::engine::EngineRequest;

use crate::app::App;
use crate::error::to_json;
use crate::ports::JobMessage;
use crate::repo::{CONFIG_SPACE_STATUS, JobKind, JobRow, JobUpdate, Repo};
use crate::services::consumer::failed_cooldown_ms;
use crate::services::gpu::admit_whole;
use crate::services::render::budget_error;
use crate::space::is_healthy;
use nolabs_core::budget::GpuUse;

const MINUTE_MS: i64 = 60_000;
const DAY_MS: i64 = 24 * 60 * MINUTE_MS;
/// Stages in which the Space will not wake on its own.
pub const RESTART_STAGES: &[&str] = &["SLEEPING", "PAUSED", "RUNTIME_ERROR"];
/// A queued job untouched this long may have lost its message.
pub const STALE_QUEUED_MS: i64 = 30 * MINUTE_MS;
/// Rows claimed but never attached to a job after this long are released.
pub const ORPHAN_MS: i64 = 10 * MINUTE_MS;
/// How far back to look for Ming jobs whose marker appeared after they were given up on.
pub const LATE_MARKER_WINDOW_MS: i64 = 2 * 60 * MINUTE_MS;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CronReport {
    pub stage: Option<String>,
    pub restarted: bool,
    pub healthy: bool,
    /// Expired leases taken back (re-queued or failed).
    pub recovered: usize,
    /// GPU jobs completed from a marker the Space wrote after the consumer gave up.
    pub markers_applied: usize,
    pub released: usize,
    pub still_deferred: usize,
    pub resent: usize,
    pub orphans: u64,
    pub errors: Vec<String>,
}

impl CronReport {
    fn note<T>(&mut self, step: &str, result: Result<T, ApiError>) -> Option<T> {
        result.map_err(|e| self.errors.push(format!("{step}: {}", e.message))).ok()
    }
}

impl App {
    pub async fn run_cron(&self) -> CronReport {
        let mut report = CronReport::default();
        self.check_space(&mut report).await;
        let r = self.recover_leases(&mut report).await;
        report.note("leases", r);
        let r = self.apply_late_markers(&mut report).await;
        report.note("markers", r);
        let r = self.release_deferred(&mut report).await;
        report.note("deferred", r);
        let r = self.resend_stale(&mut report).await;
        report.note("resend", r);
        let now = self.now_ms();
        let r = self.repo().release_orphan_renders(now, now - ORPHAN_MS).await.map_err(ApiError::from);
        report.orphans = report.note("orphans", r).unwrap_or(0);
        let r = self.prune(now).await;
        report.note("prune", r);
        report
    }

    async fn check_space(&self, report: &mut CronReport) {
        let space = self.space();
        let repo_name = &self.config.space_repo;
        let stage = match space.runtime_stage(repo_name).await {
            Ok(stage) => stage,
            Err(e) => {
                report.errors.push(format!("runtime: {e}"));
                "UNKNOWN".to_owned()
            }
        };
        if RESTART_STAGES.contains(&stage.as_str()) {
            match space.restart(repo_name).await {
                Ok(()) => report.restarted = true,
                Err(e) => report.errors.push(format!("restart: {e}")),
            }
        }
        // Skip the health call right after a restart: the Space is rebuilding and the call would only queue.
        if !report.restarted {
            match space.health().await {
                Ok(h) => report.healthy = is_healthy(&h),
                Err(e) => report.errors.push(format!("health: {e}")),
            }
        }
        let status = SpaceStatus { stage: stage.clone(), healthy: report.healthy, checked_at_ms: self.now_ms() };
        let stored = match to_json(&status) {
            Ok(json) => self.repo().config_put(CONFIG_SPACE_STATUS, &json, self.now_ms()).await.map_err(ApiError::from),
            Err(e) => Err(e),
        };
        report.note("store status", stored);
        report.stage = Some(stage);
    }

    /// Complete a Ming job from its marker, if the Space wrote one.
    async fn try_marker(&self, job: &JobRow) -> Result<bool, ApiError> {
        if job.engine != Engine::Ming || job.kind != JobKind::Render {
            return Ok(false);
        }
        let Some(response) = self.read_marker(&job.id).await? else {
            return Ok(false);
        };
        let request: EngineRequest = serde_json::from_str(&job.request_json)
            .map_err(|e| ApiError::internal(format!("stored engine request: {e}")))?;
        self.apply_response(job, &request, &response).await?;
        Ok(true)
    }

    async fn recover_leases(&self, report: &mut CronReport) -> Result<(), ApiError> {
        let now = self.now_ms();
        for job in self.repo().expired_lease_jobs(now).await? {
            if self.try_marker(&job).await? {
                report.markers_applied += 1;
                continue;
            }
            let next = job::apply(&job.machine(), &Event::LeaseExpired)
                .map_err(|e| ApiError::internal(format!("job state machine: {e}")))?;
            let error = (next.state == RenderState::Failed)
                .then(|| EngineError::Internal("the render was interrupted".into()).to_api());
            let update = JobUpdate {
                id: &job.id,
                from: RenderState::Rendering,
                to: &next,
                requested_s: job.requested_s,
                error: error.as_ref(),
            };
            let mut stmts = vec![Repo::transition_job_stmt(&update, now)];
            if job.kind == JobKind::Render {
                let retry = (next.state == RenderState::Failed).then(|| now + failed_cooldown_ms(job.engine));
                stmts.push(Repo::renders_for_job_stmt(&job.id, next.state, retry, error.as_ref(), now));
            }
            let applied = self.repo().batch(&stmts).await?;
            if applied.first().is_some_and(|r| r.changes > 0) {
                report.recovered += 1;
                if next.state == RenderState::Queued {
                    self.queue.send(&JobMessage { job_id: job.id.clone() }, 0).await?;
                } else if job.kind == JobKind::Prepare {
                    let voice = job.voice.strip_prefix("user:").unwrap_or(&job.voice);
                    self.repo().set_voice_failed(voice, "preparation was interrupted", now).await?;
                }
            }
        }
        Ok(())
    }

    async fn apply_late_markers(&self, report: &mut CronReport) -> Result<(), ApiError> {
        let since = self.now_ms() - LATE_MARKER_WINDOW_MS;
        for job in self.repo().recent_failed_gpu_jobs(since).await? {
            if self.try_marker(&job).await? {
                report.markers_applied += 1;
            }
        }
        Ok(())
    }

    /// Release deferred jobs whose time has come, re-admitting GPU jobs through the budget in creation order.
    async fn release_deferred(&self, report: &mut CronReport) -> Result<(), ApiError> {
        let now = self.now_ms();
        let repo = self.repo();
        let due = repo.due_deferred_jobs(now).await?;
        if due.is_empty() {
            return Ok(());
        }
        let mut history: Vec<GpuUse> = self.gpu_history(now, None).await?;
        for job in due {
            let mut requested_s = job.requested_s;
            if job.engine == Engine::Ming && job.kind == JobKind::Render {
                let request: EngineRequest = serde_json::from_str(&job.request_json)
                    .map_err(|e| ApiError::internal(format!("stored engine request: {e}")))?;
                let chars: Vec<usize> = request.items.iter().map(|i| i.text.chars().count()).collect();
                match admit_whole(&self.config.gpu, &history, now, &chars) {
                    Ok(request_s) => {
                        requested_s = request_s;
                        history.push(GpuUse { at_ms: now, requested_s: request_s, used_s: 0 });
                    }
                    Err(retry_at) => {
                        let to = job::Row { retry_at_ms: Some(retry_at), ..job.machine() };
                        let error = budget_error(retry_at, now);
                        let update = JobUpdate {
                            id: &job.id,
                            from: RenderState::Deferred,
                            to: &to,
                            requested_s,
                            error: Some(&error),
                        };
                        repo.batch(&[
                            Repo::transition_job_stmt(&update, now),
                            Repo::renders_for_job_stmt(
                                &job.id,
                                RenderState::Deferred,
                                Some(retry_at),
                                Some(&error),
                                now,
                            ),
                        ])
                        .await?;
                        report.still_deferred += 1;
                        continue;
                    }
                }
            }
            let next = job::apply(&job.machine(), &Event::Due)
                .map_err(|e| ApiError::internal(format!("job state machine: {e}")))?;
            let update = JobUpdate { id: &job.id, from: RenderState::Deferred, to: &next, requested_s, error: None };
            let applied = repo
                .batch(&[
                    Repo::transition_job_stmt(&update, now),
                    Repo::renders_for_job_stmt(&job.id, next.state, None, None, now),
                ])
                .await?;
            if applied.first().is_some_and(|r| r.changes > 0) {
                self.queue.send(&JobMessage { job_id: job.id.clone() }, 0).await?;
                report.released += 1;
            }
        }
        Ok(())
    }

    async fn resend_stale(&self, report: &mut CronReport) -> Result<(), ApiError> {
        let now = self.now_ms();
        let repo = self.repo();
        for job in repo.stale_queued_jobs(now - STALE_QUEUED_MS, now).await? {
            repo.touch_job(&job.id, now).await?;
            self.queue.send(&JobMessage { job_id: job.id.clone() }, 0).await?;
            report.resent += 1;
        }
        Ok(())
    }

    async fn prune(&self, now: i64) -> Result<(), ApiError> {
        let repo = self.repo();
        repo.prune_auth_failures(now - DAY_MS).await?;
        repo.prune_ledger(now - 2 * DAY_MS).await?;
        repo.prune_finished_jobs(now - 7 * DAY_MS).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
