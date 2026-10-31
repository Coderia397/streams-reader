//! Invite checks before sign-up (callable signed out): `invite_status(code)` and `reserve_invite(code, email)`.

use serde::Deserialize;
use serde_json::json;

use super::send;
use crate::api::ClientError;

/// `invite_status` answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InviteStatus {
    Valid,
    Used,
    Expired,
    Unknown,
    /// `reserve_invite` only: bound to this email (or already was).
    Reserved,
    /// `reserve_invite` only: bound to another email.
    Taken,
    #[serde(other)]
    Invalid,
}

impl InviteStatus {
    pub fn usable(self) -> bool {
        matches!(self, InviteStatus::Valid | InviteStatus::Reserved)
    }

    /// The line under the invite field when the code cannot be used.
    pub fn problem(self) -> Option<String> {
        match self {
            InviteStatus::Valid | InviteStatus::Reserved => None,
            InviteStatus::Used => Some(t!("invite-used")),
            InviteStatus::Expired => Some(t!("invite-expired")),
            InviteStatus::Taken => Some(t!("invite-taken")),
            InviteStatus::Unknown | InviteStatus::Invalid => Some(t!("invite-unknown")),
        }
    }
}

fn parse(text: &str) -> InviteStatus {
    serde_json::from_str(text).unwrap_or(InviteStatus::Invalid)
}

pub async fn invite_status(code: &str) -> Result<InviteStatus, ClientError> {
    let body = json!({ "code": code.trim() }).to_string();
    send("POST", "rpc/invite_status", Some(&body), None).await.map(|t| parse(&t))
}

/// Binds the invite to `email` before an OAuth sign-up (which cannot carry the code).
pub async fn reserve_invite(code: &str, email: &str) -> Result<InviteStatus, ClientError> {
    let body = json!({ "code": code.trim(), "email": email.trim() }).to_string();
    send("POST", "rpc/reserve_invite", Some(&body), None).await.map(|t| parse(&t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rpc_answers() {
        assert_eq!(parse("\"valid\""), InviteStatus::Valid);
        assert_eq!(parse("\"reserved\""), InviteStatus::Reserved);
        assert_eq!(parse("\"bad_email\""), InviteStatus::Invalid);
        assert!(InviteStatus::Reserved.usable());
        assert!(!InviteStatus::Taken.usable());
    }
}
