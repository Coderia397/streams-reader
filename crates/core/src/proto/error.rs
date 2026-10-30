use serde::{Deserialize, Serialize};

/// Machine-readable error codes. The web app branches on these; `message` is for people.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    PayloadTooLarge,
    RateLimited,
    /// The daily GPU budget (seconds or runs) is spent; `retry_after_s` says when it frees up.
    GpuBudget,
    /// The speech Space is asleep, building or restarting.
    EngineUnavailable,
    /// The speech Space returned an error for this request.
    EngineFailed,
    UpstreamFetch,
    UpgradeRequired,
    Internal,
}

impl ErrorCode {
    pub fn http_status(self) -> u16 {
        match self {
            ErrorCode::BadRequest => 400,
            ErrorCode::Unauthorized => 401,
            ErrorCode::Forbidden => 403,
            ErrorCode::NotFound => 404,
            ErrorCode::Conflict | ErrorCode::UpgradeRequired => 409,
            ErrorCode::PayloadTooLarge => 413,
            ErrorCode::RateLimited | ErrorCode::GpuBudget => 429,
            ErrorCode::UpstreamFetch => 502,
            ErrorCode::EngineUnavailable => 503,
            ErrorCode::EngineFailed | ErrorCode::Internal => 500,
        }
    }
}

/// The JSON body of every non-2xx API response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_s: Option<u64>,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), retry_after_s: None }
    }

    pub fn with_retry_after(mut self, seconds: u64) -> Self {
        self.retry_after_s = Some(seconds);
        self
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::BadRequest, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn status(&self) -> u16 {
        self.code.http_status()
    }
}
