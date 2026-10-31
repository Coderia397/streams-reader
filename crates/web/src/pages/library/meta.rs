//! Lines shown under a document's title.

use nolabs_core::proto::api::SourceKind;

use crate::model::DocumentSummary;

use crate::components::icon::I;
use crate::config::UPLOAD_STALE_MS;
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
        SourceKind::Url => d.meta.source_url.as_deref().and_then(domain).unwrap_or_else(|| t!("source-web")),
        SourceKind::Paste => t!("source-text"),
        SourceKind::Epub => t!("source-book"),
        SourceKind::Pdf => t!("source-pdf"),
        SourceKind::Text => t!("source-text-file"),
        SourceKind::Markdown => t!("source-markdown"),
        SourceKind::Html => t!("source-web"),
    }
}

/// The small glyph for a document's source.
pub fn source_icon(d: &DocumentSummary) -> I {
    match d.meta.source {
        SourceKind::Url | SourceKind::Html => I::Article,
        SourceKind::Epub => I::Book,
        SourceKind::Pdf => I::Pdf,
        SourceKind::Paste | SourceKind::Text | SourceKind::Markdown => I::File,
    }
}

/// Text searched by the library filter.
pub fn haystack(d: &DocumentSummary) -> String {
    format!("{} {} {}", d.meta.title, d.meta.author.as_deref().unwrap_or_default(), source_label(d))
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
        return t!("doc-finished");
    }
    match d.progress {
        Some(p) if p.char_offset > 0 => {
            format::time_left(seconds_for_chars(d.meta.char_count.saturating_sub(p.char_offset), rate))
        }
        _ => format::duration_short(seconds_for_chars(d.meta.char_count, rate)),
    }
}

/// Where a document's import stands, as the library row shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Upload {
    Ready,
    /// The body is on its way; the Worker sets `ready` when it lands.
    Preparing,
    /// Still not ready long after the row was written: the upload was cut off.
    Stuck,
}

pub fn upload_state(d: &DocumentSummary, now_ms: i64) -> Upload {
    match d.ready {
        true => Upload::Ready,
        false if now_ms - d.updated_at_ms > UPLOAD_STALE_MS => Upload::Stuck,
        false => Upload::Preparing,
    }
}

/// Milliseconds until a preparing row counts as stuck (0 when it already does).
pub fn ms_until_stuck(d: &DocumentSummary, now_ms: i64) -> i64 {
    (d.updated_at_ms + UPLOAD_STALE_MS - now_ms).max(0)
}

/// Unicode first strong isolate and pop directional isolate: each part keeps its own direction, so in Arabic
/// "9 د" stays beside its number and a Latin domain does not swallow it.
const FSI: char = '\u{2068}';
const PDI: char = '\u{2069}';

fn isolate(part: &str) -> String {
    format!("{FSI}{part}{PDI}")
}

/// "nytimes.com · 30 min", each part isolated for bidi.
pub fn meta_line(d: &DocumentSummary, rate: f64) -> String {
    format!("{} · {}", isolate(&source_label(d)), isolate(&time_line(d, rate)))
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::DocumentMeta;

    use crate::model::Progress;

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

    /// The meta line without its bidi isolates, for comparing the visible words.
    fn plain(s: &str) -> String {
        s.chars().filter(|c| *c != FSI && *c != PDI).collect()
    }

    #[test]
    fn lines_for_each_state() {
        // 15 chars per second at 1x: 27,000 chars = 30 min.
        let fresh = doc(SourceKind::Url, Some("https://www.nytimes.com/x"), 27_000, None);
        assert_eq!(plain(&meta_line(&fresh, 1.0)), "nytimes.com · 30 min");
        let half = doc(SourceKind::Epub, None, 27_000, Some(13_500));
        assert_eq!(plain(&meta_line(&half, 1.0)), "Book · 15 min left");
        assert_eq!(time_line(&half, 1.5), "10 min left");
        assert!((fraction(&half) - 0.5).abs() < 1e-9);
        let done = doc(SourceKind::Pdf, None, 27_000, Some(26_990));
        assert_eq!(plain(&meta_line(&done, 1.0)), "PDF · Finished");
        assert_eq!(fraction(&doc(SourceKind::Paste, None, 0, Some(5))), 0.0);
    }

    #[test]
    fn upload_states() {
        let mut d = doc(SourceKind::Paste, None, 100, None);
        d.updated_at_ms = 1_000;
        assert_eq!(upload_state(&d, 1_000_000), Upload::Ready);
        d.ready = false;
        assert_eq!(upload_state(&d, 1_000 + UPLOAD_STALE_MS), Upload::Preparing);
        assert_eq!(ms_until_stuck(&d, 1_000), UPLOAD_STALE_MS);
        assert_eq!(upload_state(&d, 1_001 + UPLOAD_STALE_MS), Upload::Stuck);
        assert_eq!(ms_until_stuck(&d, 5_000 + UPLOAD_STALE_MS), 0);
    }

    #[test]
    fn meta_line_isolates_each_part() {
        let d = doc(SourceKind::Url, Some("https://essays.example.org/a"), 27_000, None);
        assert_eq!(meta_line(&d, 1.0), "\u{2068}essays.example.org\u{2069} · \u{2068}30 min\u{2069}");
    }
}
