//! Numbers inside messages. Fluent's own formatting ignores `maximumFractionDigits` and always writes a full stop, so
//! every bundle gets [`formatter`]: `{ NUMBER($n, maximumFractionDigits: 1) }` rounds to one decimal, trailing zeros
//! are dropped down to `minimumFractionDigits`, and the decimal separator is the language's own ("1,5" in German).
//! Digits stay Latin in every language, like the rest of the interface. Plural selection is not affected.

use fluent_bundle::FluentValue;
use fluent_bundle::types::FluentNumber;

/// Formats `n` with its `NUMBER()` options and `decimal` as the separator.
pub fn text(n: &FluentNumber, decimal: char) -> String {
    let min = n.options.minimum_fraction_digits.unwrap_or(0);
    let mut s = match n.options.maximum_fraction_digits {
        Some(max) => {
            let mut s = format!("{:.*}", max, n.value);
            if let Some(dot) = s.find('.') {
                let keep = dot + 1 + min.min(max);
                while s.len() > keep && s.ends_with('0') {
                    s.pop();
                }
                if s.ends_with('.') {
                    s.pop();
                }
            }
            s
        }
        None => n.as_string().into_owned(),
    };
    // Rounding can leave "-0" for a tiny negative value.
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s.remove(0);
    }
    if decimal == '.' { s } else { s.replace('.', decimal.encode_utf8(&mut [0; 4])) }
}

fn with(value: &FluentValue, decimal: char) -> Option<String> {
    match value {
        FluentValue::Number(n) => Some(text(n, decimal)),
        _ => None,
    }
}

fn point<M>(value: &FluentValue, _: &M) -> Option<String> {
    with(value, '.')
}

fn comma<M>(value: &FluentValue, _: &M) -> Option<String> {
    with(value, ',')
}

/// The bundle formatter for a language whose decimal separator is `decimal` ('.' or ',').
pub fn formatter<M>(decimal: char) -> fn(&FluentValue, &M) -> Option<String> {
    if decimal == ',' { comma::<M> } else { point::<M> }
}

#[cfg(test)]
mod tests {
    use fluent_bundle::types::FluentNumberOptions;

    use super::*;

    fn num(value: f64, min: Option<usize>, max: Option<usize>) -> FluentNumber {
        let options =
            FluentNumberOptions { minimum_fraction_digits: min, maximum_fraction_digits: max, ..Default::default() };
        FluentNumber::new(value, options)
    }

    #[test]
    fn rounds_trims_and_localises() {
        assert_eq!(text(&num(822.44, None, Some(1)), '.'), "822.4");
        assert_eq!(text(&num(822.44, None, Some(1)), ','), "822,4");
        assert_eq!(text(&num(12.0, None, Some(1)), ','), "12");
        assert_eq!(text(&num(1.25, None, Some(2)), ','), "1,25");
        assert_eq!(text(&num(1.5, None, Some(2)), '.'), "1.5");
        assert_eq!(text(&num(2.999_999, None, Some(2)), '.'), "3");
        assert_eq!(text(&num(1.0, Some(1), Some(2)), ','), "1,0");
        assert_eq!(text(&num(-0.001, None, Some(1)), '.'), "0");
        assert_eq!(text(&num(7.0, None, None), ','), "7");
    }
}
