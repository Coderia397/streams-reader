//! The session cookie: `__Host-nolabs_s=<base64url(payload)>.<base64url(hmac)>`, where the payload is the JSON of
//! [`Claims`] and the MAC is HMAC-SHA256 under `SESSION_KEY` over a domain tag plus the encoded payload. Expiry is
//! checked on the server (the cookie's Max-Age is only a hint to the browser), and `epoch` must equal the current
//! `session_epoch` row, so bumping that row revokes every session at once.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use hmac::{Hmac, Mac};
use nolabs_core::proto::api::SESSION_COOKIE;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// Sessions last 180 days from the last refresh.
pub const SESSION_TTL_MS: i64 = 180 * DAY_MS;
/// A request with less than this left gets a fresh cookie (sliding expiry).
pub const REFRESH_BELOW_MS: i64 = 30 * DAY_MS;

/// Separates cookie MACs from the passphrase hash, which uses the same key.
const COOKIE_MAC_TAG: &[u8] = b"nolabs-session-v1.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    pub sid: String,
    pub iat_ms: i64,
    pub exp_ms: i64,
    pub epoch: u64,
}

fn mac(key: &[u8]) -> HmacSha256 {
    // HMAC takes keys of any length, so `new_from_slice` cannot fail; the fallback keeps this total without a panic.
    HmacSha256::new_from_slice(key).unwrap_or_else(|_| <HmacSha256 as Mac>::new(&Default::default()))
}

/// Encode and sign the claims as a cookie value.
pub fn sign(key: &[u8], claims: &Claims) -> String {
    let payload = B64.encode(serde_json::to_vec(claims).unwrap_or_default());
    let mut m = mac(key);
    m.update(COOKIE_MAC_TAG);
    m.update(payload.as_bytes());
    let tag = B64.encode(m.finalize().into_bytes());
    format!("{payload}.{tag}")
}

/// Check the MAC (in constant time) and decode the claims. Says nothing about expiry or epoch.
pub fn verify(key: &[u8], value: &str) -> Option<Claims> {
    let (payload, tag) = value.split_once('.')?;
    let tag = B64.decode(tag).ok()?;
    let mut m = mac(key);
    m.update(COOKIE_MAC_TAG);
    m.update(payload.as_bytes());
    m.verify_slice(&tag).ok()?;
    serde_json::from_slice(&B64.decode(payload).ok()?).ok()
}

/// True when HMAC-SHA256(key, passphrase) equals `expected` (compared in constant time).
pub fn passphrase_matches(key: &[u8], passphrase: &str, expected: &[u8]) -> bool {
    let mut m = mac(key);
    m.update(passphrase.as_bytes());
    m.verify_slice(expected).is_ok()
}

/// `Set-Cookie` value that stores `value` for `max_age_s` seconds.
pub fn set_cookie(value: &str, max_age_s: i64) -> String {
    format!("{SESSION_COOKIE}={value}; Path=/; Max-Age={max_age_s}; HttpOnly; Secure; SameSite=Strict")
}

/// `Set-Cookie` value that deletes the session cookie.
pub fn clear_cookie() -> String {
    set_cookie("", 0)
}

/// The session cookie's value from a `Cookie` request header.
pub fn from_cookie_header(header: &str) -> Option<&str> {
    header
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8] = &[7; 32];

    fn claims() -> Claims {
        Claims { sid: "s1".into(), iat_ms: 1, exp_ms: 2, epoch: 1 }
    }

    #[test]
    fn signed_cookies_verify_and_round_trip() {
        let v = sign(KEY, &claims());
        assert_eq!(verify(KEY, &v), Some(claims()));
        assert!(!v.contains('=') && !v.contains('+') && !v.contains('/'));
    }

    #[test]
    fn tampering_or_another_key_fails() {
        let v = sign(KEY, &claims());
        assert_eq!(verify(&[8; 32], &v), None);
        let (payload, tag) = v.split_once('.').unwrap();
        let forged = B64.encode(br#"{"sid":"s1","iat_ms":1,"exp_ms":999999999999999,"epoch":1}"#);
        assert_eq!(verify(KEY, &format!("{forged}.{tag}")), None);
        assert_eq!(verify(KEY, payload), None);
        assert_eq!(verify(KEY, &format!("{payload}.")), None);
        assert_eq!(verify(KEY, "garbage"), None);
    }

    #[test]
    fn passphrase_hash_is_hmac_of_the_passphrase() {
        let mut m = HmacSha256::new_from_slice(KEY).unwrap();
        m.update(b"correct horse");
        let expected = m.finalize().into_bytes().to_vec();
        assert!(passphrase_matches(KEY, "correct horse", &expected));
        assert!(!passphrase_matches(KEY, "correct hors", &expected));
        assert!(!passphrase_matches(KEY, "correct horse", &expected[..31]));
    }

    #[test]
    fn cookie_attributes_and_parsing() {
        let c = set_cookie("abc", 60);
        assert_eq!(c, "__Host-nolabs_s=abc; Path=/; Max-Age=60; HttpOnly; Secure; SameSite=Strict");
        assert!(clear_cookie().contains("Max-Age=0"));
        assert_eq!(from_cookie_header("a=1; __Host-nolabs_s=x.y; b=2"), Some("x.y"));
        assert_eq!(from_cookie_header("nolabs_s=x"), None);
    }
}
