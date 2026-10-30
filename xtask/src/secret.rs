//! Credentials in memory. A [`Secret`] cannot be printed by accident (its `Debug` is redacted and it has no
//! `Display`), is wiped on drop, and is only read through [`Secret::expose`] at the point where it goes into a request
//! header, a request body or a child process environment. [`Redactor`] scrubs known secret values out of any text
//! that came back from a server before it reaches an error message.

use std::fmt;

use zeroize::Zeroize;

/// What every secret value is replaced with in printed text.
pub const REDACTED: &str = "[redacted]";

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The raw value. Call sites are the only places a secret leaves this type, so keep them few and obvious.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret({REDACTED})")
    }
}

/// Replaces every occurrence of the registered secrets in a piece of text.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    secrets: Vec<Secret>,
}

impl Redactor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a secret. Empty values are ignored: replacing the empty string would mangle every message.
    pub fn add(&mut self, secret: &Secret) {
        if !secret.is_empty() && !self.secrets.contains(secret) {
            self.secrets.push(secret.clone());
            // Longest first, so a secret that contains another is replaced whole.
            self.secrets.sort_by_key(|s| std::cmp::Reverse(s.expose().len()));
        }
    }

    pub fn with(mut self, secret: &Secret) -> Self {
        self.add(secret);
        self
    }

    pub fn redact(&self, text: &str) -> String {
        self.secrets.iter().fold(text.to_owned(), |acc, s| acc.replace(s.expose(), REDACTED))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_the_value() {
        let s = Secret::new("hunter2-very-secret");
        let shown = format!("{s:?} {:?}", Some(&s));
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(shown.contains(REDACTED));
    }

    #[test]
    fn expose_returns_the_value() {
        assert_eq!(Secret::new("abc").expose(), "abc");
    }

    #[test]
    fn redacts_every_occurrence_of_every_secret() {
        let r = Redactor::new().with(&Secret::new("tok123")).with(&Secret::new("key-xyz"));
        assert_eq!(
            r.redact("bad token tok123 and key-xyz, again tok123"),
            "bad token [redacted] and [redacted], again [redacted]"
        );
    }

    #[test]
    fn longer_secret_wins_over_a_contained_shorter_one() {
        let r = Redactor::new().with(&Secret::new("abc")).with(&Secret::new("abcdef"));
        assert_eq!(r.redact("x abcdef y abc"), "x [redacted] y [redacted]");
    }

    #[test]
    fn empty_secret_is_ignored() {
        let r = Redactor::new().with(&Secret::new(""));
        assert_eq!(r.redact("plain text"), "plain text");
    }

    #[test]
    fn text_without_secrets_is_unchanged() {
        let r = Redactor::new().with(&Secret::new("s3cr3t"));
        assert_eq!(r.redact("nothing here"), "nothing here");
    }
}
