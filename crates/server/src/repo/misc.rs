//! GPU ledger, login failures and the config table.

use nolabs_core::budget::GpuUse;

use super::row;
use super::{Repo, sql};
use crate::ports::{PortError, Stmt};

impl Repo<'_> {
    /// Ledger entries newer than `since_ms`, oldest first.
    pub async fn gpu_uses_since(&self, since_ms: i64) -> Result<Vec<GpuUse>, PortError> {
        self.all(sql::LEDGER_SINCE, vec![since_ms.into()])
            .await?
            .iter()
            .map(|r| {
                Ok(GpuUse {
                    at_ms: row::i64(r, "at_ms")?,
                    requested_s: row::u32(r, "requested_s")?,
                    used_s: row::u32(r, "used_s")?,
                })
            })
            .collect()
    }

    pub fn ledger_insert_stmt(job_id: &str, attempt: u32, at_ms: i64, requested_s: u32) -> Stmt {
        Stmt::new(sql::LEDGER_INSERT, vec![job_id.into(), attempt.into(), at_ms.into(), requested_s.into()])
    }

    pub fn ledger_used_stmt(job_id: &str, attempt: u32, used_s: u32) -> Stmt {
        Stmt::new(sql::LEDGER_SET_USED, vec![used_s.into(), job_id.into(), attempt.into()])
    }

    pub fn ledger_delete_stmt(job_id: &str, attempt: u32) -> Stmt {
        Stmt::new(sql::LEDGER_DELETE, vec![job_id.into(), attempt.into()])
    }

    pub async fn prune_ledger(&self, before_ms: i64) -> Result<u64, PortError> {
        self.run(sql::LEDGER_PRUNE, vec![before_ms.into()]).await
    }

    /// Number of failed logins after `since_ms` and the time of the oldest of them.
    pub async fn auth_failures_since(&self, since_ms: i64) -> Result<(u32, Option<i64>), PortError> {
        match self.all(sql::AUTH_FAILURES_SINCE, vec![since_ms.into()]).await?.first() {
            Some(r) => Ok((row::u32(r, "n")?, row::opt_i64(r, "first_ms")?)),
            None => Ok((0, None)),
        }
    }

    pub async fn record_auth_failure(&self, at_ms: i64) -> Result<(), PortError> {
        self.run(sql::AUTH_FAILURE_INSERT, vec![at_ms.into()]).await?;
        Ok(())
    }

    pub async fn clear_auth_failures(&self) -> Result<(), PortError> {
        self.run(sql::AUTH_FAILURES_CLEAR, vec![]).await?;
        Ok(())
    }

    pub async fn prune_auth_failures(&self, before_ms: i64) -> Result<u64, PortError> {
        self.run(sql::AUTH_FAILURES_PRUNE, vec![before_ms.into()]).await
    }

    pub async fn config_get(&self, key: &str) -> Result<Option<String>, PortError> {
        match self.all(sql::CONFIG_GET, vec![key.into()]).await?.first() {
            Some(r) => row::opt_str(r, "value"),
            None => Ok(None),
        }
    }

    pub async fn config_put(&self, key: &str, value: &str, now_ms: i64) -> Result<(), PortError> {
        self.run(sql::CONFIG_PUT, vec![key.into(), value.into(), now_ms.into()]).await?;
        Ok(())
    }

    /// The current session epoch (1 when never bumped).
    pub async fn session_epoch(&self) -> Result<u64, PortError> {
        Ok(self.config_get(CONFIG_SESSION_EPOCH).await?.and_then(|v| v.parse().ok()).unwrap_or(1))
    }

    /// Increment the session epoch, revoking every cookie issued before; returns the new epoch.
    pub async fn bump_session_epoch(&self, now_ms: i64) -> Result<u64, PortError> {
        let rows = self.all(sql::CONFIG_BUMP_EPOCH, vec![now_ms.into()]).await?;
        rows.first()
            .map(|r| row::str(r, "value"))
            .transpose()?
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| PortError::new("session epoch not returned"))
    }
}

pub const CONFIG_SESSION_EPOCH: &str = "session_epoch";
pub const CONFIG_SPACE_STATUS: &str = "space_status";
