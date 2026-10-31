//! Audio URLs. Media elements and plain fetches of clips cannot rely on `Authorization`, so the Worker hands out a
//! short-lived audio token (`POST /api/audio-token`, D22) that rides on the URL as `?t=`. One token serves every
//! clip; it is minted again a few minutes before it expires, or once when the Worker refuses it.

use std::cell::RefCell;

use nolabs_core::proto::api::{AUDIO_TOKEN_PARAM, AudioToken};

use super::ClientError;
use super::client::{Body, encode_component, encode_segment, read_json, send};
use crate::util::timer::now_ms;

/// Mint a new token when the current one has less than this left.
const RENEW_BEFORE_MS: i64 = 5 * 60_000;

thread_local! {
    static TOKEN: RefCell<Option<AudioToken>> = const { RefCell::new(None) };
}

/// The cache key and path of a clip, without a token.
pub fn clip_path(key: &str) -> String {
    format!("/api/audio/{}", encode_segment(key))
}

/// Whether `token` is still good for at least [`RENEW_BEFORE_MS`] at `now`.
pub fn fresh(token: &AudioToken, now: i64) -> bool {
    token.expires_at_ms - now > RENEW_BEFORE_MS
}

async fn mint() -> Result<AudioToken, ClientError> {
    let resp = send("POST", "/api/audio-token", Body::Empty).await?;
    let token: AudioToken = read_json(&resp).await?;
    TOKEN.with(|t| *t.borrow_mut() = Some(token.clone()));
    Ok(token)
}

async fn token() -> Result<String, ClientError> {
    let current = TOKEN.with(|t| t.borrow().clone());
    match current {
        Some(t) if fresh(&t, now_ms()) => Ok(t.token),
        _ => Ok(mint().await?.token),
    }
}

/// Forgets the token (sign-out, or the Worker refused it).
pub fn forget() {
    TOKEN.with(|t| *t.borrow_mut() = None);
}

/// A playable URL for the clip.
pub async fn url(key: &str) -> Result<String, ClientError> {
    Ok(format!("{}?{AUDIO_TOKEN_PARAM}={}", clip_path(key), encode_component(&token().await?)))
}

/// The clip's response; a refused token is replaced once.
pub async fn fetch(key: &str) -> Result<web_sys::Response, ClientError> {
    match send("GET", &url(key).await?, Body::Empty).await {
        Err(ClientError::Api(e))
            if e.code == nolabs_core::proto::ErrorCode::Unauthorized
                || e.code == nolabs_core::proto::ErrorCode::Forbidden =>
        {
            forget();
            send("GET", &url(key).await?, Body::Empty).await
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_renewed_before_they_lapse() {
        let t = AudioToken { token: "a1.u.1.x".into(), expires_at_ms: 1_000_000 };
        assert!(fresh(&t, 1_000_000 - RENEW_BEFORE_MS - 1));
        assert!(!fresh(&t, 1_000_000 - RENEW_BEFORE_MS));
        assert_eq!(clip_path("ab:cd"), "/api/audio/ab:cd");
    }
}
