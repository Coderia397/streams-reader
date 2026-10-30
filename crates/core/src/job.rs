//! Render state machine shared by the Worker's D1 rows and the queue consumer.
//!
//! ```text
//! (miss) --claim--> Queued --start--> Rendering --done--> Ready
//!                      ^                  |  \--fail(transient, CPU, attempts left)--> Queued
//!                      |                  |---fail(budget)--> Deferred --due--> Queued
//!                      |                  \---fail(other) or GPU failure--> Failed
//!                      \--lease expired (CPU only)--/
//! ```
//! GPU renders are never re-queued automatically after they started, because the Space may still finish and a retry
//! would spend another of the day's few runs; the consumer checks the R2 completion marker instead.

use crate::engine_error::EngineError;
use crate::proto::api::{Engine, RenderState};

pub const MAX_CPU_ATTEMPTS: u32 = 4;
/// How long a consumer owns a Rendering row before the cron may take it back.
pub const LEASE_MS: i64 = 12 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start { now_ms: i64 },
    Done,
    Fail { error: EngineError, now_ms: i64 },
    LeaseExpired,
    Due,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub state: RenderState,
    pub engine: Engine,
    pub attempts: u32,
    pub lease_until_ms: Option<i64>,
    pub retry_at_ms: Option<i64>,
}

/// Apply one event. Invalid transitions return the row unchanged with `Err` describing why.
/// - `Start` from Queued: Rendering, attempts + 1, lease = now + LEASE_MS.
/// - `Done` from Rendering: Ready, lease cleared.
/// - `Fail` from Rendering: budget errors → Deferred with retry_at = now + retry_after (or +1 h); transient errors on
///   Kokoro with attempts < MAX_CPU_ATTEMPTS → Queued with retry_at = now + 2^attempts * 15 s; everything else
///   (including any Ming failure that is not a budget error) → Failed.
/// - `LeaseExpired` from Rendering: Kokoro → Queued; Ming → Failed (never resubmit a GPU job blindly).
/// - `Due` from Deferred: Queued.
pub fn apply(row: &Row, event: &Event) -> Result<Row, String> {
    let _ = (row, event);
    todo!("implemented in the core crate")
}
