//! Stable non-cryptographic hashing for visual identity (cover colours), so a title always gets the same cover on
//! every device without storing anything.

/// 32-bit FNV-1a over the UTF-8 bytes.
pub fn fnv1a(text: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in text.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Avalanche finaliser (lowbias32): spreads FNV's clustered low bits so nearby titles get distant values.
pub fn mix(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    h
}

/// Well-mixed 32-bit hash of a string.
pub fn stable_hash(text: &str) -> u32 {
    mix(fnv1a(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_reference_vectors() {
        assert_eq!(fnv1a(""), 0x811c_9dc5);
        assert_eq!(fnv1a("a"), 0xe40c_292c);
        assert_eq!(fnv1a("foobar"), 0xbf9c_f968);
    }

    #[test]
    fn mixing_spreads_similar_inputs() {
        let a = stable_hash("Notes on Craft") % 360;
        let b = stable_hash("The Last Train to Lisbon") % 360;
        assert!(a.abs_diff(b) > 30, "{a} {b}");
        assert_eq!(mix(0), 0);
    }
}
