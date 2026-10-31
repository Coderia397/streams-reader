//! HTML escaping. Every translated string and every value from outside the code goes through these.

/// Escape text for an element body.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escape a string for a JSON value placed inside `<script type="application/ld+json">`.
/// `serde_json` already escapes quotes; this also neutralises `</script` and HTML comment openers.
pub fn json_for_script(value: &serde_json::Value) -> String {
    value.to_string().replace('<', "\\u003c").replace('>', "\\u003e").replace('&', "\\u0026")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markup_and_quotes() {
        assert_eq!(esc(r#"<a href="x">Tom's & co</a>"#), "&lt;a href=&quot;x&quot;&gt;Tom&#39;s &amp; co&lt;/a&gt;");
    }

    #[test]
    fn json_in_script_cannot_close_the_tag() {
        let v = serde_json::json!({ "name": "</script><b>" });
        assert!(!json_for_script(&v).contains("</script"));
    }
}
