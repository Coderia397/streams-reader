//! The subset of Fluent the email wording uses: `key = value` messages on one line, `#` comments, blank lines and
//! `{ $name }` placeables. A full Fluent runtime (plural rules, selectors) would add size to the Worker for features
//! these messages do not need; the files stay valid Fluent so the UI's i18n tooling can read them too.

use std::collections::HashMap;

/// Parse an `.ftl` file. Lines that are neither comments, blank nor `key = value` are reported with their number.
pub fn parse(source: &str) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    for (i, raw) in source.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once('=').ok_or_else(|| format!("line {}: expected key = value", i + 1))?;
        let key = key.trim();
        let valid_key = key.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !valid_key {
            return Err(format!("line {}: invalid message id {key:?}", i + 1));
        }
        if out.insert(key.to_owned(), value.trim().to_owned()).is_some() {
            return Err(format!("line {}: duplicate message id {key}", i + 1));
        }
    }
    Ok(out)
}

/// Replace each `{ $name }` with its value. An unknown name stays as written, so a gap is visible instead of blank.
pub fn fill(raw: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find("{ $") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 3..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = after[..end].trim();
        match vars.iter().find(|(k, _)| *k == name) {
            Some((_, v)) => out.push_str(v),
            None => out.push_str(&rest[start..start + 3 + end + 1]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The variable names a message uses, in order of appearance.
pub fn variables(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(start) = rest.find("{ $") {
        let after = &rest[start + 3..];
        let Some(end) = after.find('}') else { break };
        out.push(after[..end].trim().to_owned());
        rest = &after[end + 1..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_messages_comments_and_blank_lines() {
        let m = parse("# c\n\nhello = Hello { $name }!\nbye=Bye\n").unwrap();
        assert_eq!(m["hello"], "Hello { $name }!");
        assert_eq!(m["bye"], "Bye");
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn rejects_garbage_and_duplicates() {
        assert!(parse("just text").unwrap_err().contains("line 1"));
        assert!(parse("Bad = x").is_err());
        assert!(parse("a = 1\na = 2").unwrap_err().contains("duplicate"));
    }

    #[test]
    fn fills_known_variables_and_keeps_unknown_ones() {
        assert_eq!(fill("{ $a } and { $b }", &[("a", "1"), ("b", "2")]), "1 and 2");
        assert_eq!(fill("keep { $x }", &[]), "keep { $x }");
        assert_eq!(fill("open { $x", &[("x", "1")]), "open { $x");
        assert_eq!(variables("{ $code } is your { $brand } code"), ["code", "brand"]);
    }
}
