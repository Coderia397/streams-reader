//! The product's public identity in one place, so a rename is one change: email subjects and bodies, the sender's
//! display name, the auth site URL and redirects, and the CORS allowlist all read these constants. Internal resource
//! names (`nolabs-api`, the D1 database, crate names) are separate and stay until a later refactor.
//!
//! Two places cannot import a Rust constant and carry the name as text: the refusal messages in the Supabase
//! migrations (`supabase/migrations/*.sql`) and the `.ftl` wording, which receives it as `{ $brand }`.

/// Shown to people: email subjects, the sender name, the app title.
pub const NAME: &str = "Streams Reader";
/// Where the web app lives; the auth site URL and the address printed in emails. No trailing slash.
pub const SITE_URL: &str = "https://listen.streams.watch";
/// Sender of the auth emails (a Brevo-verified sender of the streams.watch domain).
pub const SENDER_EMAIL: &str = "no-reply@streams.watch";
/// URL scheme of the native apps' deep links (`<scheme>://auth`).
pub const APP_SCHEME: &str = "nolabs";

/// The sender as an RFC 5322 display form: `Streams Reader <no-reply@streams.watch>`.
pub fn sender() -> String {
    format!("{NAME} <{SENDER_EMAIL}>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sender_has_display_name_and_address() {
        assert_eq!(sender(), "Streams Reader <no-reply@streams.watch>");
        assert!(SITE_URL.starts_with("https://") && !SITE_URL.ends_with('/'));
    }
}
