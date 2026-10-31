//! Sign-in methods and how identities become accounts. One person must end up with ONE account whichever way they
//! sign in (email code, Google, Microsoft, Apple). Supabase does the linking; this module states the rules the apps
//! and the settings page rely on, so they are written down and tested in one place:
//!
//! 1. Automatic linking: a new identity whose provider reports a *verified* email equal to an existing account's
//!    email joins that account (Supabase's built-in behaviour). Unverified emails never link; they would let anyone
//!    who registers an address at some provider take over the account.
//! 2. Apple's private relay addresses (`…@privaterelay.appleid.com`) never equal an existing email, so they cannot
//!    link automatically. The settings page offers manual linking (`supabase.auth.linkIdentity({ provider })` while
//!    signed in; enabled in the project with `security_manual_linking_enabled`).
//! 3. A sign-in that would create a NEW account needs an invite (database hook): an invite code typed with email
//!    sign-up, or an invite bound to that verified email (`reserve_invite`) for OAuth; the owner's email is always
//!    allowed.
//! 4. Unlinking: an account keeps at least one identity (Supabase refuses to remove the last one); the settings page
//!    hides the unlink button in that case.

use serde::{Deserialize, Serialize};

/// The sign-in methods the apps offer. `Azure` is Supabase's name for Microsoft accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Email,
    Google,
    Azure,
    Apple,
}

impl Provider {
    pub const OAUTH: [Provider; 3] = [Provider::Google, Provider::Azure, Provider::Apple];

    /// The label on the settings page and sign-in buttons.
    pub fn label(self) -> &'static str {
        match self {
            Provider::Email => "Email",
            Provider::Google => "Google",
            Provider::Azure => "Microsoft",
            Provider::Apple => "Apple",
        }
    }
}

/// One linked sign-in method, as the settings page lists it. Built from `supabase.auth.getUserIdentities()`
/// (`identities[]` of the GoTrue user: `identity_id`, `provider`, `identity_data.email`, `created_at`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub identity_id: String,
    pub provider: Provider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub created_at: String,
}

/// What the settings page shows for each method: linked (with or without an unlink button) or available to link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityRow {
    pub provider: Provider,
    pub label: &'static str,
    /// The identity when linked.
    pub identity: Option<Identity>,
    pub can_unlink: bool,
    /// Offered when not linked and the provider is enabled in the project.
    pub can_link: bool,
}

/// Apple's private relay domain: an address that hides the real one and never matches another provider's email.
pub const APPLE_RELAY_DOMAIN: &str = "privaterelay.appleid.com";

pub fn is_private_relay(email: &str) -> bool {
    email.rsplit_once('@').is_some_and(|(_, domain)| domain.trim().eq_ignore_ascii_case(APPLE_RELAY_DOMAIN))
}

/// Emails compare without case and surrounding spaces (Supabase stores them lowercased).
pub fn same_email(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// What happens when an identity signs in for the first time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrival {
    /// Joins the existing account with the same verified email (rule 1).
    LinksToExisting,
    /// Would create a new account: allowed only with an invite (rule 3).
    NeedsInvite,
}

/// Rule 1 and 2 as a function of the incoming identity and the existing accounts' emails.
pub fn arrival(incoming_email: &str, email_verified: bool, existing_emails: &[&str]) -> Arrival {
    let links = email_verified
        && !is_private_relay(incoming_email)
        && existing_emails.iter().any(|e| same_email(e, incoming_email));
    if links { Arrival::LinksToExisting } else { Arrival::NeedsInvite }
}

/// The settings page rows: every method in a fixed order, linked ones with their identity. Unlinking is offered only
/// while another identity remains (rule 4).
pub fn identity_rows(identities: &[Identity], enabled: &[Provider]) -> Vec<IdentityRow> {
    let order = [Provider::Email, Provider::Google, Provider::Azure, Provider::Apple];
    order
        .iter()
        .map(|&provider| {
            let identity = identities.iter().find(|i| i.provider == provider).cloned();
            let linked = identity.is_some();
            IdentityRow {
                provider,
                label: provider.label(),
                identity,
                can_unlink: linked && identities.len() > 1,
                can_link: !linked && provider != Provider::Email && enabled.contains(&provider),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(provider: Provider, email: &str) -> Identity {
        Identity {
            identity_id: format!("{provider:?}"),
            provider,
            email: Some(email.into()),
            created_at: "2026-10-30T00:00:00Z".into(),
        }
    }

    #[test]
    fn verified_same_email_links_whatever_the_provider() {
        let existing = ["ada@example.com"];
        assert_eq!(arrival("Ada@Example.com ", true, &existing), Arrival::LinksToExisting);
        assert_eq!(arrival("other@example.com", true, &existing), Arrival::NeedsInvite);
    }

    #[test]
    fn unverified_emails_never_link() {
        assert_eq!(arrival("ada@example.com", false, &["ada@example.com"]), Arrival::NeedsInvite);
    }

    #[test]
    fn apple_relay_addresses_never_link_automatically() {
        let relay = "x1y2z3@privaterelay.appleid.com";
        assert!(is_private_relay(relay) && is_private_relay("A@PrivateRelay.AppleID.com"));
        assert!(!is_private_relay("ada@icloud.com"));
        assert_eq!(arrival(relay, true, &[relay]), Arrival::NeedsInvite);
    }

    #[test]
    fn settings_rows_offer_link_and_unlink_correctly() {
        let only_email = [id(Provider::Email, "ada@example.com")];
        let rows = identity_rows(&only_email, &[Provider::Google]);
        assert_eq!(rows.len(), 4);
        assert!(!rows[0].can_unlink, "the last identity cannot be removed");
        assert!(rows[1].can_link, "Google is enabled and not linked");
        assert!(!rows[2].can_link && !rows[3].can_link, "Microsoft and Apple are not enabled");
        assert_eq!(rows[2].label, "Microsoft");

        let two = [id(Provider::Email, "ada@example.com"), id(Provider::Apple, "r@privaterelay.appleid.com")];
        let rows = identity_rows(&two, &Provider::OAUTH);
        assert!(rows[0].can_unlink && rows[3].can_unlink);
        assert!(!rows[3].can_link);
    }

    #[test]
    fn identity_json_matches_the_provider_names_supabase_uses() {
        let i: Identity = serde_json::from_str(r#"{"identity_id":"x","provider":"azure","created_at":"t"}"#).unwrap();
        assert_eq!((i.provider, i.email), (Provider::Azure, None));
    }
}
