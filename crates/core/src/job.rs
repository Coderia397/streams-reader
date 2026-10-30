//! Render state machine shared by the Worker's D1 rows and the queue consumer.
//!
//! ```text
//! (miss) --claim--> Queued --start--> Rendering --done--> Ready
//!                      ^                  |  \--fail(transient, CPU, attempts left)--> Queued
//!                      |                  |---fail(budget)--> Deferred --due--> Queued
//!                      |                  \---fail(other) or GPU failure--> Failed
//!                      \--lease expired (CPU only)--/
//! ```
//! A CPU row whose lease expires after its last attempt fails instead of re-queueing.
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

/// Wait before a failed CPU job is retried: `2^attempts * 15 s` (30 s, 60 s, 120 s for attempts 1–3).
const CPU_BACKOFF_UNIT_MS: i64 = 15_000;
/// Wait before a budget-deferred job is retried when the error gave no time.
const BUDGET_DEFAULT_RETRY_MS: i64 = 60 * 60 * 1000;

/// Apply one event. Invalid transitions return the row unchanged with `Err` describing why.
/// - `Start` from Queued: Rendering, attempts + 1, lease = now + LEASE_MS.
/// - `Done` from Rendering: Ready, lease cleared.
/// - `Fail` from Rendering: budget errors → Deferred with retry_at = now + retry_after (or +1 h); transient errors on
///   Kokoro with attempts < MAX_CPU_ATTEMPTS → Queued with retry_at = now + 2^attempts * 15 s; everything else
///   (including any Ming failure that is not a budget error) → Failed.
/// - `LeaseExpired` from Rendering: Kokoro → Queued; Ming → Failed (never resubmit a GPU job blindly). A Kokoro row that
///   has already used `MAX_CPU_ATTEMPTS` attempts → Failed, so a text that crashes the consumer cannot loop forever.
/// - `Due` from Deferred: Queued.
///
/// Every transition clears what no longer applies: the lease outside Rendering, `retry_at_ms` outside Queued and
/// Deferred.
pub fn apply(row: &Row, event: &Event) -> Result<Row, String> {
    use RenderState::{Deferred, Failed, Queued, Ready, Rendering};

    let to =
        |state: RenderState, retry_at_ms: Option<i64>| Row { state, lease_until_ms: None, retry_at_ms, ..row.clone() };
    match (row.state, event) {
        (Queued, Event::Start { now_ms }) => Ok(Row {
            state: Rendering,
            attempts: row.attempts.saturating_add(1),
            lease_until_ms: Some(now_ms.saturating_add(LEASE_MS)),
            retry_at_ms: None,
            ..row.clone()
        }),
        (Rendering, Event::Done) => Ok(to(Ready, None)),
        (Rendering, Event::Fail { error, now_ms }) => Ok(if error.is_budget() {
            to(Deferred, Some(now_ms.saturating_add(budget_retry_ms(error))))
        } else if error.is_transient() && row.engine == Engine::Kokoro && row.attempts < MAX_CPU_ATTEMPTS {
            to(Queued, Some(now_ms.saturating_add(cpu_backoff_ms(row.attempts))))
        } else {
            to(Failed, None)
        }),
        (Rendering, Event::LeaseExpired) => Ok(match row.engine {
            Engine::Kokoro if row.attempts < MAX_CPU_ATTEMPTS => to(Queued, None),
            Engine::Kokoro | Engine::Ming => to(Failed, None),
        }),
        (Deferred, Event::Due) => Ok(to(Queued, None)),
        (state, event) => Err(format!("cannot apply {} to a {state:?} row", event_name(event))),
    }
}

fn budget_retry_ms(error: &EngineError) -> i64 {
    match error {
        EngineError::Quota { retry_after_s: Some(s) } => i64::try_from(*s).unwrap_or(i64::MAX).saturating_mul(1000),
        _ => BUDGET_DEFAULT_RETRY_MS,
    }
}

fn cpu_backoff_ms(attempts: u32) -> i64 {
    // Attempts stay below MAX_CPU_ATTEMPTS here; the clamp only keeps the shift defined.
    CPU_BACKOFF_UNIT_MS.saturating_mul(1i64 << attempts.min(32))
}

fn event_name(event: &Event) -> &'static str {
    match event {
        Event::Start { .. } => "Start",
        Event::Done => "Done",
        Event::Fail { .. } => "Fail",
        Event::LeaseExpired => "LeaseExpired",
        Event::Due => "Due",
    }
}

#[cfg(test)]
mod tests;
