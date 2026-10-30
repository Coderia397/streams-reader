//! `POST /hooks/auth-email`: Supabase's Send Email hook. Verify the Standard Webhooks signature, read the payload,
//! write each email in the user's language and send it through Brevo from the brand's sender address. Supabase shows
//! the hook's error message to the person signing in, so refusals are short and readable; secrets never appear.

use nolabs_core::brand;
use nolabs_mail::hook::{self, Hook, Plan};
use nolabs_mail::webhook::{self, Refused};
use nolabs_mail::{Brand, brevo, render};

use crate::app::App;
use crate::ports::{HttpRequest, Method};

/// How long sign-in codes work, as the emails say. Must match `mailer_otp_exp` (600 s) set by `xtask supabase-auth`.
pub const CODE_MINUTES: u32 = 10;
const SEND_TIMEOUT_MS: u32 = 15_000;

/// The headers Supabase signs with.
#[derive(Debug, Clone, Copy)]
pub struct HookHeaders<'a> {
    pub id: &'a str,
    pub timestamp: &'a str,
    pub signature: &'a str,
}

/// A refusal in the shape Supabase expects: `{"error": {"http_code": …, "message": …}}` with that status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookError {
    pub http_code: u16,
    pub message: String,
}

impl HookError {
    fn new(http_code: u16, message: impl Into<String>) -> Self {
        HookError { http_code, message: message.into() }
    }

    pub fn body(&self) -> serde_json::Value {
        serde_json::json!({ "error": { "http_code": self.http_code, "message": self.message } })
    }
}

/// What a successful call did (logged by the Worker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookOutcome {
    Sent(usize),
    Skipped(String),
}

