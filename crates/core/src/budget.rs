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
///
/// Uses recorded after `now_ms` (clock skew between Worker isolates) count as inside the window. This view never
/// under-counts HF's own chained windows: every use in HF's current window is also within the last 24 h.
pub fn view(policy: &GpuPolicy, history: &[GpuUse], now_ms: i64) -> BudgetView {
    let since = now_ms.saturating_sub(DAY_MS);
    let mut used_s = 0u32;
    let mut runs_used = 0u32;
    let mut first: Option<i64> = None;
    for use_ in history.iter().filter(|u| u.at_ms > since) {
        used_s = used_s.saturating_add(use_.used_s.max(charged(policy, use_.requested_s)));
        runs_used = runs_used.saturating_add(1);
        first = Some(first.map_or(use_.at_ms, |f| f.min(use_.at_ms)));
    }
    BudgetView {
        used_s,
        runs_used,
        remaining_s: policy.daily_cap_s.saturating_sub(used_s),
        runs_left: policy.runs_cap.saturating_sub(runs_used),
        resets_at_ms: first.map(|f| f.saturating_add(DAY_MS)),
    }
}

/// Requested seconds for a batch with this many input chars: `base_s + s_per_char * chars`, rounded up, capped at
/// `max_call_s`.
pub fn estimate_seconds(policy: &GpuPolicy, chars: usize) -> u32 {
    clamp_seconds(raw_estimate(policy, chars).min(f64::from(policy.max_call_s)))
}

/// Decide what to submit given per-item char counts, in order. Admit when the whole batch fits both caps; otherwise the
/// longest prefix whose `estimate * duration_factor` fits `remaining_s` (at least one item); otherwise Defer to
/// `resets_at_ms` (or `now + DAY_MS` when unknown). No runs left also defers.
///
/// "Both caps" are the window's remaining seconds and `max_call_s`: a batch whose uncapped estimate exceeds
/// `max_call_s` would run out of GPU time mid-batch, so only a prefix that fits within one call is admitted. A single
/// item is the smallest unit and only has to fit the remaining seconds at its capped estimate. An empty batch is
/// admitted with a zero request (there is nothing to run).
pub fn admit(policy: &GpuPolicy, history: &[GpuUse], now_ms: i64, item_chars: &[usize]) -> Admission {
    let budget = view(policy, history, now_ms);
    admit_within(policy, &budget, budget.resets_at_ms, now_ms, item_chars)
}

/// One user's slice of the deployment's budget: their own uses and their daily share in seconds.
#[derive(Debug, Clone, Copy)]
pub struct Share<'a> {
    pub daily_s: u32,
    pub history: &'a [GpuUse],
}

/// [`admit`] for one user: the call must fit the deployment's budget (`all`, which also holds the runs cap, an HF
/// account limit shared by everyone) and the user's own share. When the share is what blocks, the retry time is
/// when the user's own window resets, not the deployment's.
pub fn admit_for_user(
    policy: &GpuPolicy,
    all: &[GpuUse],
    share: Share<'_>,
    now_ms: i64,
    item_chars: &[usize],
) -> Admission {
    let global = view(policy, all, now_ms);
    let user_policy = GpuPolicy { daily_cap_s: share.daily_s.min(policy.daily_cap_s), runs_cap: u32::MAX, ..*policy };
    let mine = view(&user_policy, share.history, now_ms);
    let (limit, resets) = if mine.remaining_s < global.remaining_s {
        (BudgetView { runs_left: global.runs_left, ..mine }, mine.resets_at_ms)
    } else {
        (global, global.resets_at_ms)
    };
    // A share too small for even one item, with nothing used yet, has no reset time: it waits a day.
    admit_within(policy, &limit, resets.or(global.resets_at_ms), now_ms, item_chars)
}

/// The admission rule over an already computed view: `remaining_s` and `runs_left` from `budget`, deferring to
/// `resets_at_ms` (or a day from now when unknown).
fn admit_within(
    policy: &GpuPolicy,
    budget: &BudgetView,
    resets_at_ms: Option<i64>,
    now_ms: i64,
    item_chars: &[usize],
) -> Admission {
    let defer = Admission::Defer { retry_at_ms: resets_at_ms.unwrap_or(now_ms.saturating_add(DAY_MS)) };
    if item_chars.is_empty() {
        return Admission::Admit { request_s: 0 };
    }
    if budget.runs_left == 0 {
        return defer;
    }
    let fits = |items: usize, chars: usize| {
        let request_s = estimate_seconds(policy, chars);
        let within_call = items == 1 || raw_estimate(policy, chars) <= f64::from(policy.max_call_s);
        (within_call && charged(policy, request_s) <= budget.remaining_s).then_some(request_s)
    };
    let mut best = None;
    let mut chars = 0usize;
    for (i, &n) in item_chars.iter().enumerate() {
        chars = chars.saturating_add(n);
        // Estimates grow with chars, so the first prefix that does not fit ends the search.
        match fits(i + 1, chars) {
            Some(request_s) => best = Some((i + 1, request_s)),
            None => break,
        }
    }
    match best {
        Some((items, request_s)) if items == item_chars.len() => Admission::Admit { request_s },
        Some((items, request_s)) => Admission::Shrink { items, request_s },
        None => defer,
    }
}

/// Seconds the scheduler charges for a request: `requested_s * duration_factor`, rounded up.
fn charged(policy: &GpuPolicy, requested_s: u32) -> u32 {
    clamp_seconds(f64::from(requested_s) * f64::from(policy.duration_factor))
}

fn raw_estimate(policy: &GpuPolicy, chars: usize) -> f64 {
    // usize -> f64 is exact below 2^53 chars, far beyond any batch.
    f64::from(policy.base_s) + f64::from(policy.s_per_char) * chars as f64
}

/// Round up to whole seconds. `f32` policy values are not exact in binary (0.06 is 0.0599999986…), so the value is
/// first rounded to the millisecond; otherwise 100 chars at 0.07 s/char would round 7.0000003 s up to 8 s.
fn clamp_seconds(seconds: f64) -> u32 {
    let seconds = (seconds * 1000.0).round() / 1000.0;
    // `as` saturates: negative and NaN become 0, anything above u32::MAX becomes u32::MAX.
    seconds.ceil() as u32
}

#[cfg(test)]
mod tests;
