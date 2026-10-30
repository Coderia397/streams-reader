//! One classifier for every way a call to the speech Space fails, and one policy for what to do about it.

use crate::proto::{ApiError, ErrorCode};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineError {
    /// "You have exceeded your ZeroGPU runs limit." (per-account run count).
    #[error("ZeroGPU runs limit reached")]
    RunsLimit,
    /// "You have exceeded your … quota (Ns requested vs. Ms left). Try again in H:MM:SS."
    #[error("ZeroGPU quota exhausted")]
    Quota { retry_after_s: Option<u64> },
    /// "ZeroGPU illegal duration: The requested GPU duration (Ns) is larger than the maximum allowed".
    #[error("requested GPU duration too long")]
    IllegalDuration,
    /// Space sleeping, building, paused or restarting (HTTP 503/502/404 from the HF proxy, or HTML instead of JSON).
    #[error("speech engine unavailable")]
    Sleeping,
    /// Gradio queue full or the GPU busy ("No GPU is currently available", "queue is full").
    #[error("speech engine busy")]
    Busy,
    /// The Space rejected the request ("bad request: …" from `contracts.ContractError`).
    #[error("bad request: {0}")]
    BadInput(String),
    /// 401/403 from the HF proxy.
    #[error("not authorised to call the speech engine")]
    Auth,
    #[error("engine error: {0}")]
    Internal(String),
}

impl EngineError {
    /// Classify an HTTP status (when the failure was at the HTTP layer) and/or an error message (Gradio SSE `error`
    /// event data, or a response body). Matching is case-insensitive on stable substrings of the messages above;
    /// `Try again in 1:02:03` and `Try again in 0:00:42` are parsed into seconds.
    ///
    /// Precedence: ZeroGPU budget messages win over everything (they arrive with assorted statuses), then the Space's
    /// own `bad request:` prefix, then busy messages, then the HTTP status (401/403 auth, 429 busy, 404/502/503/504
    /// sleeping), then an HTML body (the HF proxy's page instead of JSON), then `engine error:` and other text.
    pub fn classify(http_status: Option<u16>, message: &str) -> EngineError {
        let message = message.trim();
        // ASCII lowercasing keeps byte offsets, so a match position also indexes `message`.
        let lower = message.to_ascii_lowercase();
        let has = |needle: &str| lower.contains(needle);

        if has("runs limit") {
            return EngineError::RunsLimit;
        }
        if has("illegal duration") {
            return EngineError::IllegalDuration;
        }
        if has("quota") && (has("exceeded") || has("gpu")) {
            return EngineError::Quota { retry_after_s: retry_after_seconds(&lower) };
        }
        if let Some(detail) = after(message, &lower, "bad request:") {
            return EngineError::BadInput(clip(detail));
        }
        if BUSY.iter().any(|n| has(n)) {
            return EngineError::Busy;
        }
        match http_status {
            Some(401 | 403) => return EngineError::Auth,
            Some(429) => return EngineError::Busy,
            Some(404 | 502 | 503 | 504) => return EngineError::Sleeping,
            _ => {}
        }
        if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
            return EngineError::Sleeping;
        }
        if let Some(detail) = after(message, &lower, "engine error:") {
            return EngineError::Internal(clip(detail));
        }
        if has("space") && SPACE_DOWN.iter().any(|n| has(n)) {
            return EngineError::Sleeping;
        }
        match (message.is_empty(), http_status) {
            (true, Some(status)) => EngineError::Internal(format!("HTTP {status}")),
            (true, None) => EngineError::Internal("no error message".to_owned()),
            (false, _) => EngineError::Internal(clip(message)),
        }
    }

    /// Failures worth retrying automatically later without user action (CPU jobs only; GPU jobs never auto-retry).
    pub fn is_transient(&self) -> bool {
        matches!(self, EngineError::Sleeping | EngineError::Busy | EngineError::Internal(_))
    }

    /// Failures caused by the GPU budget; the job becomes `deferred`.
    pub fn is_budget(&self) -> bool {
        matches!(self, EngineError::RunsLimit | EngineError::Quota { .. } | EngineError::IllegalDuration)
    }

    pub fn to_api(&self) -> ApiError {
        let code = match self {
            EngineError::RunsLimit | EngineError::Quota { .. } | EngineError::IllegalDuration => ErrorCode::GpuBudget,
            EngineError::Sleeping | EngineError::Busy => ErrorCode::EngineUnavailable,
            EngineError::BadInput(_) => ErrorCode::BadRequest,
            EngineError::Auth | EngineError::Internal(_) => ErrorCode::EngineFailed,
        };
        let err = ApiError::new(code, self.to_string());
        match self {
            EngineError::Quota { retry_after_s: Some(s) } => err.with_retry_after(*s),
            _ => err,
        }
    }
}

/// Gradio queue or ZeroGPU scheduler is saturated; worth retrying soon.
const BUSY: &[&str] = &["no gpu is currently available", "queue is full", "too many requests"];

/// Words the HF proxy and Gradio use for a Space that is not running, matched only when the text mentions the Space.
const SPACE_DOWN: &[&str] = &["sleeping", "paused", "building", "restarting", "starting", "not running"];

/// Longest message kept in an error, in chars; HTML or tracebacks beyond this only bloat D1 rows.
const MAX_MESSAGE_CHARS: usize = 300;

/// The trimmed text after the first occurrence of `marker` (ASCII, lowercase) in `message`.
fn after<'a>(message: &'a str, lower: &str, marker: &str) -> Option<&'a str> {
    lower.find(marker).map(|at| message[at + marker.len()..].trim())
}

fn clip(text: &str) -> String {
    match text.char_indices().nth(MAX_MESSAGE_CHARS) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_owned(),
    }
}

/// Seconds in `try again in H:MM:SS` (also `M:SS` or a bare number of seconds), from the lowercased message.
fn retry_after_seconds(lower: &str) -> Option<u64> {
    const MARKER: &str = "try again in";
    let rest = lower[lower.find(MARKER)? + MARKER.len()..].trim_start();
    let clock: &str = &rest[..rest.find(|c: char| !(c.is_ascii_digit() || c == ':')).unwrap_or(rest.len())];
    let parts: Vec<&str> = clock.split(':').collect();
    if parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    parts.iter().try_fold(0u64, |total, part| total.checked_mul(60)?.checked_add(part.parse().ok()?))
}

#[cfg(test)]
mod tests;
