//! The D1 repository: typed reads and writes over [`Db`]. All SQL text lives in [`sql`]; this module and its children
//! only bind parameters and decode rows. The tests run it on SQLite against `migrations/*.sql`.

mod jobs;
mod misc;
mod renders;
mod row;
pub mod sql;
mod voices;

pub use jobs::{JobKind, JobRow, JobUpdate, NewJob};
pub use misc::{CONFIG_SPACE_STATUS, LedgerUse, Owned};
pub use renders::{NewClaim, RenderRow, seconds_until};
pub use voices::VoiceRow;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::ports::{Db, PortError, SqlValue, Stmt, StmtResult};

pub struct Repo<'a> {
    db: &'a dyn Db,
}

impl<'a> Repo<'a> {
    pub fn new(db: &'a dyn Db) -> Self {
        Repo { db }
    }

    async fn all(&self, sql: impl Into<String>, params: Vec<SqlValue>) -> Result<Vec<row::Row>, PortError> {
        self.db.all(&Stmt::new(sql, params)).await
    }

    async fn run(&self, sql: impl Into<String>, params: Vec<SqlValue>) -> Result<u64, PortError> {
        self.db.run(&Stmt::new(sql, params)).await
    }

    /// Run statements atomically. Callers build them with the `*_stmt` helpers of each child module.
    pub async fn batch(&self, stmts: &[Stmt]) -> Result<Vec<StmtResult>, PortError> {
        if stmts.is_empty() {
            return Ok(Vec::new());
        }
        self.db.batch(stmts).await
    }
}

/// The snake_case wire name of a serde enum (`RenderState::Queued` → `queued`), as stored in TEXT columns.
pub fn enum_text<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        _ => String::new(),
    }
}

/// The inverse of [`enum_text`].
pub fn parse_enum<T: DeserializeOwned>(text: &str) -> Result<T, PortError> {
    serde_json::from_value(serde_json::Value::String(text.to_owned()))
        .map_err(|_| PortError::new(format!("unexpected enum value {text:?} in database")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nolabs_core::proto::api::RenderState;

    #[test]
    fn enum_text_round_trips() {
        assert_eq!(enum_text(&RenderState::Deferred), "deferred");
        assert_eq!(parse_enum::<RenderState>("rendering").unwrap(), RenderState::Rendering);
        assert!(parse_enum::<RenderState>("nope").is_err());
    }
}