impl App {
    pub async fn handle_auth_email(&self, headers: HookHeaders<'_>, body: &[u8]) -> Result<HookOutcome, HookError> {
        let secret = self
            .config
            .send_email_hook_secret
            .as_deref()
            .ok_or_else(|| HookError::new(500, "email hook not configured"))?;
        let now_s = self.now_ms().div_euclid(1000);
        webhook::verify(secret, headers.id, headers.timestamp, headers.signature, body, now_s).map_err(
            |e| match e {
                Refused::Secret => HookError::new(500, "email hook misconfigured"),
                Refused::Timestamp => HookError::new(401, "stale or malformed webhook timestamp"),
                Refused::Signature => HookError::new(401, "webhook signature check failed"),
            },
        )?;
        let hook: Hook = serde_json::from_slice(body).map_err(|_| HookError::new(400, "unreadable hook payload"))?;
        let emails = match hook::plan(&hook, &self.config.site_url, CODE_MINUTES) {
            Plan::Send(emails) => emails,
            Plan::Skip(action) => return Ok(HookOutcome::Skipped(action)),
        };
        let key =
            self.config.brevo_api_key.as_deref().ok_or_else(|| HookError::new(500, "email sending not configured"))?;
        let brand = Brand { name: brand::NAME, site_url: &self.config.site_url };
        for email in &emails {
            let rendered = render::render(&brand, email);
            let tag = hook.email_data.email_action_type.as_str();
            let payload = brevo::body(brand::NAME, brand::SENDER_EMAIL, &email.to, tag, &rendered);
            let req = HttpRequest::new(Method::Post, brevo::ENDPOINT)
                .header("api-key", key)
                .header("accept", "application/json")
                .header("user-agent", brevo::USER_AGENT)
                .json_body(payload.to_string().into_bytes())
                .timeout_ms(SEND_TIMEOUT_MS);
            match self.http.send(req).await {
                Ok(resp) if (200..300).contains(&resp.status) => {}
                Ok(resp) => {
                    let detail: String = String::from_utf8_lossy(&resp.body).chars().take(200).collect();
                    return Err(HookError::new(
                        500,
                        format!("the email could not be sent (Brevo {}: {detail})", resp.status),
                    ));
                }
                Err(e) => return Err(HookError::new(500, format!("the email could not be sent ({e})"))),
            }
        }
        Ok(HookOutcome::Sent(emails.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::header_value;
    use crate::testing::fakes::response;
    use crate::testing::{TestEnv, run};

    const SECRET: &str = "v1,whsec_c2VjcmV0LWtleS1mb3ItdGVzdGluZy1vbmx5";

    fn payload(action: &str) -> Vec<u8> {
        serde_json::json!({
            "user": { "email": "ada@example.com", "user_metadata": { "locale": "es" } },
            "email_data": { "token": "123456", "token_hash": "hash1", "email_action_type": action,
                            "redirect_to": "", "site_url": "", "token_new": "", "token_hash_new": "" }
        })
        .to_string()
        .into_bytes()
    }

    fn call(env: &TestEnv, body: &[u8], signed_at_s: i64) -> Result<HookOutcome, HookError> {
        let ts = signed_at_s.to_string();
        let sig = webhook::sign(SECRET, "msg_1", &ts, body).unwrap();
        run(env.app.handle_auth_email(HookHeaders { id: "msg_1", timestamp: &ts, signature: &sig }, body))
    }

    #[test]
    fn a_signed_magic_link_is_sent_through_brevo_in_the_users_language() {
        let env = TestEnv::new();
        env.http.set(|_| Ok(response(201, &[], r#"{"messageId":"m"}"#)));
        let got = call(&env, &payload("magiclink"), env.now() / 1000).unwrap();
        assert_eq!(got, HookOutcome::Sent(1));
        let req = env.http.requests.borrow()[0].clone();
        assert_eq!(req.url, "https://api.brevo.com/v3/smtp/email");
        assert_eq!(header_value(&req.headers, "api-key"), Some("brevo-test-key"));
        let body: serde_json::Value = serde_json::from_slice(req.body.as_deref().unwrap()).unwrap();
        assert_eq!(body["sender"], serde_json::json!({"name": "Streams Reader", "email": "no-reply@streams.watch"}));
        assert_eq!(body["to"][0]["email"], "ada@example.com");
        assert_eq!(body["subject"], "123456 es tu código para entrar en Streams Reader");
        let html = body["htmlContent"].as_str().unwrap();
        assert!(html.contains("https://listen.streams.watch/auth/confirm?token_hash=hash1&amp;type=magiclink"));
    }

    #[test]
    fn bad_signatures_and_replays_are_refused_before_sending() {
        let env = TestEnv::new();
        let body = payload("magiclink");
        let now_s = env.now() / 1000;
        let stale = call(&env, &body, now_s - 301).unwrap_err();
        assert_eq!(stale.http_code, 401);
        let ts = now_s.to_string();
        let forged = webhook::sign("v1,whsec_b3RoZXI=", "msg_1", &ts, &body).unwrap();
        let e = run(env.app.handle_auth_email(HookHeaders { id: "msg_1", timestamp: &ts, signature: &forged }, &body))
            .unwrap_err();
        assert_eq!(e.http_code, 401);
        assert!(env.http.requests.borrow().is_empty());
        assert_eq!(e.body()["error"]["http_code"], 401);
    }

    #[test]
    fn notifications_are_acknowledged_without_sending() {
        let env = TestEnv::new();
        let got = call(&env, &payload("password_changed_notification"), env.now() / 1000).unwrap();
        assert_eq!(got, HookOutcome::Skipped("password_changed_notification".into()));
        assert!(env.http.requests.borrow().is_empty());
    }

    #[test]
    fn brevo_failures_and_missing_configuration_are_500() {
        let mut env = TestEnv::new();
        env.http.set(|_| Ok(response(400, &[], r#"{"code":"invalid_parameter"}"#)));
        let e = call(&env, &payload("signup"), env.now() / 1000).unwrap_err();
        assert_eq!(e.http_code, 500);
        assert!(!e.message.contains("brevo-test-key"));
        env.app.config.brevo_api_key = None;
        assert_eq!(call(&env, &payload("signup"), env.now() / 1000).unwrap_err().http_code, 500);
        env.app.config.send_email_hook_secret = None;
        assert_eq!(call(&env, &payload("signup"), env.now() / 1000).unwrap_err().http_code, 500);
    }

    #[test]
    fn garbage_payloads_are_400() {
        let env = TestEnv::new();
        assert_eq!(call(&env, b"not json", env.now() / 1000).unwrap_err().http_code, 400);
    }
}
