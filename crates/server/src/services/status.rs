//! `GET /api/status`: the Space's last known stage (written by the cron), the deployment's GPU budget, the caller's
//! share of it and the render backlog.

use nolabs_core::budget;
use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::{GpuBudgetView, SpaceStatus, SystemStatus, UserGpuView};

use crate::app::App;
use crate::auth::AuthUser;
use crate::repo::CONFIG_SPACE_STATUS;

impl App {
    /// The Space status the cron stored last, or `UNKNOWN` before its first run.
    pub async fn space_status(&self) -> Result<SpaceStatus, ApiError> {
        let stored = self.repo().config_get(CONFIG_SPACE_STATUS).await?;
        Ok(stored.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| SpaceStatus {
            stage: "UNKNOWN".to_owned(),
            healthy: false,
            checked_at_ms: 0,
        }))
    }

    pub async fn system_status(&self, user: &AuthUser) -> Result<SystemStatus, ApiError> {
        let now = self.now_ms();
        let policy = self.config.gpu;
        let history = self.gpu_history(now, &user.id, None).await?;
        let view = budget::view(&policy, &history.all, now);
        let share_s = self.accounts.gpu_share(user.token()?, policy.daily_cap_s).await?;
        let mine = budget::view(&budget::GpuPolicy { daily_cap_s: share_s, ..policy }, &history.mine, now);
        Ok(SystemStatus {
            version: self.config.app_version.clone(),
            space: self.space_status().await?,
            gpu: GpuBudgetView {
                used_s: view.used_s,
                cap_s: policy.daily_cap_s,
                runs_used: view.runs_used,
                runs_cap: policy.runs_cap,
                resets_at_ms: view.resets_at_ms,
            },
            my_gpu: UserGpuView { used_s: mine.used_s, share_s, resets_at_ms: mine.resets_at_ms },
            queued: self.repo().count_active_renders().await?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{TestEnv, run};
    use serde_json::json;

    #[test]
    fn reports_stage_budget_and_backlog() {
        let env = TestEnv::new();
        let now = env.now();
        env.db.exec(&format!(
            "INSERT INTO config (key, value, updated_at_ms) VALUES ('space_status', \
             '{{\"stage\":\"RUNNING\",\"healthy\":true,\"checked_at_ms\":{now}}}', 0); \
             INSERT INTO gpu_ledger (job_id, attempt, at_ms, requested_s, used_s, user_id) VALUES \
             ('j', 1, {now}, 20, 40, 'alice'), ('k', 1, {now}, 10, 12, 'bob'); \
             INSERT INTO renders (key, engine, voice, state, object_key, chars, created_at_ms, updated_at_ms) VALUES \
             ('a', 'kokoro', 'kokoro:af_heart', 'queued', 'o', 1, 0, 0), \
             ('b', 'kokoro', 'kokoro:af_heart', 'ready', 'o', 1, 0, 0)"
        ));
        env.accounts.set_share("alice", 60);
        let s = run(env.app.system_status(&env.user("alice"))).unwrap();
        assert_eq!(s.version, "test");
        assert_eq!((s.space.stage.as_str(), s.space.healthy), ("RUNNING", true));
        assert_eq!((s.gpu.used_s, s.gpu.runs_used, s.gpu.cap_s, s.gpu.runs_cap), (55, 2, 280, 3));
        assert_eq!((s.my_gpu.used_s, s.my_gpu.share_s), (40, 60));
        assert_eq!(s.queued, 1);
        let fresh = TestEnv::new();
        let s = run(fresh.app.system_status(&fresh.user("bob"))).unwrap();
        assert_eq!(serde_json::to_value(&s.space).unwrap()["stage"], json!("UNKNOWN"));
    }
}
