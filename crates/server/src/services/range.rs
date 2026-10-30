//! Single-range `Range: bytes=…` handling for audio (RFC 9110 §14). Multi-range and malformed headers are ignored
//! (the full body is served), which the RFC allows; a well-formed range that starts past the end is 416.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteRange {
    /// Serve everything with 200.
    Full,
    /// Serve bytes `start..=end` with 206.
    Partial { start: u64, end: u64 },
    /// Answer 416 with `Content-Range: bytes */total`.
    Unsatisfiable,
}

impl ByteRange {
    /// Byte count of the served body.
    pub fn len(&self, total: u64) -> u64 {
        match self {
            ByteRange::Full => total,
            ByteRange::Partial { start, end } => end - start + 1,
            ByteRange::Unsatisfiable => 0,
        }
    }

    pub fn is_empty(&self, total: u64) -> bool {
        self.len(total) == 0
    }

    /// The `Content-Range` header value, when one is due.
    pub fn content_range(&self, total: u64) -> Option<String> {
        match self {
            ByteRange::Full => None,
            ByteRange::Partial { start, end } => Some(format!("bytes {start}-{end}/{total}")),
            ByteRange::Unsatisfiable => Some(format!("bytes */{total}")),
        }
    }
}

/// Resolve a `Range` header against an object of `total` bytes.
pub fn parse_range(header: Option<&str>, total: u64) -> ByteRange {
    let Some(spec) = header.map(str::trim).and_then(|h| h.strip_prefix("bytes=")) else {
        return ByteRange::Full;
    };
    if spec.contains(',') || total == 0 {
        return ByteRange::Full;
    }
    let Some((first, last)) = spec.trim().split_once('-') else {
        return ByteRange::Full;
    };
    let num = |s: &str| -> Option<u64> {
        let s = s.trim();
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) { None } else { s.parse().ok() }
    };
    match (first.trim().is_empty(), last.trim().is_empty()) {
        // bytes=-N: the last N bytes.
        (true, false) => match num(last) {
            Some(0) => ByteRange::Unsatisfiable,
            Some(n) => ByteRange::Partial { start: total.saturating_sub(n), end: total - 1 },
            None => ByteRange::Full,
        },
        // bytes=S-
        (false, true) => match num(first) {
            Some(start) if start >= total => ByteRange::Unsatisfiable,
            Some(start) => ByteRange::Partial { start, end: total - 1 },
            None => ByteRange::Full,
        },
        // bytes=S-E
        (false, false) => match (num(first), num(last)) {
            (Some(start), Some(end)) if start <= end => {
                if start >= total {
                    ByteRange::Unsatisfiable
                } else {
                    ByteRange::Partial { start, end: end.min(total - 1) }
                }
            }
            _ => ByteRange::Full,
        },
        (true, true) => ByteRange::Full,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ByteRange::*;

    #[test]
    fn range_table() {
        let cases: &[(Option<&str>, u64, ByteRange)] = &[
            (None, 100, Full),
            (Some("bytes=0-"), 100, Partial { start: 0, end: 99 }),
            (Some("bytes=0-0"), 100, Partial { start: 0, end: 0 }),
            (Some("bytes=10-19"), 100, Partial { start: 10, end: 19 }),
            (Some("bytes=90-200"), 100, Partial { start: 90, end: 99 }),
            (Some("bytes=-10"), 100, Partial { start: 90, end: 99 }),
            (Some("bytes=-500"), 100, Partial { start: 0, end: 99 }),
            (Some(" bytes= 5 - 6 "), 100, Partial { start: 5, end: 6 }),
            (Some("bytes=100-"), 100, Unsatisfiable),
            (Some("bytes=100-120"), 100, Unsatisfiable),
            (Some("bytes=-0"), 100, Unsatisfiable),
            (Some("bytes=20-10"), 100, Full),
            (Some("bytes=0-1,5-6"), 100, Full),
            (Some("bytes=a-b"), 100, Full),
            (Some("bytes=+1-2"), 100, Full),
            (Some("bytes=-"), 100, Full),
            (Some("bytes=5"), 100, Full),
            (Some("items=0-1"), 100, Full),
            (Some("bytes=0-"), 0, Full),
            (Some("bytes=18446744073709551616-"), 100, Full),
        ];
        for (header, total, want) in cases {
            assert_eq!(parse_range(*header, *total), *want, "{header:?} of {total}");
        }
    }

    #[test]
    fn headers_and_lengths() {
        assert_eq!(Partial { start: 10, end: 19 }.content_range(100).unwrap(), "bytes 10-19/100");
        assert_eq!(Partial { start: 10, end: 19 }.len(100), 10);
        assert_eq!(Unsatisfiable.content_range(100).unwrap(), "bytes */100");
        assert_eq!(Full.content_range(100), None);
        assert_eq!(Full.len(100), 100);
        assert!(Unsatisfiable.is_empty(100));
    }
}
