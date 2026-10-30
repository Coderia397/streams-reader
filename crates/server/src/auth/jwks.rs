//! The project's signing keys (`/auth/v1/.well-known/jwks.json`) cached in the Worker isolate's memory. Keys are
//! fetched on first use and again after [`TTL_MS`]. A token whose `kid` is not in the cache triggers an early refetch
//! (Supabase rotated its key), at most once per [`REFRESH_MIN_MS`] so a flood of forged `kid`s cannot turn every
//! request into a fetch. Only P-256 (`ES256`) keys are kept; others are skipped.

use std::cell::RefCell;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use p256::ecdsa::VerifyingKey;
use p256::{EncodedPoint, FieldBytes};
use serde::Deserialize;

pub const TTL_MS: i64 = 10 * 60 * 1000;
pub const REFRESH_MIN_MS: i64 = 30 * 1000;

#[derive(Debug, Deserialize)]
struct Jwk {
    #[serde(default)]
    kty: String,
    #[serde(default)]
    crv: String,
    #[serde(default)]
    kid: Option<String>,
    #[serde(default)]
    x: Option<String>,
    #[serde(default)]
    y: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

/// Verification keys by `kid`.
#[derive(Debug, Clone, Default)]
pub struct KeySet {
    keys: Vec<(String, VerifyingKey)>,
}

impl KeySet {
    /// Parse a JWKS document. Keys that are not EC P-256 or have no `kid` are skipped, not errors.
    pub fn parse(json: &[u8]) -> Result<KeySet, String> {
        let set: JwkSet = serde_json::from_slice(json).map_err(|e| format!("JWKS is not valid JSON: {e}"))?;
        let mut keys = Vec::new();
        for jwk in set.keys {
            if jwk.kty != "EC" || jwk.crv != "P-256" {
                continue;
            }
            let (Some(kid), Some(x), Some(y)) = (jwk.kid, jwk.x, jwk.y) else { continue };
            let coord = |s: &str| -> Option<FieldBytes> {
                let bytes = B64.decode(s).ok()?;
                (bytes.len() == 32).then(|| FieldBytes::clone_from_slice(&bytes))
            };
            let (Some(x), Some(y)) = (coord(&x), coord(&y)) else { continue };
            let point = EncodedPoint::from_affine_coordinates(&x, &y, false);
            if let Ok(key) = VerifyingKey::from_encoded_point(&point) {
                keys.push((kid, key));
            }
        }
        Ok(KeySet { keys })
    }

    pub fn get(&self, kid: &str) -> Option<&VerifyingKey> {
        self.keys.iter().find(|(k, _)| k == kid).map(|(_, key)| key)
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

#[derive(Debug, Default)]
struct State {
    keys: Option<KeySet>,
    fetched_at_ms: i64,
    last_attempt_ms: Option<i64>,
}

/// Shared by every request an isolate serves (the Worker keeps one in a thread-local). Not `Sync`: the Workers runtime
/// is single-threaded.
#[derive(Debug, Default)]
pub struct JwksCache {
    state: RefCell<State>,
}

/// What the caller should do for a `kid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Found(VerifyingKey),
    /// Fetch the JWKS and [`JwksCache::store`] it, then look up again.
    Fetch,
    /// Not known, and a refetch happened too recently.
    Unknown,
}

impl JwksCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// A fresh cached key; else a fetch when none happened in the last [`REFRESH_MIN_MS`]; else the stale key (a
    /// failing JWKS endpoint must not sign everyone out, nor be hit on every request); else unknown.
    pub fn lookup(&self, kid: &str, now_ms: i64) -> Lookup {
        let state = self.state.borrow();
        let cached = state.keys.as_ref().and_then(|k| k.get(kid)).copied();
        let fresh = state.keys.is_some() && now_ms - state.fetched_at_ms < TTL_MS;
        if let (true, Some(key)) = (fresh, cached) {
            return Lookup::Found(key);
        }
        if state.last_attempt_ms.is_none_or(|t| now_ms - t >= REFRESH_MIN_MS) {
            return Lookup::Fetch;
        }
        cached.map_or(Lookup::Unknown, Lookup::Found)
    }

    /// Record a fetch attempt (successful or not) so refetches stay rate-limited.
    pub fn attempted(&self, now_ms: i64) {
        self.state.borrow_mut().last_attempt_ms = Some(now_ms);
    }

    pub fn store(&self, keys: KeySet, now_ms: i64) {
        let mut state = self.state.borrow_mut();
        state.keys = Some(keys);
        state.fetched_at_ms = now_ms;
        state.last_attempt_ms = Some(now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::testing::{jwks, signing_key};

    #[test]
    fn parses_p256_keys_and_skips_others() {
        let a = signing_key(1);
        let json = jwks(&[(&a, "a")]);
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v["keys"].as_array_mut().unwrap().push(serde_json::json!({"kty": "RSA", "kid": "r", "n": "x", "e": "AQAB"}));
        v["keys"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"kty": "EC", "crv": "P-256", "kid": "bad", "x": "AA", "y": "AA"}));
        let set = KeySet::parse(v.to_string().as_bytes()).unwrap();
        assert_eq!(set.len(), 1);
        assert_eq!(set.get("a"), Some(a.verifying_key()));
        assert!(KeySet::parse(b"nope").is_err());
    }

    #[test]
    fn the_real_project_key_parses() {
        // Published by project atxbmgfuxjtcwmoaldqy on 2026-10-30 (public key).
        let json = br#"{"keys":[{"alg":"ES256","crv":"P-256","ext":true,"key_ops":["verify"],"kid":"00e9aeae-028f-44eb-a7b4-155e964e569c","kty":"EC","use":"sig","x":"UsTl-xdIAXtDLD3rtOgZZ1iSplvhqQWq8IStIBUEWtU","y":"jwI-a_Q0dAh8y63YT5x7WYMZX_WA2tNgnPNMwjUa8jI"}]}"#;
        assert!(KeySet::parse(json).unwrap().get("00e9aeae-028f-44eb-a7b4-155e964e569c").is_some());
    }

    #[test]
    fn cache_ttl_and_rotation_refetch_are_rate_limited() {
        let cache = JwksCache::new();
        let a = signing_key(1);
        assert_eq!(cache.lookup("a", 0), Lookup::Fetch, "empty cache fetches");
        cache.store(KeySet::parse(jwks(&[(&a, "a")]).as_bytes()).unwrap(), 0);
        assert_eq!(cache.lookup("a", 1000), Lookup::Found(*a.verifying_key()));
        // A new kid right after a fetch: no refetch storm.
        assert_eq!(cache.lookup("b", 1000), Lookup::Unknown);
        assert_eq!(cache.lookup("b", REFRESH_MIN_MS), Lookup::Fetch);
        // After the TTL even a known kid refetches; the stale key is still available meanwhile.
        assert_eq!(cache.lookup("a", TTL_MS), Lookup::Fetch);
        // The fetch failed: the stale key serves until the next allowed attempt.
        cache.attempted(TTL_MS);
        assert_eq!(cache.lookup("a", TTL_MS + 1), Lookup::Found(*a.verifying_key()));
        assert_eq!(cache.lookup("a", TTL_MS + REFRESH_MIN_MS), Lookup::Fetch);
    }
}
