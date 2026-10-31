//! Supabase access tokens (JWS compact form) verified without a network call: the signature against the project's
//! keys, then expiry, issuer, audience and role. New Supabase projects sign with asymmetric ES256 keys published as
//! JWKS ([`super::jwks`]); projects still on the legacy shared secret sign HS256, accepted only when the Worker has
//! `SUPABASE_JWT_SECRET`. Every other algorithm, `none` included, is refused before any key is looked at.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use hmac::{Hmac, Mac};
use p256::ecdsa::signature::Verifier as _;
use p256::ecdsa::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::Sha256;

/// Clock skew tolerated between Supabase Auth and the Worker.
pub const LEEWAY_S: i64 = 30;
/// The audience and role of a signed-in user's token (anon and service-role keys carry other roles).
pub const AUDIENCE: &str = "authenticated";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JwtError {
    #[error("malformed token")]
    Malformed,
    #[error("unsupported signing algorithm {0}")]
    UnsupportedAlgorithm(String),
    #[error("token signed with an unknown key")]
    UnknownKey,
    #[error("bad signature")]
    BadSignature,
    #[error("token expired")]
    Expired,
    #[error("token from another issuer")]
    WrongIssuer,
    #[error("token for another audience")]
    WrongAudience,
    #[error("not a signed-in user's token")]
    WrongRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Header {
    pub alg: String,
    #[serde(default)]
    pub kid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Audience {
    One(String),
    Many(Vec<String>),
}

impl Audience {
    fn contains(&self, wanted: &str) -> bool {
        match self {
            Audience::One(a) => a == wanted,
            Audience::Many(list) => list.iter().any(|a| a == wanted),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iss: String,
    pub aud: Audience,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

/// A token split into its parts, signature not yet checked.
#[derive(Debug, Clone)]
pub struct Decoded<'a> {
    pub header: Header,
    pub claims: Claims,
    /// `<header>.<payload>` as sent: what the signature covers.
    signing_input: &'a str,
    signature: Vec<u8>,
}

pub fn decode(token: &str) -> Result<Decoded<'_>, JwtError> {
    let mut parts = token.split('.');
    let (Some(h), Some(p), Some(s), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(JwtError::Malformed);
    };
    let header: Header =
        serde_json::from_slice(&B64.decode(h).map_err(|_| JwtError::Malformed)?).map_err(|_| JwtError::Malformed)?;
    let claims: Claims =
        serde_json::from_slice(&B64.decode(p).map_err(|_| JwtError::Malformed)?).map_err(|_| JwtError::Malformed)?;
    let signature = B64.decode(s).map_err(|_| JwtError::Malformed)?;
    Ok(Decoded { header, claims, signing_input: &token[..h.len() + 1 + p.len()], signature })
}

impl Decoded<'_> {
    /// ES256: the JWS signature is the raw 64-byte `r || s` (not DER).
    pub fn verify_es256(&self, key: &VerifyingKey) -> Result<(), JwtError> {
        let sig = Signature::from_slice(&self.signature).map_err(|_| JwtError::BadSignature)?;
        key.verify(self.signing_input.as_bytes(), &sig).map_err(|_| JwtError::BadSignature)
    }

    /// HS256 with the project's legacy JWT secret (used as raw bytes, as Supabase does). Constant-time compare.
    pub fn verify_hs256(&self, secret: &[u8]) -> Result<(), JwtError> {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret).map_err(|_| JwtError::BadSignature)?;
        mac.update(self.signing_input.as_bytes());
        mac.verify_slice(&self.signature).map_err(|_| JwtError::BadSignature)
    }
}

/// Expiry (with [`LEEWAY_S`]), issuer, audience, role and a non-empty subject.
pub fn check_claims(claims: &Claims, issuer: &str, now_s: i64) -> Result<(), JwtError> {
    if claims.exp + LEEWAY_S <= now_s {
        return Err(JwtError::Expired);
    }
    if claims.iss != issuer {
        return Err(JwtError::WrongIssuer);
    }
    if !claims.aud.contains(AUDIENCE) {
        return Err(JwtError::WrongAudience);
    }
    if claims.role.as_deref() != Some(AUDIENCE) || claims.sub.trim().is_empty() {
        return Err(JwtError::WrongRole);
    }
    Ok(())
}

#[cfg(test)]
pub mod testing {
    //! Token minting for tests: a fixed P-256 key (deterministic RFC 6979 signatures) and HS256.

    use super::*;
    use p256::ecdsa::SigningKey;
    use p256::ecdsa::signature::Signer as _;

    pub const ISSUER: &str = "https://ref.supabase.co/auth/v1";

    pub fn signing_key(seed: u8) -> SigningKey {
        let mut bytes = [seed; 32];
        bytes[0] = 1;
        SigningKey::from_slice(&bytes).unwrap()
    }

    pub fn claims(sub: &str, exp: i64) -> serde_json::Value {
        serde_json::json!({
            "sub": sub, "exp": exp, "iat": exp - 3600, "iss": ISSUER, "aud": "authenticated",
            "role": "authenticated", "email": "ada@example.com", "session_id": "s1"
        })
    }

    fn encode(header: &serde_json::Value, claims: &serde_json::Value) -> String {
        format!("{}.{}", B64.encode(header.to_string()), B64.encode(claims.to_string()))
    }

    pub fn es256(key: &SigningKey, kid: &str, claims: &serde_json::Value) -> String {
        let input = encode(&serde_json::json!({"alg": "ES256", "kid": kid, "typ": "JWT"}), claims);
        let sig: Signature = key.sign(input.as_bytes());
        format!("{input}.{}", B64.encode(sig.to_bytes()))
    }

    pub fn hs256(secret: &[u8], claims: &serde_json::Value) -> String {
        let input = encode(&serde_json::json!({"alg": "HS256", "typ": "JWT"}), claims);
        let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
        mac.update(input.as_bytes());
        format!("{input}.{}", B64.encode(mac.finalize().into_bytes()))
    }

    /// The JWKS JSON Supabase would publish for these keys.
    pub fn jwks(keys: &[(&SigningKey, &str)]) -> String {
        let list: Vec<serde_json::Value> = keys
            .iter()
            .map(|(k, kid)| {
                let point = k.verifying_key().to_encoded_point(false);
                serde_json::json!({
                    "kty": "EC", "crv": "P-256", "alg": "ES256", "use": "sig", "kid": kid,
                    "x": B64.encode(point.x().unwrap()), "y": B64.encode(point.y().unwrap()),
                })
            })
            .collect();
        serde_json::json!({ "keys": list }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    const NOW: i64 = 1_793_448_000;

    #[test]
    fn es256_signatures_verify_and_tampering_fails() {
        let key = signing_key(7);
        let token = es256(&key, "k1", &claims("u1", NOW + 600));
        let d = decode(&token).unwrap();
        assert_eq!((d.header.alg.as_str(), d.header.kid.as_deref()), ("ES256", Some("k1")));
        d.verify_es256(key.verifying_key()).unwrap();
        assert_eq!(d.verify_es256(signing_key(8).verifying_key()), Err(JwtError::BadSignature));
        // Swap in another payload under the same signature.
        let (h, rest) = token.split_once('.').unwrap();
        let (_, s) = rest.split_once('.').unwrap();
        let forged = format!("{h}.{}.{s}", B64.encode(claims("admin", NOW + 600).to_string()));
        assert_eq!(decode(&forged).unwrap().verify_es256(key.verifying_key()), Err(JwtError::BadSignature));
    }

    #[test]
    fn hs256_signatures_verify_with_the_right_secret_only() {
        let token = hs256(b"legacy-secret", &claims("u1", NOW + 600));
        let d = decode(&token).unwrap();
        d.verify_hs256(b"legacy-secret").unwrap();
        assert_eq!(d.verify_hs256(b"other"), Err(JwtError::BadSignature));
    }

    #[test]
    fn claims_checks() {
        let ok = |v: serde_json::Value| check_claims(&serde_json::from_value(v).unwrap(), ISSUER, NOW);
        assert_eq!(ok(claims("u1", NOW + 1)), Ok(()));
        assert_eq!(ok(claims("u1", NOW - LEEWAY_S + 1)), Ok(()), "inside the leeway");
        assert_eq!(ok(claims("u1", NOW - LEEWAY_S)), Err(JwtError::Expired));
        let mut c = claims("u1", NOW + 60);
        c["iss"] = "https://other.supabase.co/auth/v1".into();
        assert_eq!(ok(c), Err(JwtError::WrongIssuer));
        let mut c = claims("u1", NOW + 60);
        c["aud"] = "anon".into();
        assert_eq!(ok(c), Err(JwtError::WrongAudience));
        let mut c = claims("u1", NOW + 60);
        c["aud"] = serde_json::json!(["x", "authenticated"]);
        assert_eq!(ok(c), Ok(()), "audience lists are accepted");
        let mut c = claims("u1", NOW + 60);
        c["role"] = "service_role".into();
        assert_eq!(ok(c), Err(JwtError::WrongRole));
        assert_eq!(ok(claims(" ", NOW + 60)), Err(JwtError::WrongRole));
    }

    #[test]
    fn malformed_tokens() {
        for t in ["", "a.b", "a.b.c.d", "!!.e30.x", "e30.e30.@@"] {
            assert_eq!(decode(t).unwrap_err(), JwtError::Malformed, "{t}");
        }
        // Valid base64 and JSON but missing claims.
        let t = format!("{}.{}.", B64.encode(r#"{"alg":"none"}"#), B64.encode("{}"));
        assert_eq!(decode(&t).unwrap_err(), JwtError::Malformed);
    }
}
