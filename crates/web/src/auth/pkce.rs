//! PKCE (RFC 7636) for OAuth and magic links: a random verifier kept on this device until the callback, and its
//! S256 challenge sent with the request.

use sha2::{Digest, Sha256};

use crate::util::dom;

const STORE_KEY: &str = "nolabs.pkce";
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Base64url without padding.
pub fn base64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    out
}

pub fn challenge(verifier: &str) -> String {
    base64url(&Sha256::digest(verifier.as_bytes()))
}

fn random_bytes() -> Option<[u8; 48]> {
    let mut buf = [0u8; 48];
    dom::window()?.crypto().ok()?.get_random_values_with_u8_array(&mut buf).ok()?;
    Some(buf)
}

/// Creates and stores a new verifier; returns its challenge.
pub fn begin() -> Option<String> {
    let verifier = base64url(&random_bytes()?);
    dom::local_storage()?.set_item(STORE_KEY, &verifier).ok()?;
    Some(challenge(&verifier))
}

/// The stored verifier, removed so it is used once.
pub fn take() -> Option<String> {
    let storage = dom::local_storage()?;
    let v = storage.get_item(STORE_KEY).ok().flatten();
    let _ = storage.remove_item(STORE_KEY);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_matches_rfc_4648_vectors() {
        assert_eq!(base64url(b""), "");
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(b"foob"), "Zm9vYg");
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8");
    }

    #[test]
    fn challenge_matches_rfc_7636_appendix_b() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
