//! Conversions into the API error body. Services return [`ApiError`] directly; adapter failures become 500s.

use nolabs_core::proto::{ApiError, ErrorCode};

use crate::ports::PortError;

impl From<PortError> for ApiError {
    fn from(e: PortError) -> Self {
        ApiError::internal(format!("storage error: {e}"))
    }
}

pub fn conflict(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::Conflict, message)
}

pub fn forbidden(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::Forbidden, message)
}

pub fn unauthorized(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::Unauthorized, message)
}

pub fn too_large(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::PayloadTooLarge, message)
}

/// Parse a JSON request body into `T`, answering 400 with serde's message on failure.
pub fn parse_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, ApiError> {
    serde_json::from_slice(body).map_err(|e| ApiError::bad_request(format!("invalid JSON body: {e}")))
}

/// Serialize to JSON for storage; failure here is a bug, reported as a 500.
pub fn to_json<T: serde::Serialize>(value: &T) -> Result<String, ApiError> {
    serde_json::to_string(value).map_err(|e| ApiError::internal(format!("serialize: {e}")))
}
