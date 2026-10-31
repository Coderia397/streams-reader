//! Who is calling: Supabase access tokens (`Authorization: Bearer …`) verified in the Worker, and the audio URL
//! tokens minted from them. There is no login route: the apps sign in with Supabase (email code, magic link or OAuth)
//! and send the access token supabase-js keeps fresh. Revocation follows Supabase: a signed-out session's access
//! token stays valid until it expires (one hour, `jwt_exp`), as for every Supabase API.

pub mod audio_token;
pub mod guard;
pub mod jwks;
pub mod jwt;

use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::{AudioToken, SessionInfo};
use p256::ecdsa::VerifyingKey;

use crate::app::App;
use crate::error::unauthorized;
use crate::ports::{HttpRequest, Method};
use jwks::{KeySet, Lookup};

const JWKS_TIMEOUT_MS: u32 = 5_000;
const JWKS_MAX_BYTES: u64 = 64 * 1024;

/// A verified caller.
#[derive(Clone, PartialEq, Eq)]
pub struct AuthUser {
    /// Supabase user id (`sub`, `auth.uid()`).
    pub id: String,
    pub email: Option<String>,
    pub expires_at_ms: i64,
    /// The access token, for PostgREST calls on the user's behalf (RLS decides). None for audio-token callers.
    pub access_token: Option<String>,
}

impl std::fmt::Debug for AuthUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthUser")
            .field("id", &self.id)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish_non_exhaustive()
    }
}

impl AuthUser {
    pub fn info(&self) -> SessionInfo {
        SessionInfo { user_id: self.id.clone(), email: self.email.clone(), expires_at_ms: self.expires_at_ms }
    }

    /// The token for PostgREST, or 401 when the caller authenticated with an audio token.
    pub fn token(&self) -> Result<&str, ApiError> {
        self.access_token.as_deref().ok_or_else(|| unauthorized("this route needs an access token"))
    }
}

