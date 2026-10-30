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
    pub fn classify(http_status: Option<u16>, message: &str) -> EngineError {
        let _ = (http_status, message);
        todo!("implemented in the core crate")
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
