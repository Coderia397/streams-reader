//! RFC 3339 timestamps, as PostgREST reads and writes `timestamptz` (`2026-10-30T12:34:56.789+00:00`). Pure, so the
//! library and progress rows can be tested natively; times inside the app are milliseconds since the Unix epoch.

const MS_PER_DAY: i64 = 86_400_000;

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { yoe + era * 400 + 1 } else { yoe + era * 400 }, m, d)
}

fn num(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Milliseconds since the epoch, or `None` when the text is not an RFC 3339 date-time. Accepts `T` or a space
/// between date and time, any number of fraction digits, and `Z` or `±HH:MM` (also `±HH` and `±HHMM`).
pub fn parse_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 || !s.is_char_boundary(19) {
        return None;
    }
    let (date, rest) = s.split_at(10);
    let mut parts = date.split('-');
    let (y, m, d) = (num(parts.next()?)?, num(parts.next()?)?, num(parts.next()?)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let rest = rest.strip_prefix(['T', 't', ' '])?;
    let (hms, mut tail) = rest.split_at(8.min(rest.len()));
    let mut t = hms.split(':');
    let (hh, mm, ss) = (num(t.next()?)?, num(t.next()?)?, num(t.next()?)?);
    if hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    let mut frac_ms = 0;
    if let Some(f) = tail.strip_prefix('.') {
        let digits = f.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let padded: String = f[..digits].chars().chain("000".chars()).take(3).collect();
        frac_ms = num(&padded)?;
        tail = &f[digits..];
    }
    let offset_min = match tail {
        "Z" | "z" | "" => 0,
        _ => {
            let sign = match tail.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let digits: String = tail[1..].chars().filter(|c| *c != ':').collect();
            let (oh, om) = match digits.len() {
                2 => (num(&digits)?, 0),
                4 => (num(&digits[..2])?, num(&digits[2..])?),
                _ => return None,
            };
            sign * (oh * 60 + om)
        }
    };
    let days = days_from_civil(y, m, d);
    Some(days * MS_PER_DAY + ((hh * 60 + mm - offset_min) * 60 + ss) * 1000 + frac_ms)
}

/// `2026-10-30T12:34:56.789Z` for milliseconds since the epoch.
pub fn format_ms(ms: i64) -> String {
    let days = ms.div_euclid(MS_PER_DAY);
    let in_day = ms.rem_euclid(MS_PER_DAY);
    let (y, m, d) = civil_from_days(days);
    let (secs, millis) = (in_day / 1000, in_day % 1000);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z", secs / 3600, secs / 60 % 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_postgrest_timestamps() {
        assert_eq!(parse_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_ms("1970-01-01T00:00:01.5+00:00"), Some(1500));
        assert_eq!(parse_ms("2026-10-30T12:34:56.789012+00:00"), Some(1_793_363_696_789));
        assert_eq!(parse_ms("2026-10-30 14:34:56.789+02:00"), Some(1_793_363_696_789));
        assert_eq!(parse_ms("2026-10-30T07:04:56.789-0530"), Some(1_793_363_696_789));
        assert_eq!(parse_ms("2026-10-30T12:34:56+00"), Some(1_793_363_696_000));
        assert_eq!(parse_ms("1969-12-31T23:59:59Z"), Some(-1000));
    }

    #[test]
    fn rejects_other_text() {
        for bad in [
            "",
            "yesterday",
            "2026-13-01T00:00:00Z",
            "2026-10-30",
            "2026-10-30T25:00:00Z",
            "2026-10-30T12:00:00.Z",
            "2026-10-30T12:00:00+1",
        ] {
            assert_eq!(parse_ms(bad), None, "{bad}");
        }
    }

    #[test]
    fn formats_and_round_trips() {
        assert_eq!(format_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(format_ms(1_793_363_696_789), "2026-10-30T12:34:56.789Z");
        for ms in [-86_400_001, 951_782_400_000, 1_709_164_800_123, 4_102_444_799_999] {
            assert_eq!(parse_ms(&format_ms(ms)), Some(ms));
        }
    }
}
