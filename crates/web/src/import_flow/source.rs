//! What the user handed us, and how to read it.

/// Selected text longer than this is imported as text even when it contains a link.
const SHARE_TEXT_AS_LINK_MAX: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSource {
    Paste { text: String, title: Option<String> },
    Link { url: String },
    File { name: String, bytes: Vec<u8> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Epub,
    Pdf,
    Text,
    Markdown,
    Html,
}

/// File types offered by the picker (`accept` attribute).
pub const ACCEPT: &str =
    ".epub,.pdf,.txt,.md,.markdown,.html,.htm,application/epub+zip,application/pdf,text/plain,text/markdown,text/html";

pub fn file_kind(name: &str) -> Option<FileKind> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    match ext.as_str() {
        "epub" => Some(FileKind::Epub),
        "pdf" => Some(FileKind::Pdf),
        "txt" | "text" => Some(FileKind::Text),
        "md" | "markdown" => Some(FileKind::Markdown),
        "html" | "htm" | "xhtml" => Some(FileKind::Html),
        _ => None,
    }
}

/// "my_great-book.v2.epub" → "my great book.v2".
pub fn title_from_filename(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    let cleaned: String = stem.chars().map(|c| if c == '_' || c == '-' { ' ' } else { c }).collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() { "Untitled".to_owned() } else { collapsed }
}

fn is_web_url(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://")) && s.len() > 10 && !s.contains(char::is_whitespace)
}

/// Accepts what people type in a link field: adds `https://` when the scheme is missing; `None` when it cannot be a
/// web address.
pub fn normalise_url(input: &str) -> Option<String> {
    let t = input.trim();
    if t.is_empty() || t.contains(char::is_whitespace) {
        return None;
    }
    let url = if t.contains("://") { t.to_owned() } else { format!("https://{t}") };
    let host = url.split("://").nth(1)?.split(['/', '?', '#']).next()?;
    (is_web_url(&url) && host.contains('.') && !host.starts_with('.') && !host.ends_with('.')).then_some(url)
}

/// First http(s) URL in free text.
pub fn find_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| matches!(c, '<' | '>' | '(' | ')' | '"' | '\'' | ',' | '.')))
        .find(|w| is_web_url(w))
        .map(str::to_owned)
}

/// Interprets a Web Share Target request (`?title&text&url`). Android often puts the page URL inside `text`.
pub fn from_share(title: Option<&str>, text: Option<&str>, url: Option<&str>) -> Option<ImportSource> {
    let title = title.map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned);
    let text = text.map(str::trim).unwrap_or_default();
    if let Some(u) = url.map(str::trim).filter(|u| is_web_url(u)) {
        if text.chars().count() > SHARE_TEXT_AS_LINK_MAX {
            return Some(ImportSource::Paste { text: text.to_owned(), title });
        }
        return Some(ImportSource::Link { url: u.to_owned() });
    }
    if let Some(u) = find_url(text) {
        let rest = text.replace(&u, "");
        if rest.trim().chars().count() <= SHARE_TEXT_AS_LINK_MAX {
            return Some(ImportSource::Link { url: u });
        }
    }
    (!text.is_empty()).then(|| ImportSource::Paste { text: text.to_owned(), title })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_kinds_by_extension() {
        assert_eq!(file_kind("Book.EPUB"), Some(FileKind::Epub));
        assert_eq!(file_kind("a.b.pdf"), Some(FileKind::Pdf));
        assert_eq!(file_kind("notes.md"), Some(FileKind::Markdown));
        assert_eq!(file_kind("page.htm"), Some(FileKind::Html));
        assert_eq!(file_kind("x.txt"), Some(FileKind::Text));
        assert_eq!(file_kind("x.docx"), None);
        assert_eq!(file_kind("README"), None);
    }

    #[test]
    fn titles_from_file_names() {
        assert_eq!(title_from_filename("my_great-book.v2.epub"), "my great book.v2");
        assert_eq!(title_from_filename(".epub"), "Untitled");
        assert_eq!(title_from_filename("plain"), "plain");
    }

    #[test]
    fn url_field_normalisation() {
        assert_eq!(normalise_url("example.com/a"), Some("https://example.com/a".into()));
        assert_eq!(normalise_url(" http://x.org "), Some("http://x.org".into()));
        assert_eq!(normalise_url("not a url"), None);
        assert_eq!(normalise_url("localhost"), None);
        assert_eq!(normalise_url("ftp://x.org/file"), None);
        assert_eq!(normalise_url(""), None);
    }

    #[test]
    fn share_with_url_param_imports_the_link() {
        assert_eq!(
            from_share(Some("Title"), Some("Look"), Some("https://example.com/post")),
            Some(ImportSource::Link { url: "https://example.com/post".into() })
        );
    }

    #[test]
    fn share_with_url_inside_text_imports_the_link() {
        assert_eq!(
            from_share(None, Some("Great read: https://example.com/a-b."), None),
            Some(ImportSource::Link { url: "https://example.com/a-b".into() })
        );
    }

    #[test]
    fn long_shared_selection_is_text() {
        let long = format!("{} https://example.com", "word ".repeat(80));
        assert!(matches!(from_share(Some("T"), Some(&long), None), Some(ImportSource::Paste { .. })));
        assert!(matches!(
            from_share(Some("T"), Some(&long), Some("https://example.com")),
            Some(ImportSource::Paste { title: Some(ref t), .. }) if t == "T"
        ));
        assert_eq!(from_share(None, Some("  "), None), None);
        assert_eq!(from_share(None, None, None), None);
    }
}
