//! Brevo's transactional email API (`POST https://api.brevo.com/v3/smtp/email`, header `api-key`), as Streams uses
//! it. Only the request body is built here; the Worker's HTTP port sends it.

use serde_json::{Value, json};

use crate::render::Rendered;

pub const ENDPOINT: &str = "https://api.brevo.com/v3/smtp/email";
/// Brevo sits behind Cloudflare, which refuses requests without a recognisable client name (seen in Streams).
pub const USER_AGENT: &str = "streams-reader-mail/1.0 (+https://listen.streams.watch)";

/// The JSON body for one message. `tags` lets the Brevo log be filtered by email type.
pub fn body(sender_name: &str, sender_email: &str, to: &str, tag: &str, email: &Rendered) -> Value {
    json!({
        "sender": { "name": sender_name, "email": sender_email },
        "to": [{ "email": to }],
        "subject": email.subject,
        "htmlContent": email.html,
        "textContent": email.text,
        "tags": ["auth", tag],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_has_sender_recipient_and_both_parts() {
        let r = Rendered { subject: "S".into(), html: "<p>H</p>".into(), text: "T".into() };
        let b = body("Streams Reader", "no-reply@streams.watch", "a@b.co", "magiclink", &r);
        assert_eq!(b["sender"], json!({"name": "Streams Reader", "email": "no-reply@streams.watch"}));
        assert_eq!(b["to"], json!([{"email": "a@b.co"}]));
        assert_eq!(
            (b["subject"].as_str(), b["htmlContent"].as_str(), b["textContent"].as_str()),
            (Some("S"), Some("<p>H</p>"), Some("T"))
        );
        assert_eq!(b["tags"], json!(["auth", "magiclink"]));
    }
}
