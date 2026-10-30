//! Login, session checks and logout for the single user, plus the request guards every API route goes through.

pub mod guard;
pub mod session;

use nolabs_core::proto::api::{LoginRequest, SessionInfo};
use nolabs_core::proto::{ApiError, ErrorCode};

use crate::app::App;
use crate::error::unauthorized;
use session::{Claims, REFRESH_BELOW_MS, SESSION_TTL_MS};

/// Failed logins allowed per rolling hour before every attempt is refused.
pub const MAX_FAILURES_PER_HOUR: u32 = 20;
const HOUR_MS: i64 = 60 * 60 * 1000;
const MAX_PASSPHRASE_BYTES: usize = 1024;

/// A request's verified session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSession {
    pub claims: Claims,
    /// A `Set-Cookie` value to attach to the response when the session was refreshed.
    pub refreshed_cookie: Option<String>,
}

impl AuthSession {
    pub fn info(&self) -> SessionInfo {
        SessionInfo { expires_at_ms: self.claims.exp_ms }
    }
}

impl App {
    /// Check the passphrase and return the `Set-Cookie` value of a new session.
    pub async fn login(&self, req: &LoginRequest) -> Result<String, ApiError> {
        let now = self.now_ms();
        let repo = self.repo();
        let (failures, oldest) = repo.auth_failures_since(now - HOUR_MS).await?;
        if failures >= MAX_FAILURES_PER_HOUR {
            let wait_ms = oldest.map_or(HOUR_MS, |t| t + HOUR_MS - now);
            let wait_s = u64::try_from(wait_ms.max(1000)).unwrap_or(3600).div_ceil(1000);
            return Err(ApiError::new(ErrorCode::RateLimited, "too many failed attempts; try again later")
                .with_retry_after(wait_s));
        }
        let ok = req.passphrase.len() <= MAX_PASSPHRASE_BYTES
            && session::passphrase_matches(&self.config.session_key, &req.passphrase, &self.config.passphrase_hash);
        if !ok {
            repo.record_auth_failure(now).await?;
            return Err(unauthorized("wrong passphrase"));
        }
        repo.clear_auth_failures().await?;
        let claims = Claims {
            sid: self.new_id()?,
            iat_ms: now,
            exp_ms: now + SESSION_TTL_MS,
            epoch: repo.session_epoch().await?,
        };
        Ok(session::set_cookie(&session::sign(&self.config.session_key, &claims), SESSION_TTL_MS / 1000))
    }

    /// Verify the session cookie from a `Cookie` header: MAC, server-side expiry and revocation epoch. Sessions with
    /// under 30 days left come back with a refreshed cookie.
    pub async fn authenticate(&self, cookie_header: Option<&str>) -> Result<AuthSession, ApiError> {
        let value = cookie_header.and_then(session::from_cookie_header).ok_or_else(|| unauthorized("not logged in"))?;
        let claims = session::verify(&self.config.session_key, value).ok_or_else(|| unauthorized("invalid session"))?;
        let now = self.now_ms();
        if claims.exp_ms <= now {
            return Err(unauthorized("session expired"));
        }
        if claims.epoch != self.repo().session_epoch().await? {
            return Err(unauthorized("session revoked"));
        }
        if claims.exp_ms - now >= REFRESH_BELOW_MS {
            return Ok(AuthSession { claims, refreshed_cookie: None });
        }
        let claims = Claims { iat_ms: now, exp_ms: now + SESSION_TTL_MS, ..claims };
        let cookie = session::set_cookie(&session::sign(&self.config.session_key, &claims), SESSION_TTL_MS / 1000);
        Ok(AuthSession { claims, refreshed_cookie: Some(cookie) })
    }

    /// Log out: when the caller holds a valid session, bump the epoch so that cookie (and every other copy of a
    /// session) stops working on the server, not just in this browser. Always returns the cookie-clearing header.
    pub async fn logout(&self, cookie_header: Option<&str>) -> Result<String, ApiError> {
        if self.authenticate(cookie_header).await.is_ok() {
            self.repo().bump_session_epoch(self.now_ms()).await?;
        }
        Ok(session::clear_cookie())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestEnv, passphrase_hash};

    fn login_req(p: &str) -> LoginRequest {
        LoginRequest { passphrase: p.into() }
    }

    fn cookie_value(set_cookie: &str) -> String {
        let v = set_cookie.split(';').next().unwrap();
        v.to_owned()
    }

