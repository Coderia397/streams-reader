//! The GPU budget as seen by this deployment: the ledger of started calls plus Ming jobs admitted but not started
//! yet (counted as if they started now), split by user so each user's calls fit their daily share as well as the
//! deployment's cap ([`nolabs_core::budget::admit_for_user`]).

use nolabs_core::budget::{self, Admission, DAY_MS, GpuUse, Share};
use nolabs_core::proto::ApiError;

use crate::app::App;

/// Everyone's GPU uses in the window, and the subset charged to one user.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GpuHistory {
    pub all: Vec<GpuUse>,
    pub mine: Vec<GpuUse>,
}

impl GpuHistory {
    /// Count an admitted call for the rest of a planning pass.
    pub fn push(&mut self, use_: GpuUse) {
        self.all.push(use_);
        self.mine.push(use_);
    }
}

impl App {
    /// GPU uses in the last 24 h plus every queued Ming job except `exclude_job`, with `user_id`'s own part.
    pub(crate) async fn gpu_history(
        &self,
        now_ms: i64,
        user_id: &str,
        exclude_job: Option<&str>,
    ) -> Result<GpuHistory, ApiError> {
        let repo = self.repo();
        let mut history = GpuHistory::default();
        for entry in repo.gpu_uses_since(now_ms - DAY_MS).await? {
            if entry.user_id == user_id {
                history.mine.push(entry.gpu);
            }
            history.all.push(entry.gpu);
        }
        for (id, requested_s, owner) in repo.pending_gpu_requests().await? {
            if Some(id.as_str()) != exclude_job {
                let use_ = GpuUse { at_ms: now_ms, requested_s, used_s: 0 };
                if owner == user_id {
                    history.mine.push(use_);
                }
                history.all.push(use_);
            }
        }
        Ok(history)
    }
}

/// Admit a whole job or nothing for its user: `Ok(request_s)`, or `Err(retry_at_ms)`. Jobs are sized at planning
/// time to fit one call, so a job that only partly fits waits for the window to reset instead of being split again.
pub fn admit_whole(
    policy: &budget::GpuPolicy,
    history: &GpuHistory,
    share_s: u32,
    now_ms: i64,
    item_chars: &[usize],
) -> Result<u32, i64> {
    let share = Share { daily_s: share_s, history: &history.mine };
    match budget::admit_for_user(policy, &history.all, share, now_ms, item_chars) {
        Admission::Admit { request_s } => Ok(request_s),
        Admission::Shrink { .. } => {
            let mine = budget::view(&budget::GpuPolicy { daily_cap_s: share_s, ..*policy }, &history.mine, now_ms);
            let all = budget::view(policy, &history.all, now_ms);
            let reset = mine.resets_at_ms.into_iter().chain(all.resets_at_ms).min().unwrap_or(now_ms + DAY_MS);
            Err(reset.max(now_ms + 60_000))
        }
        Admission::Defer { retry_at_ms } => Err(retry_at_ms),
    }
}

/// The share a job is admitted against: the one recorded when it was planned. Jobs from before accounts carry no
/// user and run against the whole cap, as they did then.
pub fn job_share(policy: &budget::GpuPolicy, job: &crate::repo::JobRow) -> u32 {
    if job.user_id.is_empty() { policy.daily_cap_s } else { job.gpu_share_s }
}

/// Most input chars one Ming call may carry so its estimate stays within `max_call_s`.
pub fn max_chars_per_call(policy: &budget::GpuPolicy) -> usize {
    let spare = policy.max_call_s.saturating_sub(policy.base_s) as f32;
    if policy.s_per_char <= 0.0 { usize::MAX } else { ((spare / policy.s_per_char).floor() as usize).max(1) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestEnv, run};
    use nolabs_core::budget::GpuPolicy;

    #[test]
    fn call_size_follows_the_policy() {
        // (180 - 8) / 0.06 = 2866.67
        assert_eq!(max_chars_per_call(&GpuPolicy::default()), 2866);
        let p = GpuPolicy { s_per_char: 0.0, ..GpuPolicy::default() };
        assert_eq!(max_chars_per_call(&p), usize::MAX);
    }

    #[test]
    fn history_splits_ledger_and_pending_jobs_by_user() {
        let env = TestEnv::new();
        let now = env.now();
        env.db.exec(&format!(
            "INSERT INTO gpu_ledger (job_id, attempt, at_ms, requested_s, used_s, user_id) VALUES \
             ('j1', 1, {now}, 20, 25, 'alice'), ('j2', 1, {now}, 30, 31, 'bob'); \
             INSERT INTO jobs (id, kind, engine, voice, state, request_json, requested_s, user_id, created_at_ms, \
             updated_at_ms) VALUES ('j3', 'render', 'ming', 'ming:US_Emma', 'queued', '{{}}', 40, 'alice', 0, 0)"
        ));
        let h = run(env.app.gpu_history(now, "alice", None)).unwrap();
        assert_eq!((h.all.len(), h.mine.len()), (3, 2));
        let h = run(env.app.gpu_history(now, "alice", Some("j3"))).unwrap();
        assert_eq!((h.all.len(), h.mine.len()), (2, 1));
    }

    #[test]
    fn a_whole_job_waits_for_the_users_own_reset() {
        let p = GpuPolicy::default();
        let now = 1_800_000_000_000;
        let spent = GpuUse { at_ms: now - 1000, requested_s: 38, used_s: 40 };
        let history = GpuHistory { all: vec![spent], mine: vec![spent] };
        assert_eq!(admit_whole(&p, &history, 60, now, &[100]), Err(now - 1000 + DAY_MS));
        assert_eq!(admit_whole(&p, &history, 280, now, &[100]), Ok(14));
    }
}
