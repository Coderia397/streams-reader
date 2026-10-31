//! The library as the screens use it: one [`DocumentSummary`] per `public.documents` row, with its `public.progress`
//! row folded in. Built from the PostgREST rows in `nolabs_core::proto::api` (times parsed to milliseconds), so the
//! UI never handles RFC 3339 text.

use nolabs_core::proto::api::{DocumentMeta, DocumentRecord, ProgressRecord};
use serde::{Deserialize, Serialize};

use crate::util::time::{format_ms, parse_ms};

/// A listening position: char offset into the reading text, and when it was taken (device clock).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub char_offset: u64,
    pub updated_at_ms: i64,
}

impl Progress {
    pub fn from_record(r: &ProgressRecord) -> Option<Progress> {
        Some(Progress { char_offset: r.char_offset, updated_at_ms: parse_ms(&r.updated_at)? })
    }

    pub fn to_record(self, document_id: &str, device: Option<String>) -> ProgressRecord {
        ProgressRecord {
            document_id: document_id.to_owned(),
            char_offset: self.char_offset,
            updated_at: format_ms(self.updated_at_ms),
            device,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSummary {
    pub id: String,
    pub meta: DocumentMeta,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    /// False until the Worker has stored the body.
    pub ready: bool,
    pub progress: Option<Progress>,
}

impl DocumentSummary {
    pub fn from_record(r: DocumentRecord, progress: Option<Progress>) -> DocumentSummary {
        let created = r.created_at.as_deref().and_then(parse_ms).unwrap_or(0);
        let updated = r.updated_at.as_deref().and_then(parse_ms).unwrap_or(created);
        DocumentSummary {
            id: r.id,
            meta: DocumentMeta {
                title: r.title,
                author: r.author,
                source: r.source,
                source_url: r.source_url,
                language: r.language,
                char_count: r.char_count,
                word_count: r.word_count,
                block_count: r.block_count,
            },
            created_at_ms: created,
            updated_at_ms: updated,
            ready: r.ready,
            progress,
        }
    }
}

/// Joins document rows with progress rows (by `document_id`); progress rows that do not parse are ignored.
pub fn join(docs: Vec<DocumentRecord>, progress: &[ProgressRecord]) -> Vec<DocumentSummary> {
    docs.into_iter()
        .map(|d| {
            let p = progress.iter().filter(|p| p.document_id == d.id).find_map(Progress::from_record);
            DocumentSummary::from_record(d, p)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::SourceKind;

    use super::*;

    fn row(id: &str) -> DocumentRecord {
        let mut r = DocumentRecord::new(
            id.into(),
            DocumentMeta {
                title: "T".into(),
                author: None,
                source: SourceKind::Paste,
                source_url: None,
                language: "en".into(),
                char_count: 100,
                word_count: 20,
                block_count: 2,
            },
            10,
        );
        r.created_at = Some("2026-10-30T10:00:00+00:00".into());
        r.updated_at = Some("2026-10-30T11:00:00.5+00:00".into());
        r.ready = true;
        r
    }

    #[test]
    fn rows_join_with_their_progress() {
        let progress = vec![ProgressRecord {
            document_id: "b".into(),
            char_offset: 42,
            updated_at: "2026-10-30T12:00:00Z".into(),
            device: None,
        }];
        let out = join(vec![row("a"), row("b")], &progress);
        assert_eq!(out[0].progress, None);
        assert_eq!(out[1].progress.map(|p| p.char_offset), Some(42));
        assert_eq!(out[1].updated_at_ms - out[1].created_at_ms, 3_600_500);
        assert!(out[1].ready);
    }

    #[test]
    fn progress_round_trips_through_the_row() {
        let p = Progress { char_offset: 7, updated_at_ms: 1_793_363_696_789 };
        let rec = p.to_record("doc", Some("web".into()));
        assert_eq!(rec.updated_at, "2026-10-30T12:34:56.789Z");
        assert_eq!(Progress::from_record(&rec), Some(p));
    }
}
