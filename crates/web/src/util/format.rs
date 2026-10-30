//! Human-readable numbers for the UI.

/// `"12 min left"`, `"1 h 5 min left"`, `"Under a minute left"`.
pub fn time_left(seconds: f64) -> String {
    let minutes = (seconds / 60.0).round() as u64;
    match minutes {
        0 => "Under a minute left".to_owned(),
        m if m < 60 => format!("{m} min left"),
        m if m % 60 == 0 => format!("{} h left", m / 60),
        m => format!("{} h {} min left", m / 60, m % 60),
    }
}

/// `"18 min"`, `"2 h 10 min"` for a total listening time.
pub fn duration_short(seconds: f64) -> String {
    let minutes = (seconds / 60.0).round().max(1.0) as u64;
    if minutes < 60 { format!("{minutes} min") } else { format!("{} h {} min", minutes / 60, minutes % 60) }
}

/// `"0:07"`, `"12:30"` for clip lengths.
pub fn clock(ms: u32) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// `"1.25x"`, `"1x"`, `"0.95x"`: shortest form without trailing zeros.
pub fn speed(rate: f64) -> String {
    let hundredths = (rate * 100.0).round() as i64;
    let whole = hundredths / 100;
    let frac = hundredths % 100;
    match frac {
        0 => format!("{whole}x"),
        f if f % 10 == 0 => format!("{whole}.{}x", f / 10),
        f => format!("{whole}.{f:02}x"),
    }
}

/// Bytes as `"12.4 MB"` (decimal units, like Android's storage screen).
pub fn bytes(n: f64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut v = n.max(0.0);
    let mut unit = 0;
    while v >= 1000.0 && unit < UNITS.len() - 1 {
        v /= 1000.0;
        unit += 1;
    }
    if unit == 0 { format!("{v:.0} {}", UNITS[unit]) } else { format!("{v:.1} {}", UNITS[unit]) }
}

/// Up to two initials from a title's significant words, skipping articles, short function words and punctuation:
/// "The Old Man and the Sea" → "OM", "Notes on Craft" → "NC".
pub fn initials(title: &str) -> String {
    const SKIP: [&str; 14] = ["the", "a", "an", "of", "on", "in", "to", "for", "and", "or", "at", "by", "with", "&"];
    let words: Vec<&str> = title
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .collect();
    let significant: Vec<&str> = words.iter().copied().filter(|w| !SKIP.contains(&w.to_lowercase().as_str())).collect();
    let significant = if significant.is_empty() { words } else { significant };
    let out: String =
        significant.iter().filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).take(2).collect();
    if out.is_empty() { "·".to_owned() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_left_reads_naturally() {
        assert_eq!(time_left(20.0), "Under a minute left");
        assert_eq!(time_left(12.0 * 60.0), "12 min left");
        assert_eq!(time_left(120.0 * 60.0), "2 h left");
        assert_eq!(time_left(65.0 * 60.0), "1 h 5 min left");
    }

    #[test]
    fn durations_and_clocks() {
        assert_eq!(duration_short(10.0), "1 min");
        assert_eq!(duration_short(130.0 * 60.0), "2 h 10 min");
        assert_eq!(clock(7_400), "0:07");
        assert_eq!(clock(750_000), "12:30");
    }

    #[test]
    fn speeds_drop_trailing_zeros() {
        assert_eq!(speed(1.0), "1x");
        assert_eq!(speed(1.25), "1.25x");
        assert_eq!(speed(1.5), "1.5x");
        assert_eq!(speed(0.95), "0.95x");
        assert_eq!(speed(2.999_999), "3x");
    }

    #[test]
    fn byte_units() {
        assert_eq!(bytes(512.0), "512 B");
        assert_eq!(bytes(12_400_000.0), "12.4 MB");
    }

    #[test]
    fn initials_skip_articles_and_punctuation() {
        assert_eq!(initials("The Old Man and the Sea"), "OM");
        assert_eq!(initials("Notes on Craft"), "NC");
        assert_eq!(initials("The Last Train to Lisbon"), "LT");
        assert_eq!(initials("The"), "T");
        assert_eq!(initials("  \"dune\" "), "D");
        assert_eq!(initials("A"), "A");
        assert_eq!(initials("—"), "·");
        assert_eq!(initials("état d'urgence"), "ÉD");
    }
}
