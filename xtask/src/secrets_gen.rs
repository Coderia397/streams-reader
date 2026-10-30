//! Generation of the login passphrase and session key, and the passphrase hash the Worker compares against.
//!
//! The passphrase is 128 random bits written as 26 lowercase RFC 4648 base32 characters (no padding; the last
//! character carries 3 bits), grouped by 4 with dashes: `abcd-efgh-ijkl-mnop-qrst-uvwx-yz`. Base32 avoids `0/O` and
//! `1/l` confusion on a phone keyboard. The hash is over the exact grouped string as stored and typed.

use anyhow::{Context as _, Result, bail};
use hmac::{Hmac, Mac as _};
use sha2::Sha256;

use crate::secret::Secret;

const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
pub const PASSPHRASE_BITS: usize = 128;
pub const PASSPHRASE_CHARS: usize = PASSPHRASE_BITS.div_ceil(5);
const GROUP: usize = 4;

/// Lowercase base32 (RFC 4648 alphabet) without padding.
pub fn base32_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() * 8).div_ceil(5));
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    for &b in bytes {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(char::from(ALPHABET[((buffer >> bits) & 31) as usize]));
        }
        buffer &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(char::from(ALPHABET[((buffer << (5 - bits)) & 31) as usize]));
    }
    out
}

/// Splits `s` into groups of `n` chars joined by `-`.
pub fn group(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    chars.chunks(n.max(1)).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-")
}

pub fn is_valid_passphrase(s: &str) -> bool {
    let raw: String = s.chars().filter(|&c| c != '-').collect();
    raw.len() == PASSPHRASE_CHARS && raw.bytes().all(|b| ALPHABET.contains(&b)) && group(&raw, GROUP) == s
}

pub fn passphrase_from_bytes(bytes: &[u8; PASSPHRASE_BITS / 8]) -> Secret {
    Secret::new(group(&base32_lower(bytes), GROUP))
}

pub fn generate_passphrase() -> Result<Secret> {
    let mut bytes = [0u8; PASSPHRASE_BITS / 8];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no OS randomness: {e}"))?;
    let out = passphrase_from_bytes(&bytes);
    zeroize::Zeroize::zeroize(&mut bytes);
    Ok(out)
}

/// 32 random bytes as 64 lowercase hex chars.
pub fn generate_session_key() -> Result<Secret> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no OS randomness: {e}"))?;
    let out = Secret::new(hex::encode(bytes));
    zeroize::Zeroize::zeroize(&mut bytes);
    Ok(out)
}

pub fn is_valid_session_key(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Lowercase hex of HMAC-SHA256(key = the session key's 32 bytes, message = the passphrase's UTF-8 bytes).
pub fn passphrase_hash(session_key_hex: &Secret, passphrase: &Secret) -> Result<Secret> {
    let mut key = hex::decode(session_key_hex.expose()).context("SESSION_KEY is not hex")?;
    if key.is_empty() {
        bail!("SESSION_KEY is empty");
    }
    let mac = Hmac::<Sha256>::new_from_slice(&key).map_err(|_| anyhow::anyhow!("SESSION_KEY rejected by HMAC"));
    zeroize::Zeroize::zeroize(&mut key);
    let mut mac = mac?;
    mac.update(passphrase.expose().as_bytes());
    Ok(Secret::new(hex::encode(mac.finalize().into_bytes())))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn base32_matches_rfc4648_vectors_lowercased() {
        // RFC 4648 section 10, padding removed.
        let cases = [
            ("", ""),
            ("f", "my"),
            ("fo", "mzxq"),
            ("foo", "mzxw6"),
            ("foob", "mzxw6yq"),
            ("fooba", "mzxw6ytb"),
            ("foobar", "mzxw6ytboi"),
        ];
        for (input, want) in cases {
            assert_eq!(base32_lower(input.as_bytes()), want, "{input}");
        }
    }

    #[test]
    fn sixteen_bytes_make_26_chars() {
        assert_eq!(PASSPHRASE_CHARS, 26);
        assert_eq!(base32_lower(&[0xff; 16]).len(), 26);
        assert_eq!(base32_lower(&[0x00; 16]), "a".repeat(26));
        // 128 one-bits: 25 full groups of five, then 3 bits padded with two zeros = 0b11100 = 28 = '4'.
        assert_eq!(base32_lower(&[0xff; 16]), format!("{}4", "7".repeat(25)));
    }

    #[test]
    fn grouping() {
        assert_eq!(group("abcdefghij", 4), "abcd-efgh-ij");
        assert_eq!(group("abcd", 4), "abcd");
        assert_eq!(group("", 4), "");
    }

    #[test]
    fn passphrase_shape() {
        let p = passphrase_from_bytes(&[0u8; 16]);
        assert_eq!(p.expose(), "aaaa-aaaa-aaaa-aaaa-aaaa-aaaa-aa");
        assert_eq!(p.expose().len(), 26 + 6);
        assert!(is_valid_passphrase(p.expose()));
        let g = generate_passphrase().unwrap();
        assert!(is_valid_passphrase(g.expose()), "{:?}", g.expose().len());
        assert_ne!(g, generate_passphrase().unwrap());
    }

    #[test]
    fn invalid_passphrases() {
        for s in [
            "",
            "aaaa",
            "aaaaaaaaaaaaaaaaaaaaaaaaaa",
            "AAAA-AAAA-AAAA-AAAA-AAAA-AAAA-AA",
            "aaaa-aaaa-aaaa-aaaa-aaaa-aaaa-a1",
            "aaa-aaaaa-aaaa-aaaa-aaaa-aaaa-aa",
        ] {
            assert!(!is_valid_passphrase(s), "{s}");
        }
    }

    #[test]
    fn session_key_shape() {
        let k = generate_session_key().unwrap();
        assert!(is_valid_session_key(k.expose()));
        assert!(!is_valid_session_key(&"A".repeat(64)));
        assert!(!is_valid_session_key(&"a".repeat(63)));
    }

    #[test]
    fn hmac_matches_rfc4231_case_2() {
        let key = Secret::new(hex::encode("Jefe"));
        let msg = Secret::new("what do ya want for nothing?");
        assert_eq!(
            passphrase_hash(&key, &msg).unwrap().expose(),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn hmac_rejects_non_hex_key() {
        assert!(passphrase_hash(&Secret::new("zz"), &Secret::new("p")).is_err());
        assert!(passphrase_hash(&Secret::new(""), &Secret::new("p")).is_err());
    }
}
