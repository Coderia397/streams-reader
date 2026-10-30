//! Supabase's Send Email hook payload and the emails it asks for. Links go to the app's own confirm page
//! (`<site>/auth/confirm?token_hash=…&type=…`), which calls `verifyOtp({ token_hash, type })`; the 6-digit code works
//! in every app, including native ones where a link would open the browser instead. The language is the user's
//! `locale` metadata (set at sign-up and when the display language changes), English when absent.

use serde::Deserialize;

use crate::render::{Email, Kind};

#[derive(Debug, Clone, Deserialize)]
pub struct Hook {
    pub user: User,
    pub email_data: EmailData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct User {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub new_email: Option<String>,
    #[serde(default)]
    pub user_metadata: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmailData {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub token_hash: String,
    #[serde(default)]
    pub token_new: String,
    #[serde(default)]
    pub token_hash_new: String,
    pub email_action_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Send(Vec<Email>),
    /// A notification type this app does not send (password changed, identity linked, …): acknowledged, not sent.
    Skip(String),
}

fn confirm_link(site_url: &str, token_hash: &str, kind: &str) -> String {
    format!("{}/auth/confirm?token_hash={}&type={kind}", site_url.trim_end_matches('/'), url_component(token_hash))
}

/// Percent-encode everything outside the unreserved set; token hashes are hex today, but this keeps links valid.
fn url_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn some(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_owned())
}

/// Which emails to send for one hook call.
pub fn plan(hook: &Hook, site_url: &str, minutes: u32) -> Plan {
    let meta = &hook.user.user_metadata;
    let lang = ["locale", "lang"].iter().find_map(|k| meta.get(*k).and_then(|v| v.as_str())).unwrap_or("en").to_owned();
    let d = &hook.email_data;
    let action = d.email_action_type.as_str();
    let email = |kind: Kind, to: &str, code: &str, hash: &str| Email {
        kind,
        lang: lang.clone(),
        to: to.to_owned(),
        code: some(code),
        link: (!hash.is_empty()).then(|| confirm_link(site_url, hash, action)),
        new_email: hook.user.new_email.clone(),
        minutes,
    };
    let to = hook.user.email.as_str();
    let emails = match action {
        "signup" => vec![email(Kind::Signup, to, &d.token, &d.token_hash)],
        "magiclink" | "email" => vec![email(Kind::Magiclink, to, &d.token, &d.token_hash)],
        "recovery" => vec![email(Kind::Recovery, to, &d.token, &d.token_hash)],
        "invite" => vec![email(Kind::Invite, to, "", &d.token_hash)],
        "reauthentication" => vec![Email { link: None, ..email(Kind::Reauthentication, to, &d.token, "") }],
        "email_change" => email_change(hook, &email),
        other => return Plan::Skip(other.to_owned()),
    };
    Plan::Send(emails)
}

/// Supabase's field names are reversed here for backward compatibility (its docs, "Email change behavior"): the
/// current address gets `token` with `token_hash_new`, the new address gets `token_new` with `token_hash`. With
/// secure email change off, only the new address gets an email.
fn email_change(hook: &Hook, email: &dyn Fn(Kind, &str, &str, &str) -> Email) -> Vec<Email> {
    let d = &hook.email_data;
    let new_address = hook.user.new_email.as_deref().filter(|e| !e.is_empty());
    let mut out = Vec::new();
    if !d.token_hash_new.is_empty() {
        out.push(email(Kind::EmailChange, &hook.user.email, &d.token, &d.token_hash_new));
        if let Some(to) = new_address {
            out.push(email(Kind::EmailChange, to, &d.token_new, &d.token_hash));
        }
    } else if let Some(to) = new_address {
        let code = if d.token_new.is_empty() { &d.token } else { &d.token_new };
        out.push(email(Kind::EmailChange, to, code, &d.token_hash));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITE: &str = "https://listen.streams.watch/";

    fn hook(json: &str) -> Hook {
        serde_json::from_str(json).unwrap()
    }

    fn sent(p: Plan) -> Vec<Email> {
        match p {
            Plan::Send(e) => e,
            Plan::Skip(a) => panic!("skipped {a}"),
        }
    }

    #[test]
    fn a_magic_link_carries_code_link_and_locale() {
        let h = hook(
            r#"{"user":{"email":"a@b.co","user_metadata":{"locale":"fr-CA"}},
                "email_data":{"token":"123456","token_hash":"abc","email_action_type":"magiclink"}}"#,
        );
        let e = &sent(plan(&h, SITE, 10))[0];
        assert_eq!(
            (e.kind, e.lang.as_str(), e.to.as_str(), e.code.as_deref()),
            (Kind::Magiclink, "fr-CA", "a@b.co", Some("123456"))
        );
        assert_eq!(e.link.as_deref(), Some("https://listen.streams.watch/auth/confirm?token_hash=abc&type=magiclink"));
    }

    #[test]
    fn signup_uses_its_own_type_and_english_by_default() {
        let h = hook(
            r#"{"user":{"email":"a@b.co"},"email_data":{"token":"1","token_hash":"h","email_action_type":"signup"}}"#,
        );
        let e = &sent(plan(&h, SITE, 10))[0];
        assert_eq!((e.kind, e.lang.as_str()), (Kind::Signup, "en"));
        assert!(e.link.as_deref().unwrap().ends_with("type=signup"));
    }

    #[test]
    fn reauthentication_is_code_only_and_invite_link_only() {
        let h = hook(
            r#"{"user":{"email":"a@b.co"},"email_data":{"token":"111222","token_hash":"x","email_action_type":"reauthentication"}}"#,
        );
        let e = &sent(plan(&h, SITE, 10))[0];
        assert!(e.link.is_none() && e.code.as_deref() == Some("111222"));
        let h = hook(
            r#"{"user":{"email":"a@b.co"},"email_data":{"token":"9","token_hash":"x","email_action_type":"invite"}}"#,
        );
        let e = &sent(plan(&h, SITE, 10))[0];
        assert!(e.code.is_none() && e.link.is_some());
    }

    #[test]
    fn secure_email_change_pairs_tokens_the_way_supabase_documents() {
        let h = hook(
            r#"{"user":{"email":"old@b.co","new_email":"new@b.co"},
                "email_data":{"token":"111111","token_hash":"for-new","token_new":"222222","token_hash_new":"for-old",
                "email_action_type":"email_change"}}"#,
        );
        let e = sent(plan(&h, SITE, 10));
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].to.as_str(), e[0].code.as_deref()), ("old@b.co", Some("111111")));
        assert!(e[0].link.as_deref().unwrap().contains("token_hash=for-old"));
        assert_eq!((e[1].to.as_str(), e[1].code.as_deref()), ("new@b.co", Some("222222")));
        assert!(e[1].link.as_deref().unwrap().contains("token_hash=for-new"));
        assert!(e.iter().all(|m| m.new_email.as_deref() == Some("new@b.co")));
    }

    #[test]
    fn plain_email_change_goes_to_the_new_address_only() {
        let h = hook(
            r#"{"user":{"email":"old@b.co","new_email":"new@b.co"},
                "email_data":{"token":"333333","token_hash":"h","email_action_type":"email_change"}}"#,
        );
        let e = sent(plan(&h, SITE, 10));
        assert_eq!(e.len(), 1);
        assert_eq!((e[0].to.as_str(), e[0].code.as_deref()), ("new@b.co", Some("333333")));
    }

    #[test]
    fn notifications_are_skipped_and_hashes_are_encoded() {
        let h =
            hook(r#"{"user":{"email":"a@b.co"},"email_data":{"email_action_type":"identity_linked_notification"}}"#);
        assert_eq!(plan(&h, SITE, 10), Plan::Skip("identity_linked_notification".into()));
        assert_eq!(url_component("a b&c"), "a%20b%26c");
    }
}
