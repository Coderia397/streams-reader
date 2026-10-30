//! Small quick-xml helpers shared by the package and XHTML readers: byte decoding, entities and attributes.

use std::borrow::Cow;

use quick_xml::XmlVersion;
use quick_xml::escape::resolve_html5_entity;
use quick_xml::events::{BytesRef, BytesStart};

/// Bytes of an EPUB XML file as text. EPUB requires UTF-8 or UTF-16; anything else is read as lossy UTF-8.
pub(crate) fn decode(bytes: &[u8]) -> Cow<'_, str> {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest);
    }
    let utf16 = |rest: &[u8], le: bool| {
        let units = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| if le { u16::from_le_bytes(pair) } else { u16::from_be_bytes(pair) });
        Cow::Owned(char::decode_utf16(units).map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER)).collect())
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        _ => String::from_utf8_lossy(bytes),
    }
}

/// Text of an entity or character reference (`&amp;`, `&nbsp;`, `&#8212;`). Unknown entities are dropped rather than
/// read aloud as `&name;`.
pub(crate) fn resolve_ref(r: &BytesRef<'_>) -> Option<String> {
    if let Ok(Some(c)) = r.resolve_char_ref() {
        return Some(c.to_string());
    }
    resolve_html5_entity(r).map(str::to_owned)
}

/// Attributes as `(lower-case qualified name, value)`. Malformed attributes are skipped; values with unknown entities
/// keep their raw text.
pub(crate) fn attributes(e: &BytesStart<'_>) -> Vec<(String, String)> {
    let mut attrs = e.attributes();
    attrs.with_checks(false);
    attrs
        .filter_map(Result::ok)
        .map(|a| {
            let value = a
                .normalized_value_with(XmlVersion::Implicit1_0, 8, resolve_html5_entity)
                .map_or_else(|_| a.value.to_string(), Cow::into_owned);
            (a.key.as_ref().to_ascii_lowercase(), value)
        })
        .collect()
}

/// Lower-case local name of an element (`dc:title` → `title`).
pub(crate) fn local_name(e: &BytesStart<'_>) -> String {
    e.local_name().as_ref().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    use super::*;

    #[test]
    fn decodes_boms() {
        assert_eq!(decode(b"\xEF\xBB\xBFhi"), "hi");
        assert_eq!(decode(&[0xFF, 0xFE, b'h', 0, b'i', 0]), "hi");
        assert_eq!(decode(&[0xFE, 0xFF, 0, b'h', 0, b'i']), "hi");
        assert_eq!(decode(b"plain"), "plain");
    }

    #[test]
    fn resolves_named_and_numeric_refs() {
        assert_eq!(resolve_ref(&BytesRef::new("amp")).as_deref(), Some("&"));
        assert_eq!(resolve_ref(&BytesRef::new("mdash")).as_deref(), Some("—"));
        assert_eq!(resolve_ref(&BytesRef::new("#x2014")).as_deref(), Some("—"));
        assert_eq!(resolve_ref(&BytesRef::new("#8217")).as_deref(), Some("’"));
        assert_eq!(resolve_ref(&BytesRef::new("nosuchentity")), None);
    }

    #[test]
    fn reads_attributes_with_entities() {
        let mut r = Reader::from_str(r#"<dc:creator opf:role="aut" id='c1' title="A &amp; B" bad=x>N</dc:creator>"#);
        let mut found = None;
        while let Ok(ev) = r.read_event() {
            match ev {
                Event::Start(e) => found = Some((local_name(&e), attributes(&e))),
                Event::Eof => break,
                _ => {}
            }
        }
        let (name, attrs) = found.unwrap_or_default();
        assert_eq!(name, "creator");
        assert!(attrs.contains(&("opf:role".into(), "aut".into())));
        assert!(attrs.contains(&("id".into(), "c1".into())));
        assert!(attrs.contains(&("title".into(), "A & B".into())));
    }
}
