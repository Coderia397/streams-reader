//! The GPU budget as seen by this deployment: the ledger of started calls plus Ming jobs admitted but not started
//! yet (counted as if they started now), fed to [`nolabs_core::budget`].

use nolabs_core::budget::{self, Admission, DAY_MS, GpuUse};
use nolabs_core::proto::ApiError;

use crate::app::App;

impl App {
    /// GPU uses in the last 24 h plus every queued Ming job except `exclude_job`.
    pub(crate) async fn gpu_history(&self, now_ms: i64, exclude_job: Option<&str>) -> Result<Vec<GpuUse>, ApiError> {
        let repo = self.repo();
        let mut history = repo.gpu_uses_since(now_ms - DAY_MS).await?;
        for (id, requested_s) in repo.pending_gpu_requests().await? {
            if Some(id.as_str()) != exclude_job {
                history.push(GpuUse { at_ms: now_ms, requested_s, used_s: 0 });
            }
        }
        Ok(history)
    }
}

/// Admit a whole job or nothing: `Ok(request_s)`, or `Err(retry_at_ms)`. Jobs are sized at planning time to fit one
/// call, so a job that only partly fits waits for the window to reset instead of being split again.
pub fn admit_whole(
    policy: &budget::GpuPolicy,
    history: &[GpuUse],
    now_ms: i64,
    item_chars: &[usize],
) -> Result<u32, i64> {
    match budget::admit(policy, history, now_ms, item_chars) {
        Admission::Admit { request_s } => Ok(request_s),
        Admission::Shrink { .. } => {
            Err(budget::view(policy, history, now_ms).resets_at_ms.unwrap_or(now_ms + DAY_MS).max(now_ms + 60_000))
        }
        Admission::Defer { retry_at_ms } => Err(retry_at_ms),
    }
}

/// Most input chars one Ming call may carry so its estimate stays within `max_call_s`.
pub fn max_chars_per_call(policy: &budget::GpuPolicy) -> usize {
    let spare = policy.max_call_s.saturating_sub(policy.base_s) as f32;
    if policy.s_per_char <= 0.0 { usize::MAX } else { ((spare / policy.s_per_char).floor() as usize).max(1) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nolabs_core::budget::GpuPolicy;

    #[test]
    fn call_size_follows_the_policy() {
        // (180 - 8) / 0.06 = 2866.67
        assert_eq!(max_chars_per_call(&GpuPolicy::default()), 2866);
        let p = GpuPolicy { s_per_char: 0.0, ..GpuPolicy::default() };
        assert_eq!(max_chars_per_call(&p), usize::MAX);
    }
}
