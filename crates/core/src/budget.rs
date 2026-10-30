//! The ZeroGPU budget: a rolling 24 h window that starts at the first GPU use, a cap on GPU seconds and on runs, and
//! the 1.5x factor the scheduler applies to the requested duration before admitting a call.

use serde::{Deserialize, Serialize};

pub const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GpuPolicy {
    /// Our own daily cap, below HF's 300 s so one surprise never locks us out.
    pub daily_cap_s: u32,
    /// Runs per window (HF staff: 3 for free accounts, April 2026; adjust after measuring).
    pub runs_cap: u32,
    /// Scheduler multiplier on the requested duration for the `large` size.
    pub duration_factor: f32,
    /// Fixed seconds per call on top of the per-char estimate (model warm-up, prompt encoding).
    pub base_s: u32,
    /// Seconds of GPU per input char, measured (see notes/08).
    pub s_per_char: f32,
    /// Ceiling for one call's requested duration.
    pub max_call_s: u32,
}

impl Default for GpuPolicy {
    fn default() -> Self {
        GpuPolicy { daily_cap_s: 280, runs_cap: 3, duration_factor: 1.5, base_s: 8, s_per_char: 0.06, max_call_s: 180 }
    }
}

/// One GPU call as recorded in the ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuUse {
    pub at_ms: i64,
    pub requested_s: u32,
    /// Actual GPU seconds reported by the Space (0 when the call failed before running).
    pub used_s: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetView {
    pub used_s: u32,
    pub runs_used: u32,
    pub remaining_s: u32,
    pub runs_left: u32,
    /// When the window resets (first use in the window + 24 h); None when nothing was used in the last 24 h.
    pub resets_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// Submit all items with this requested duration.
    Admit { request_s: u32 },
    /// Submit only the first `items` items with this requested duration; queue the rest.
    Shrink { items: usize, request_s: u32 },
    /// Nothing fits; try again at this time.
    Defer { retry_at_ms: i64 },
}

/// Budget state at `now_ms`. The window is HF's: it starts at the earliest use within the last 24 h and resets 24 h
/// after it; uses older than that are ignored. `used_s` counts `max(used_s, requested_s * duration_factor)` per call,
/// because admission is charged on the request.
pub fn view(policy: &GpuPolicy, history: &[GpuUse], now_ms: i64) -> BudgetView {
    let _ = (policy, history, now_ms);
    todo!("implemented in the core crate")
}

/// Requested seconds for a batch with this many input chars: `base_s + s_per_char * chars`, rounded up, capped at
/// `max_call_s`.
pub fn estimate_seconds(policy: &GpuPolicy, chars: usize) -> u32 {
    let _ = (policy, chars);
    todo!("implemented in the core crate")
}

/// Decide what to submit given per-item char counts, in order. Admit when the whole batch fits both caps; otherwise the
/// longest prefix whose `estimate * duration_factor` fits `remaining_s` (at least one item); otherwise Defer to
/// `resets_at_ms` (or `now + DAY_MS` when unknown). No runs left also defers.
pub fn admit(policy: &GpuPolicy, history: &[GpuUse], now_ms: i64, item_chars: &[usize]) -> Admission {
    let _ = (policy, history, now_ms, item_chars);
    todo!("implemented in the core crate")
}
