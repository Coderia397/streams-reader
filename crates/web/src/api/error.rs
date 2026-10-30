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

    pub fn is_unauthorized(&self) -> bool {
        self.code() == Some(ErrorCode::Unauthorized)
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
            ClientError::Network(_) => "You're offline. Check your connection and try again.".to_owned(),
            ClientError::Decode(_) => "The server sent something unexpected. Try again in a moment.".to_owned(),
            ClientError::Browser(msg) => format!("Your browser refused: {msg}"),
            ClientError::Api(e) => friendly_api(e),
        }
    }
}

fn friendly_api(e: &ApiError) -> String {
    match e.code {
        ErrorCode::BadRequest => "That request wasn't quite right. Try again.".to_owned(),
        ErrorCode::Unauthorized => "Please sign in again.".to_owned(),
        ErrorCode::Forbidden => "That isn't allowed.".to_owned(),
        ErrorCode::NotFound => "That item no longer exists.".to_owned(),
        ErrorCode::Conflict => "That changed somewhere else. Refresh and try again.".to_owned(),
        ErrorCode::PayloadTooLarge => "That's too large to upload.".to_owned(),
        ErrorCode::RateLimited => match e.retry_after_s {
            Some(s) => format!("Too many attempts. Try again in {}.", wait_phrase(s)),
            None => "Too many attempts. Wait a little and try again.".to_owned(),
        },
        ErrorCode::GpuBudget => match e.retry_after_s {
            Some(s) => format!("Today's expressive voice time is used up. It frees up in {}.", wait_phrase(s)),
            None => "Today's expressive voice time is used up. Try a Standard voice.".to_owned(),
        },
        ErrorCode::EngineUnavailable => "The voice engine is waking up. This can take a minute.".to_owned(),
        ErrorCode::EngineFailed => "The voice engine couldn't read that part.".to_owned(),
        ErrorCode::UpstreamFetch => "Couldn't load that page. Check the link or paste the text instead.".to_owned(),
        ErrorCode::UpgradeRequired => "A new version is available. Reloading…".to_owned(),
        ErrorCode::Internal => "Something went wrong on the server. Try again.".to_owned(),
    }
}

/// `"about 20 seconds"`, `"3 minutes"`, `"2 hours"`.
pub fn wait_phrase(seconds: u64) -> String {
    match seconds {
        0..=59 => "under a minute".to_owned(),
        60..=3599 => {
            let m = seconds.div_ceil(60);
            if m == 1 { "a minute".to_owned() } else { format!("{m} minutes") }
        }
        _ => {
            let h = (seconds as f64 / 3600.0).round() as u64;
            if h <= 1 { "about an hour".to_owned() } else { format!("about {h} hours") }
        }
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
        assert!(e.is_unauthorized());
        assert!(!e.is_offline());
        assert!(ClientError::Api(ApiError::not_found("x")).is_not_found());
    }
}
