//! Standard Webhooks signatures, which Supabase uses for HTTP auth hooks: headers `webhook-id`, `webhook-timestamp`
//! (Unix seconds) and `webhook-signature` (`v1,<base64>`, several separated by spaces during secret rotation), the
//! signature being HMAC-SHA256 over `<id>.<timestamp>.<body>` with the key from the secret `v1,whsec_<base64>`.
//! Requests more than five minutes from now are refused so a captured request cannot be replayed later.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const TOLERANCE_S: i64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The configured secret is not `[v1,]whsec_<base64>` (a deployment error).
    Secret,
    Timestamp,
    Signature,
}

fn key(secret: &str) -> Result<Vec<u8>, Refused> {
    let encoded = secret.trim().split_once("whsec_").map(|(_, b64)| b64).ok_or(Refused::Secret)?;
    let key = STANDARD.decode(encoded.trim()).map_err(|_| Refused::Secret)?;
    if key.is_empty() { Err(Refused::Secret) } else { Ok(key) }
}

fn mac(key: &[u8], id: &str, timestamp: &str, body: &[u8]) -> Result<Hmac<Sha256>, Refused> {
    let mut m = Hmac::<Sha256>::new_from_slice(key).map_err(|_| Refused::Secret)?;
    m.update(id.as_bytes());
    m.update(b".");
    m.update(timestamp.as_bytes());
    m.update(b".");
    m.update(body);
    Ok(m)
}

/// The `webhook-signature` value a sender would write (tests, and trying the endpoint by hand).
pub fn sign(secret: &str, id: &str, timestamp: &str, body: &[u8]) -> Result<String, Refused> {
    let tag = mac(&key(secret)?, id, timestamp, body)?.finalize().into_bytes();
    Ok(format!("v1,{}", STANDARD.encode(tag)))
}

/// Accept when the timestamp is within [`TOLERANCE_S`] of `now_s` and one `v1` signature matches (compared in
/// constant time).
pub fn verify(
    secret: &str,
    id: &str,
    timestamp: &str,
    signatures: &str,
    body: &[u8],
    now_s: i64,
) -> Result<(), Refused> {
    let key = key(secret)?;
    let sent: i64 = timestamp.trim().parse().map_err(|_| Refused::Timestamp)?;
    if (now_s - sent).abs() > TOLERANCE_S {
        return Err(Refused::Timestamp);
    }
    for candidate in signatures.split_whitespace() {
        let Some(Ok(tag)) = candidate.strip_prefix("v1,").map(|b64| STANDARD.decode(b64)) else { continue };
        if mac(&key, id, timestamp, body)?.verify_slice(&tag).is_ok() {
            return Ok(());
        }
    }
    Err(Refused::Signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "v1,whsec_c2VjcmV0LWtleS1mb3ItdGVzdGluZy1vbmx5";

    #[test]
    fn signed_requests_pass() {
        let sig = sign(SECRET, "msg_1", "1000000", b"{\"a\":1}").unwrap();
        assert_eq!(verify(SECRET, "msg_1", "1000000", &sig, b"{\"a\":1}", 1_000_010), Ok(()));
        // The secret may be configured without the version prefix.
        assert_eq!(
            verify("whsec_c2VjcmV0LWtleS1mb3ItdGVzdGluZy1vbmx5", "msg_1", "1000000", &sig, b"{\"a\":1}", 1_000_000),
            Ok(())
        );
    }

    /// Known answer computed with Python hmac: HMAC-SHA256(key = "secret-key-for-testing-only", "msg_1.1000000.hello").
    #[test]
    fn matches_an_independent_computation() {
        let sig = sign(SECRET, "msg_1", "1000000", b"hello").unwrap();
        assert_eq!(sig, "v1,95IUi8gQdEz+NqbZGY842Otp9a2tfMnjBWy9WYTom+E=");
    }

    #[test]
    fn tampering_and_other_secrets_fail() {
        let sig = sign(SECRET, "m", "5", b"body").unwrap();
        assert_eq!(verify(SECRET, "m", "5", &sig, b"bodY", 5), Err(Refused::Signature));
        assert_eq!(verify(SECRET, "m2", "5", &sig, b"body", 5), Err(Refused::Signature));
        let other = sign("whsec_b3RoZXI=", "m", "5", b"body").unwrap();
        assert_eq!(verify(SECRET, "m", "5", &other, b"body", 5), Err(Refused::Signature));
        assert_eq!(verify(SECRET, "m", "5", "v2,AAAA", b"body", 5), Err(Refused::Signature));
    }

    #[test]
    fn old_future_or_malformed_timestamps_fail() {
        let sig = sign(SECRET, "m", "1000000", b"x").unwrap();
        assert_eq!(verify(SECRET, "m", "1000000", &sig, b"x", 1_000_000 + TOLERANCE_S + 1), Err(Refused::Timestamp));
        assert_eq!(verify(SECRET, "m", "1000000", &sig, b"x", 1_000_000 - TOLERANCE_S - 1), Err(Refused::Timestamp));
        assert_eq!(verify(SECRET, "m", "1000000", &sig, b"x", 1_000_000 + TOLERANCE_S), Ok(()));
        assert_eq!(verify(SECRET, "m", "soon", &sig, b"x", 1), Err(Refused::Timestamp));
    }

    #[test]
    fn one_matching_signature_among_several_is_enough() {
        let good = sign(SECRET, "m", "5", b"b").unwrap();
        assert_eq!(verify(SECRET, "m", "5", &format!("v1,AAAA v1,!!! {good}"), b"b", 5), Ok(()));
    }

    #[test]
    fn a_malformed_secret_is_a_configuration_error() {
        assert_eq!(verify("not-a-secret", "m", "5", "v1,AA", b"b", 5), Err(Refused::Secret));
        assert_eq!(verify("whsec_", "m", "5", "v1,AA", b"b", 5), Err(Refused::Secret));
    }
}
