//! Lines shown under a document's title.

use nolabs_core::proto::api::{DocumentSummary, SourceKind};

use crate::reading::doc::seconds_for_chars;
use crate::util::format;

/// Within this many chars of the end counts as finished.
const FINISHED_MARGIN: u64 = 40;

/// `example.com` from a URL (no scheme, no `www.`).
pub fn domain(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next()?.rsplit('@').next()?;
    let host = host.split(':').next()?.trim_start_matches("www.");
    (!host.is_empty()).then(|| host.to_owned())
}

pub fn source_label(d: &DocumentSummary) -> String {
    match d.meta.source {
        SourceKind::Url => d.meta.source_url.as_deref().and_then(domain).unwrap_or_else(|| "Web page".to_owned()),
        SourceKind::Paste => "Text".to_owned(),
        SourceKind::Epub => "Book".to_owned(),
        SourceKind::Pdf => "PDF".to_owned(),
        SourceKind::Text => "Text file".to_owned(),
        SourceKind::Markdown => "Markdown".to_owned(),
        SourceKind::Html => "Web page".to_owned(),
    }
}

pub fn is_finished(d: &DocumentSummary) -> bool {
    d.progress.is_some_and(|p| p.char_offset + FINISHED_MARGIN >= d.meta.char_count)
}

/// 0.0–1.0 listened.
pub fn fraction(d: &DocumentSummary) -> f64 {
    match d.progress {
        Some(p) if d.meta.char_count > 0 => (p.char_offset as f64 / d.meta.char_count as f64).clamp(0.0, 1.0),
        _ => 0.0,
    }
}

/// "Finished", "12 min left" or the total length ("18 min").
pub fn time_line(d: &DocumentSummary, rate: f64) -> String {
    if is_finished(d) {
        return "Finished".to_owned();
    }
    match d.progress {
        Some(p) if p.char_offset > 0 => {
            format::time_left(seconds_for_chars(d.meta.char_count.saturating_sub(p.char_offset), rate))
        }
        _ => format::duration_short(seconds_for_chars(d.meta.char_count, rate)),
    }
}

pub fn meta_line(d: &DocumentSummary, rate: f64) -> String {
    format!("{} · {}", source_label(d), time_line(d, rate))
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::{DocumentMeta, Progress};

    use super::*;

    fn doc(source: SourceKind, url: Option<&str>, chars: u64, offset: Option<u64>) -> DocumentSummary {
        DocumentSummary {
            id: "d".into(),
            meta: DocumentMeta {
                title: "T".into(),
                author: None,
                source,
                source_url: url.map(str::to_owned),
                language: "en".into(),
                char_count: chars,
                word_count: 0,
                block_count: 0,
            },
            created_at_ms: 0,
            updated_at_ms: 0,
            ready: true,
            progress: offset.map(|o| Progress { char_offset: o, updated_at_ms: 1 }),
        }
    }

    #[test]
    fn domains() {
        assert_eq!(domain("https://www.example.com/a?b"), Some("example.com".into()));
        assert_eq!(domain("http://user@news.site.org:8080/x"), Some("news.site.org".into()));
        assert_eq!(domain("https:///"), None);
    }

    #[test]
    fn lines_for_each_state() {
        // 15 chars per second at 1x: 27,000 chars = 30 min.
        let fresh = doc(SourceKind::Url, Some("https://www.nytimes.com/x"), 27_000, None);
        assert_eq!(meta_line(&fresh, 1.0), "nytimes.com · 30 min");
        let half = doc(SourceKind::Epub, None, 27_000, Some(13_500));
        assert_eq!(meta_line(&half, 1.0), "Book · 15 min left");
        assert_eq!(time_line(&half, 1.5), "10 min left");
        assert!((fraction(&half) - 0.5).abs() < 1e-9);
        let done = doc(SourceKind::Pdf, None, 27_000, Some(26_990));
        assert_eq!(meta_line(&done, 1.0), "PDF · Finished");
        assert_eq!(fraction(&doc(SourceKind::Paste, None, 0, Some(5))), 0.0);
    }
}
