//! Supabase Auth errors, sorted into what the sign-in screen does next. GoTrue answers either
//! `{"code":400,"error_code":"otp_expired","msg":"…"}` or `{"error":"invalid_grant","error_description":"…"}`.

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthErrorKind {
    /// No answer (offline, DNS, blocked).
    Network,
    /// Wrong or expired 6-digit code.
    BadCode,
    /// No account has this email yet (sign-in without sign-up): ask for an invite.
    NoAccount,
    /// Sign-up needs an invite code (refused by the sign-up hook).
    InviteRequired,
    /// The invite code given was not accepted.
    BadInvite,
    RateLimited,
    /// The refresh token is no longer valid: sign in again.
    SessionGone,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct AuthError {
    pub kind: AuthErrorKind,
    pub status: u16,
    pub message: String,
}

#[derive(Deserialize, Default)]
struct Body {
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    msg: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

pub fn classify(status: u16, code: &str, message: &str) -> AuthErrorKind {
    let m = message.to_ascii_lowercase();
    let c = code.to_ascii_lowercase();
    if c.contains("invite") || m.contains("invite") {
        return if m.contains("invalid") || m.contains("not valid") || m.contains("used") || c.contains("invalid") {
            AuthErrorKind::BadInvite
        } else {
            AuthErrorKind::InviteRequired
        };
    }
    if status == 429 || c.contains("rate_limit") {
        return AuthErrorKind::RateLimited;
    }
    if c == "otp_disabled" || m.contains("signups not allowed") {
        return AuthErrorKind::NoAccount;
    }
    if c == "otp_expired" || m.contains("token has expired or is invalid") {
        return AuthErrorKind::BadCode;
    }
    if c == "refresh_token_not_found"
        || c == "refresh_token_already_used"
        || c == "session_not_found"
        || (c == "invalid_grant" && m.contains("refresh"))
    {
        return AuthErrorKind::SessionGone;
    }
    AuthErrorKind::Other
}

impl AuthError {
    pub fn network(detail: &str) -> AuthError {
        AuthError { kind: AuthErrorKind::Network, status: 0, message: detail.to_owned() }
    }

    pub fn from_response(status: u16, text: &str) -> AuthError {
        let body: Body = serde_json::from_str(text).unwrap_or_default();
        let code = body.error_code.or(body.error).unwrap_or_default();
        let message = body.msg.or(body.message).or(body.error_description).unwrap_or_else(|| format!("HTTP {status}"));
        AuthError { kind: classify(status, &code, &message), status, message }
    }

    /// One calm sentence for the screen.
    pub fn friendly(&self) -> String {
        match self.kind {
            AuthErrorKind::Network => t!("auth-error-offline"),
            AuthErrorKind::BadCode => t!("auth-error-code"),
            AuthErrorKind::InviteRequired | AuthErrorKind::NoAccount => t!("auth-invite-needed"),
            AuthErrorKind::BadInvite => t!("auth-error-invite"),
            AuthErrorKind::RateLimited => t!("auth-error-rate"),
            AuthErrorKind::SessionGone => t!("auth-error-session"),
            AuthErrorKind::Other => t!("auth-error-other"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_gotrue_answers() {
        let e = AuthError::from_response(
            403,
            r#"{"code":403,"error_code":"otp_expired","msg":"Token has expired or is invalid"}"#,
        );
        assert_eq!(e.kind, AuthErrorKind::BadCode);
        let e = AuthError::from_response(
            400,
            r#"{"error":"invalid_grant","error_description":"Invalid Refresh Token: Refresh Token Not Found"}"#,
        );
        assert_eq!(e.kind, AuthErrorKind::SessionGone);
        let e = AuthError::from_response(
            422,
            r#"{"code":422,"error_code":"unexpected_failure","msg":"An invite code is required to sign up"}"#,
        );
        assert_eq!(e.kind, AuthErrorKind::InviteRequired);
        let e = AuthError::from_response(422, r#"{"msg":"Invite code is invalid or already used"}"#);
        assert_eq!(e.kind, AuthErrorKind::BadInvite);
        assert_eq!(AuthError::from_response(429, "").kind, AuthErrorKind::RateLimited);
        let e = AuthError::from_response(
            422,
            r#"{"code":422,"error_code":"otp_disabled","msg":"Signups not allowed for otp"}"#,
        );
        assert_eq!(e.kind, AuthErrorKind::NoAccount);
        assert_eq!(AuthError::from_response(500, "<html>").message, "HTTP 500");
    }
}
