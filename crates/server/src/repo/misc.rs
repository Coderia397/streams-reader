//! GPU ledger, the ownership cache and the config table.

use nolabs_core::budget::GpuUse;

use super::row;
use super::{Repo, sql};
use crate::ports::{PortError, Stmt};

/// A ledger entry and the user it is charged to (empty for calls from before accounts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerUse {
    pub user_id: String,
    pub gpu: GpuUse,
}

/// What an ownership-cache entry vouches for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owned {
    /// The user's document, with its body uploaded.
    ReadyDocument,
    Voice,
}

impl Owned {
    fn kind(self) -> &'static str {
        match self {
            Owned::ReadyDocument => "doc",
            Owned::Voice => "voice",
        }
    }
}

impl Repo<'_> {
    /// Ledger entries newer than `since_ms`, oldest first.
    pub async fn gpu_uses_since(&self, since_ms: i64) -> Result<Vec<LedgerUse>, PortError> {
        self.all(sql::LEDGER_SINCE, vec![since_ms.into()])
            .await?
            .iter()
            .map(|r| {
                Ok(LedgerUse {
                    user_id: row::str(r, "user_id")?,
                    gpu: GpuUse {
                        at_ms: row::i64(r, "at_ms")?,
                        requested_s: row::u32(r, "requested_s")?,
                        used_s: row::u32(r, "used_s")?,
                    },
                })
            })
            .collect()
    }

    pub fn ledger_insert_stmt(job_id: &str, attempt: u32, at_ms: i64, requested_s: u32, user_id: &str) -> Stmt {
        Stmt::new(
            sql::LEDGER_INSERT,
            vec![job_id.into(), attempt.into(), at_ms.into(), requested_s.into(), user_id.into()],
        )
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

    /// True while a positive ownership answer for this user and object is cached.
    pub async fn owner_cached(
        &self,
        what: Owned,
        object_id: &str,
        user_id: &str,
        now_ms: i64,
    ) -> Result<bool, PortError> {
        let rows = self
            .all(sql::OWNER_CACHE_HIT, vec![what.kind().into(), object_id.into(), user_id.into(), now_ms.into()])
            .await?;
        Ok(!rows.is_empty())
    }

    pub async fn cache_owner(
        &self,
        what: Owned,
        object_id: &str,
        user_id: &str,
        until_ms: i64,
    ) -> Result<(), PortError> {
        self.run(sql::OWNER_CACHE_PUT, vec![what.kind().into(), object_id.into(), user_id.into(), until_ms.into()])
            .await?;
        Ok(())
    }

    /// Drop every cached answer about an object (it was deleted).
    pub async fn forget_owner(&self, what: Owned, object_id: &str) -> Result<(), PortError> {
        self.run(sql::OWNER_CACHE_FORGET, vec![what.kind().into(), object_id.into()]).await?;
        Ok(())
    }

    pub async fn prune_owner_cache(&self, now_ms: i64) -> Result<u64, PortError> {
        self.run(sql::OWNER_CACHE_PRUNE, vec![now_ms.into()]).await
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
}

pub const CONFIG_SPACE_STATUS: &str = "space_status";
