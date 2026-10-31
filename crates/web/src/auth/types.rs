//! Shapes of the Supabase Auth (GoTrue) answers the app reads. Unknown fields are ignored.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    /// Seconds until the access token expires.
    pub expires_in: u32,
    pub user: User,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct User {
    pub id: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub identities: Vec<Identity>,
}

/// One way of signing in that is linked to the account (`email`, `google`, `azure`, `apple`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Identity {
    pub identity_id: String,
    pub provider: String,
    #[serde(default)]
    pub identity_data: Option<IdentityData>,
    #[serde(default)]
    pub created_at: String,
}

impl Identity {
    /// The shape `nolabs_core::auth::identity_rows` reads; identities of providers the app does not offer are
    /// left out.
    pub fn to_core(&self) -> Option<nolabs_core::auth::Identity> {
        Some(nolabs_core::auth::Identity {
            identity_id: self.identity_id.clone(),
            provider: provider_from_id(&self.provider)?,
            email: self.identity_data.as_ref().and_then(|d| d.email.clone()),
            created_at: self.created_at.clone(),
        })
    }
}

impl User {
    pub fn core_identities(&self) -> Vec<nolabs_core::auth::Identity> {
        self.identities.iter().filter_map(Identity::to_core).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct IdentityData {
    #[serde(default)]
    pub email: Option<String>,
}

/// `GET /auth/v1/settings`: which external providers are switched on.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct AuthSettings {
    #[serde(default)]
    pub external: std::collections::BTreeMap<String, bool>,
}

/// Sign-in methods (`nolabs_core::auth::Provider`; Microsoft is Supabase's `azure`).
pub use nolabs_core::auth::Provider;

/// The provider's id in Supabase and in `identities[].provider`.
pub fn provider_id(p: Provider) -> &'static str {
    match p {
        Provider::Email => "email",
        Provider::Google => "google",
        Provider::Azure => "azure",
        Provider::Apple => "apple",
    }
}

pub fn provider_from_id(id: &str) -> Option<Provider> {
    [Provider::Email, Provider::Google, Provider::Azure, Provider::Apple].into_iter().find(|p| provider_id(*p) == id)
}

impl AuthSettings {
    /// Enabled providers, in display order.
    pub fn providers(&self) -> Vec<Provider> {
        Provider::OAUTH.into_iter().filter(|p| self.external.get(provider_id(*p)).copied().unwrap_or(false)).collect()
    }
}

/// Extra sign-up data stored on the new user (`raw_user_meta_data`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignUpData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite_code: Option<String>,
    pub locale: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_session_and_settings() {
        let s: Result<Session, _> = serde_json::from_str(
            r#"{"access_token":"a","token_type":"bearer","expires_in":3600,"expires_at":1,"refresh_token":"r",
                "user":{"id":"u","email":"e@x.org","identities":[{"identity_id":"i","id":"x","provider":"google",
                "identity_data":{"email":"e@x.org","sub":"1"}}]}}"#,
        );
        let s = s.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(s.user.identities[0].provider, "google");
        let settings: AuthSettings = serde_json::from_str(
            r#"{"external":{"google":true,"azure":false,"apple":true,"email":true},"disable_signup":false}"#,
        )
        .unwrap_or_default();
        assert_eq!(settings.providers(), vec![Provider::Google, Provider::Apple]);
        assert_eq!(provider_from_id("azure"), Some(Provider::Azure));
        let core = s.user.core_identities();
        assert_eq!(core[0].provider, Provider::Google);
        assert_eq!(core[0].email.as_deref(), Some("e@x.org"));
    }

    #[test]
    fn sign_up_data_omits_a_missing_invite() {
        let d = SignUpData { invite_code: None, locale: "ar".into() };
        assert_eq!(serde_json::to_string(&d).unwrap_or_default(), r#"{"locale":"ar"}"#);
    }
}
