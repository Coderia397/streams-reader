//! `/api/session`: passphrase login, logout and the session check at start-up.

use nolabs_core::proto::api::{LoginRequest, SessionInfo};

use super::ClientError;
use super::client::{Body, get_json, send_no_content};

pub async fn login(passphrase: &str) -> Result<(), ClientError> {
    let body = Body::json(&LoginRequest { passphrase: passphrase.to_owned() })?;
    send_no_content("POST", "/api/session", body).await
}

pub async fn logout() -> Result<(), ClientError> {
    send_no_content("DELETE", "/api/session", Body::Empty).await
}

pub async fn info() -> Result<SessionInfo, ClientError> {
    get_json("/api/session").await
}
