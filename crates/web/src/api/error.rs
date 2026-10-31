//! Client-side errors and the friendly lines shown in toasts. Codes come from [`nolabs_core::proto::ErrorCode`]; the
//! UI branches on the code, never on the server's message text.

use nolabs_core::proto::{ApiError, ErrorCode};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClientError {
    /// The Worker answered with an error body (or a status we mapped to one).
    #[error("{0}")]
    Api(ApiError),
    /// No answer: offline, DNS, aborted.
    #[error("network: {0}")]
    Network(String),
    /// The answer did not parse.
    #[error("unexpected answer: {0}")]
    Decode(String),
    /// A browser API failed (storage, audio, files).
    #[error("{0}")]
    Browser(String),
}

impl ClientError {
    pub fn code(&self) -> Option<ErrorCode> {
        match self {
            ClientError::Api(e) => Some(e.code),
            _ => None,
        }
    }

    pub fn is_not_found(&self) -> bool {
        self.code() == Some(ErrorCode::NotFound)
    }

    pub fn is_offline(&self) -> bool {
        matches!(self, ClientError::Network(_))
    }

    /// One calm sentence for a toast.
    pub fn friendly(&self) -> String {
        match self {
            ClientError::Network(_) => t!("error-offline"),
            ClientError::Decode(_) => t!("error-unexpected"),
            ClientError::Browser(msg) => t!("error-browser", detail = msg.clone()),
            ClientError::Api(e) => friendly_api(e),
        }
    }
}

fn friendly_api(e: &ApiError) -> String {
    match e.code {
        ErrorCode::BadRequest => t!("error-bad-request"),
        ErrorCode::Unauthorized => t!("error-unauthorized"),
        ErrorCode::Forbidden => t!("error-forbidden"),
        ErrorCode::NotFound => t!("error-not-found"),
        ErrorCode::Conflict => t!("error-conflict"),
        ErrorCode::PayloadTooLarge => t!("error-too-large"),
        ErrorCode::RateLimited => match e.retry_after_s {
            Some(s) => t!("error-rate-limited-wait", wait = wait_phrase(s)),
            None => t!("error-rate-limited"),
        },
        ErrorCode::GpuBudget => match e.retry_after_s {
            Some(s) => t!("error-gpu-budget-wait", wait = wait_phrase(s)),
            None => t!("error-gpu-budget"),
        },
        ErrorCode::EngineUnavailable => t!("error-engine-waking"),
        ErrorCode::EngineFailed => t!("error-engine-failed"),
        ErrorCode::UpstreamFetch => t!("error-upstream"),
        ErrorCode::UpgradeRequired => t!("error-upgrade"),
        ErrorCode::Internal => t!("error-internal"),
    }
}

/// `"under a minute"`, `"3 minutes"`, `"about 2 hours"`.
pub fn wait_phrase(seconds: u64) -> String {
    match seconds {
        0..=59 => t!("wait-under-minute"),
        60..=3599 => t!("wait-minutes", minutes = seconds.div_ceil(60)),
        _ => t!("wait-hours", hours = ((seconds as f64 / 3600.0).round() as u64).max(1)),
    }
}

/// Fallback error for a non-2xx answer whose body is not an [`ApiError`] (e.g. a proxy page).
pub fn from_status(status: u16) -> ApiError {
    let code = match status {
        400 => ErrorCode::BadRequest,
        401 => ErrorCode::Unauthorized,
        403 => ErrorCode::Forbidden,
        404 => ErrorCode::NotFound,
        409 => ErrorCode::Conflict,
        413 => ErrorCode::PayloadTooLarge,
        429 => ErrorCode::RateLimited,
        502 | 504 => ErrorCode::UpstreamFetch,
        503 => ErrorCode::EngineUnavailable,
        _ => ErrorCode::Internal,
    };
    ApiError::new(code, format!("HTTP {status}"))
}

/// Parses an error body, falling back to the status.
pub fn parse_error_body(status: u16, body: &str) -> ApiError {
    serde_json::from_str::<ApiError>(body).unwrap_or_else(|_| from_status(status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_api_error_bodies_and_falls_back_to_status() {
        let e = parse_error_body(429, r#"{"code":"gpu_budget","message":"spent","retry_after_s":7200}"#);
        assert_eq!(e.code, ErrorCode::GpuBudget);
        assert_eq!(e.retry_after_s, Some(7200));
        assert_eq!(parse_error_body(503, "<html>").code, ErrorCode::EngineUnavailable);
        assert_eq!(parse_error_body(418, "").code, ErrorCode::Internal);
    }

    #[test]
    fn friendly_lines_use_the_retry_hint() {
        let e = ClientError::Api(ApiError::new(ErrorCode::GpuBudget, "x").with_retry_after(7200));
        assert_eq!(e.friendly(), "Today's expressive voice time is used up. It frees up in about 2 hours.");
        let e = ClientError::Api(ApiError::new(ErrorCode::RateLimited, "x").with_retry_after(90));
        assert_eq!(e.friendly(), "Too many attempts. Try again in 2 minutes.");
        assert!(ClientError::Network("x".into()).friendly().contains("offline"));
    }

    #[test]
    fn wait_phrases() {
        assert_eq!(wait_phrase(20), "under a minute");
        assert_eq!(wait_phrase(60), "a minute");
        assert_eq!(wait_phrase(61), "2 minutes");
        assert_eq!(wait_phrase(3600), "about an hour");
        assert_eq!(wait_phrase(5 * 3600 + 100), "about 5 hours");
    }

    #[test]
    fn predicates() {
        let e = ClientError::Api(ApiError::new(ErrorCode::Unauthorized, "x"));
        assert_eq!(e.code(), Some(ErrorCode::Unauthorized));
        assert!(!e.is_offline());
        assert!(ClientError::Api(ApiError::not_found("x")).is_not_found());
    }
}
