//! Opaque ids: 16 chars of lowercase Crockford base32 from 10 random bytes. Randomness comes from the caller
//! (`crypto.getRandomValues` in the Worker), so this module stays pure.

const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

pub fn id_from_bytes(bytes: [u8; 10]) -> String {
    let mut bits: u128 = 0;
    for b in bytes {
        bits = (bits << 8) | u128::from(b);
    }
    (0..16).rev().map(|i| ALPHABET[((bits >> (i * 5)) & 31) as usize] as char).collect()
}

pub fn is_id(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| ALPHABET.contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_16_chars_from_the_alphabet() {
        let id = id_from_bytes([0xff; 10]);
        assert_eq!(id, "zzzzzzzzzzzzzzzz");
        assert!(is_id(&id_from_bytes([7, 1, 2, 3, 4, 5, 6, 7, 8, 9])));
        assert!(!is_id("ABC"));
        assert!(!is_id("0123456789abcdei"));
    }
}