/// The token of an `Authorization: Bearer <token>` header (scheme case-insensitive).
pub fn bearer(header: Option<&str>) -> Option<&str> {
    let (scheme, token) = header?.trim().split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

impl App {
    /// Verify the caller's Supabase access token: signature (ES256 from JWKS, or HS256 with the legacy secret),
    /// then expiry, issuer, audience and role.
    pub async fn authenticate(&self, authorization: Option<&str>) -> Result<AuthUser, ApiError> {
        let token = bearer(authorization).ok_or_else(|| unauthorized("sign in first"))?;
        let decoded = jwt::decode(token).map_err(|e| unauthorized(e.to_string()))?;
        match decoded.header.alg.as_str() {
            "ES256" => {
                let kid = decoded.header.kid.as_deref().ok_or_else(|| unauthorized("token has no key id"))?;
                let key = self.signing_key(kid).await?;
                decoded.verify_es256(&key)
            }
            "HS256" => match &self.config.supabase_jwt_secret {
                Some(secret) => decoded.verify_hs256(secret),
                None => Err(jwt::JwtError::UnsupportedAlgorithm("HS256".into())),
            },
            other => Err(jwt::JwtError::UnsupportedAlgorithm(other.to_owned())),
        }
        .map_err(|e| unauthorized(e.to_string()))?;
        let now_s = self.now_ms().div_euclid(1000);
        jwt::check_claims(&decoded.claims, &self.config.jwt_issuer(), now_s)
            .map_err(|e| unauthorized(e.to_string()))?;
        let claims = decoded.claims;
        Ok(AuthUser {
            id: claims.sub,
            email: claims.email,
            expires_at_ms: claims.exp.saturating_mul(1000),
            access_token: Some(token.to_owned()),
        })
    }

    /// The caller of an audio URL: an access token if the request has one, else the `?t=` audio token.
    pub async fn authenticate_audio(
        &self,
        authorization: Option<&str>,
        audio_token: Option<&str>,
    ) -> Result<AuthUser, ApiError> {
        if authorization.is_some() {
            return self.authenticate(authorization).await;
        }
        let token = audio_token.ok_or_else(|| unauthorized("sign in first"))?;
        let now_s = self.now_ms().div_euclid(1000);
        let id = audio_token::verify(&self.config.audio_url_key, token, now_s)
            .ok_or_else(|| unauthorized("audio link expired; ask for a new one"))?;
        Ok(AuthUser { id, email: None, expires_at_ms: now_s * 1000, access_token: None })
    }

    /// `POST /api/audio-token`.
    pub fn mint_audio_token(&self, user: &AuthUser) -> Result<AudioToken, ApiError> {
        let now_s = self.now_ms().div_euclid(1000);
        let (token, exp_s) = audio_token::mint(&self.config.audio_url_key, &user.id, now_s)
            .ok_or_else(|| ApiError::internal("cannot mint an audio token"))?;
        Ok(AudioToken { token, expires_at_ms: exp_s * 1000 })
    }

    /// The verification key for `kid`, refreshing the isolate's JWKS cache when needed.
    async fn signing_key(&self, kid: &str) -> Result<VerifyingKey, ApiError> {
        let now = self.now_ms();
        if let Lookup::Found(key) = self.jwks.lookup(kid, now) {
            return Ok(key);
        }
        if self.jwks.lookup(kid, now) == Lookup::Fetch {
            self.jwks.attempted(now);
            match self.fetch_jwks().await {
                Ok(keys) => self.jwks.store(keys, now),
                Err(e) => return self.stale_or(kid, now, e),
            }
        }
        match self.jwks.lookup(kid, now) {
            Lookup::Found(key) => Ok(key),
            _ => Err(unauthorized("token signed with an unknown key")),
        }
    }

    fn stale_or(&self, kid: &str, now: i64, error: ApiError) -> Result<VerifyingKey, ApiError> {
        match self.jwks.lookup(kid, now) {
            Lookup::Found(key) => Ok(key),
            _ => Err(error),
        }
    }

    async fn fetch_jwks(&self) -> Result<KeySet, ApiError> {
        let req = HttpRequest::new(Method::Get, self.config.jwks_url())
            .header("accept", "application/json")
            .timeout_ms(JWKS_TIMEOUT_MS)
            .max_body_bytes(JWKS_MAX_BYTES);
        let resp =
            self.http.send(req).await.map_err(|e| ApiError::internal(format!("cannot load signing keys: {e}")))?;
        if resp.status != 200 {
            return Err(ApiError::internal(format!("signing keys answered HTTP {}", resp.status)));
        }
        let keys = KeySet::parse(&resp.body).map_err(ApiError::internal)?;
        if keys.is_empty() {
            return Err(ApiError::internal("the project publishes no ES256 signing key"));
        }
        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::jwt::testing::{claims, es256, hs256, jwks, signing_key};
    use super::*;
    use crate::testing::fakes::response;
    use crate::testing::{TestEnv, run};
    use nolabs_core::proto::ErrorCode;

    fn env_with_keys(keys: &[(u8, &str)]) -> TestEnv {
        let env = TestEnv::new();
        env.set_jwks(keys);
        env
    }

    fn header(token: &str) -> String {
        format!("Bearer {token}")
    }

    #[test]
    fn bearer_header_parsing() {
        assert_eq!(bearer(Some("Bearer abc")), Some("abc"));
        assert_eq!(bearer(Some("bearer  abc ")), Some("abc"));
        assert_eq!(bearer(Some("Basic abc")), None);
        assert_eq!(bearer(Some("Bearer ")), None);
        assert_eq!(bearer(None), None);
    }

    #[test]
    fn a_valid_es256_token_authenticates_and_the_keys_are_cached() {
        let env = env_with_keys(&[(1, "k1")]);
        let exp = env.now() / 1000 + 600;
        let token = es256(&signing_key(1), "k1", &claims("user-1", exp));
        run(async {
            let u = env.app.authenticate(Some(&header(&token))).await.unwrap();
            assert_eq!(
                (u.id.as_str(), u.email.as_deref(), u.expires_at_ms),
                ("user-1", Some("ada@example.com"), exp * 1000)
            );
            assert_eq!(u.info().user_id, "user-1");
            assert!(!format!("{u:?}").contains(&token), "Debug never shows the token");
            env.app.authenticate(Some(&header(&token))).await.unwrap();
        });
        assert_eq!(env.jwks_fetches(), 1, "second request served from the cache");
    }

    #[test]
    fn expired_forged_and_foreign_tokens_are_401() {
        let env = env_with_keys(&[(1, "k1")]);
        let now_s = env.now() / 1000;
        let key = signing_key(1);
        let mut wrong_iss = claims("u", now_s + 600);
        wrong_iss["iss"] = "https://evil.supabase.co/auth/v1".into();
        let mut wrong_aud = claims("u", now_s + 600);
        wrong_aud["aud"] = "anon".into();
        let cases = [
            es256(&key, "k1", &claims("u", now_s - 3600)),
            es256(&signing_key(2), "k1", &claims("u", now_s + 600)),
            es256(&key, "k1", &wrong_iss),
            es256(&key, "k1", &wrong_aud),
            hs256(b"secret", &claims("u", now_s + 600)),
            "not-a-token".to_owned(),
        ];
        run(async {
            for t in &cases {
                let e = env.app.authenticate(Some(&header(t))).await.unwrap_err();
                assert_eq!(e.code, ErrorCode::Unauthorized, "{t}");
            }
            assert_eq!(env.app.authenticate(None).await.unwrap_err().code, ErrorCode::Unauthorized);
        });
    }

    #[test]
    fn alg_none_is_refused() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let env = env_with_keys(&[(1, "k1")]);
        let c = claims("u", env.now() / 1000 + 600);
        let t = format!("{}.{}.", b64.encode(r#"{"alg":"none"}"#), b64.encode(c.to_string()));
        let e = run(env.app.authenticate(Some(&header(&t)))).unwrap_err();
        assert!(e.message.contains("unsupported"), "{}", e.message);
    }

    #[test]
    fn hs256_works_only_with_the_legacy_secret_configured() {
        let mut env = env_with_keys(&[(1, "k1")]);
        let t = hs256(b"legacy", &claims("u2", env.now() / 1000 + 600));
        assert!(run(env.app.authenticate(Some(&header(&t)))).is_err());
        env.app.config.supabase_jwt_secret = Some(b"legacy".to_vec());
        assert_eq!(run(env.app.authenticate(Some(&header(&t)))).unwrap().id, "u2");
    }

    #[test]
    fn key_rotation_refetches_once_for_a_new_kid() {
        let env = env_with_keys(&[(1, "old")]);
        let exp = env.now() / 1000 + 600;
        run(env.app.authenticate(Some(&header(&es256(&signing_key(1), "old", &claims("u", exp)))))).unwrap();
        // Supabase rotates: the JWKS now also holds the new key.
        env.set_jwks(&[(1, "old"), (2, "new")]);
        env.advance(jwks::REFRESH_MIN_MS);
        let fresh = es256(&signing_key(2), "new", &claims("u", exp));
        assert_eq!(run(env.app.authenticate(Some(&header(&fresh)))).unwrap().id, "u");
        assert_eq!(env.jwks_fetches(), 2);
        // An unknown kid right away does not refetch again.
        let bogus = es256(&signing_key(3), "bogus", &claims("u", exp));
        assert!(run(env.app.authenticate(Some(&header(&bogus)))).is_err());
        assert_eq!(env.jwks_fetches(), 2);
    }

    #[test]
    fn a_jwks_outage_keeps_cached_keys_working() {
        let env = env_with_keys(&[(1, "k1")]);
        let exp = env.now() / 1000 + 3 * 3600;
        let t = es256(&signing_key(1), "k1", &claims("u", exp));
        run(env.app.authenticate(Some(&header(&t)))).unwrap();
        env.http.set(|_| Ok(response(503, &[], "down")));
        env.advance(jwks::TTL_MS + 1);
        assert_eq!(run(env.app.authenticate(Some(&header(&t)))).unwrap().id, "u");
    }

    #[test]
    fn audio_tokens_round_trip_and_audio_accepts_either_credential() {
        let env = env_with_keys(&[(1, "k1")]);
        let exp = env.now() / 1000 + 600;
        let bearer_header = header(&es256(&signing_key(1), "k1", &claims("u3", exp)));
        run(async {
            let user = env.app.authenticate(Some(&bearer_header)).await.unwrap();
            let t = env.app.mint_audio_token(&user).unwrap();
            assert_eq!(t.expires_at_ms, env.now() + audio_token::TTL_S * 1000);
            let via_t = env.app.authenticate_audio(None, Some(&t.token)).await.unwrap();
            assert_eq!(via_t.id, "u3");
            assert_eq!(via_t.token().unwrap_err().code, ErrorCode::Unauthorized);
            assert_eq!(env.app.authenticate_audio(Some(&bearer_header), None).await.unwrap().id, "u3");
            assert!(env.app.authenticate_audio(None, Some("a1.u3.1.AAAA")).await.is_err());
            assert!(env.app.authenticate_audio(None, None).await.is_err());
            env.advance(audio_token::TTL_S * 1000);
            assert!(env.app.authenticate_audio(None, Some(&t.token)).await.is_err());
        });
    }

    #[test]
    fn jwks_published_keys_are_used_as_given() {
        let json = jwks(&[(&signing_key(9), "k9")]);
        assert_eq!(KeySet::parse(json.as_bytes()).unwrap().len(), 1);
    }
}