    #[test]
    fn login_issues_a_cookie_that_authenticates() {
        let env = TestEnv::new();
        futures::executor::block_on(async {
            let set = env.app.login(&login_req("secret words")).await.unwrap();
            assert!(set.starts_with("__Host-nolabs_s=") && set.contains("HttpOnly; Secure; SameSite=Strict"));
            assert!(set.contains("Max-Age=15552000"));
            let s = env.app.authenticate(Some(&cookie_value(&set))).await.unwrap();
            assert_eq!(s.claims.exp_ms, env.now() + SESSION_TTL_MS);
            assert_eq!(s.refreshed_cookie, None);
            assert_eq!(s.info().expires_at_ms, s.claims.exp_ms);
        });
        assert_eq!(passphrase_hash("x").len(), 32);
    }

    #[test]
    fn wrong_passphrase_is_401_and_counted() {
        let env = TestEnv::new();
        futures::executor::block_on(async {
            let e = env.app.login(&login_req("nope")).await.unwrap_err();
            assert_eq!(e.code, ErrorCode::Unauthorized);
            assert_eq!(env.app.repo().auth_failures_since(0).await.unwrap().0, 1);
            // Success clears the counter.
            env.app.login(&login_req("secret words")).await.unwrap();
            assert_eq!(env.app.repo().auth_failures_since(0).await.unwrap().0, 0);
        });
    }

    #[test]
    fn twenty_failures_in_an_hour_lock_out_even_the_right_passphrase() {
        let env = TestEnv::new();
        futures::executor::block_on(async {
            for _ in 0..MAX_FAILURES_PER_HOUR {
                env.advance(1_000);
                env.app.login(&login_req("nope")).await.unwrap_err();
            }
            let e = env.app.login(&login_req("secret words")).await.unwrap_err();
            assert_eq!(e.code, ErrorCode::RateLimited);
            assert_eq!(e.status(), 429);
            // The oldest failure leaves the window 59 m 41 s from now.
            assert_eq!(e.retry_after_s, Some(3600 - 19));
            env.advance(HOUR_MS - 19_000);
            env.app.login(&login_req("secret words")).await.unwrap();
        });
    }

    #[test]
    fn expired_sessions_are_rejected_on_the_server() {
        let env = TestEnv::new();
        futures::executor::block_on(async {
            let set = env.app.login(&login_req("secret words")).await.unwrap();
            env.advance(SESSION_TTL_MS);
            let e = env.app.authenticate(Some(&cookie_value(&set))).await.unwrap_err();
            assert_eq!(e.code, ErrorCode::Unauthorized);
        });
    }

    #[test]
    fn sessions_near_expiry_are_refreshed() {
        let env = TestEnv::new();
        futures::executor::block_on(async {
            let set = env.app.login(&login_req("secret words")).await.unwrap();
            env.advance(SESSION_TTL_MS - REFRESH_BELOW_MS - 1);
            let s = env.app.authenticate(Some(&cookie_value(&set))).await.unwrap();
            assert!(s.refreshed_cookie.is_none());
            env.advance(2);
            let s = env.app.authenticate(Some(&cookie_value(&set))).await.unwrap();
            let fresh = s.refreshed_cookie.unwrap();
            assert_eq!(s.claims.exp_ms, env.now() + SESSION_TTL_MS);
            let again = env.app.authenticate(Some(&cookie_value(&fresh))).await.unwrap();
            assert_eq!(again.claims.sid, s.claims.sid);
        });
    }

    #[test]
    fn logout_bumps_the_epoch_and_revokes_every_session() {
        let env = TestEnv::new();
        futures::executor::block_on(async {
            let a = cookie_value(&env.app.login(&login_req("secret words")).await.unwrap());
            let b = cookie_value(&env.app.login(&login_req("secret words")).await.unwrap());
            let cleared = env.app.logout(Some(&a)).await.unwrap();
            assert!(cleared.contains("Max-Age=0"));
            assert_eq!(env.app.authenticate(Some(&a)).await.unwrap_err().message, "session revoked");
            assert!(env.app.authenticate(Some(&b)).await.is_err());
            // A new login works under the new epoch; logging out without a session changes nothing.
            let c = cookie_value(&env.app.login(&login_req("secret words")).await.unwrap());
            env.app.logout(None).await.unwrap();
            assert!(env.app.authenticate(Some(&c)).await.is_ok());
        });
    }

    #[test]
    fn missing_or_forged_cookies_are_401() {
        let env = TestEnv::new();
        futures::executor::block_on(async {
            assert_eq!(env.app.authenticate(None).await.unwrap_err().code, ErrorCode::Unauthorized);
            assert!(env.app.authenticate(Some("__Host-nolabs_s=a.b")).await.is_err());
            let forged = session::sign(&[9; 32], &Claims { sid: "x".into(), iat_ms: 0, exp_ms: i64::MAX, epoch: 1 });
            assert!(env.app.authenticate(Some(&format!("__Host-nolabs_s={forged}"))).await.is_err());
        });
    }
}
