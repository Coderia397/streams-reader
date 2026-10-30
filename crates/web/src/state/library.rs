//! The document list, with local progress laid over the server's (local is written more often).

use std::collections::HashMap;

use leptos::prelude::*;
use nolabs_core::proto::api::{DocumentSummary, Progress};

use super::toast::Toasts;
use crate::api::{ClientError, documents};
use crate::storage::{bodies, progress};

#[derive(Clone, Copy)]
pub struct LibraryState {
    /// `None` until the first answer.
    pub docs: RwSignal<Option<Vec<DocumentSummary>>>,
    pub loading: RwSignal<bool>,
    pub failed: RwSignal<bool>,
}

/// Newest first by last activity (progress or upload).
pub fn sort_docs(docs: &mut [DocumentSummary]) {
    docs.sort_by_key(|d| std::cmp::Reverse(last_activity(d)));
}

pub fn last_activity(d: &DocumentSummary) -> i64 {
    d.progress.map_or(d.updated_at_ms, |p| p.updated_at_ms.max(d.updated_at_ms))
}

/// Started and not finished: the "Continue listening" candidates, most recent first.
pub fn in_progress(docs: &[DocumentSummary]) -> Option<&DocumentSummary> {
    docs.iter()
        .filter(|d| d.progress.is_some_and(|p| p.char_offset > 0 && p.char_offset + 40 < d.meta.char_count))
        .max_by_key(|d| d.progress.map_or(0, |p| p.updated_at_ms))
}

fn overlay(mut docs: Vec<DocumentSummary>, local: &HashMap<String, Progress>) -> Vec<DocumentSummary> {
    for d in &mut docs {
        let l = local.get(&d.id).copied();
        d.progress = match (l, d.progress) {
            (Some(l), Some(s)) => Some(if s.updated_at_ms > l.updated_at_ms { s } else { l }),
            (l, s) => l.or(s),
        };
    }
    sort_docs(&mut docs);
    docs
}

impl LibraryState {
    pub fn new() -> LibraryState {
        LibraryState { docs: RwSignal::new(None), loading: RwSignal::new(false), failed: RwSignal::new(false) }
    }

    pub async fn refresh(self, toasts: Toasts) {
        self.loading.set(true);
        let result = documents::library().await;
        let local: HashMap<String, Progress> =
            progress::all().await.into_iter().map(|p| (p.doc_id.clone(), p.progress())).collect();
        match result {
            Ok(lib) => {
                self.docs.set(Some(overlay(lib.documents, &local)));
                self.failed.set(false);
            }
            Err(e) => {
                self.failed.set(self.docs.with_untracked(Option::is_none));
                toasts.api_error(&e);
            }
        }
        self.loading.set(false);
    }

    pub fn get(&self, id: &str) -> Option<DocumentSummary> {
        self.docs.with_untracked(|d| d.as_ref().and_then(|d| d.iter().find(|x| x.id == id).cloned()))
    }

    pub fn upsert(&self, doc: DocumentSummary) {
        self.docs.update(|d| {
            let list = d.get_or_insert_with(Vec::new);
            list.retain(|x| x.id != doc.id);
            list.insert(0, doc);
        });
    }

    pub fn set_progress(&self, id: &str, p: Progress) {
        self.docs.update(|d| {
            if let Some(doc) = d.as_mut().and_then(|d| d.iter_mut().find(|x| x.id == id)) {
                doc.progress = Some(p);
            }
        });
    }

    pub async fn delete(self, id: String) -> Result<(), ClientError> {
        documents::delete(&id).await?;
        self.docs.update(|d| {
            if let Some(d) = d.as_mut() {
                d.retain(|x| x.id != id);
            }
        });
        bodies::remove(&id).await;
        progress::remove(&id).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::{DocumentMeta, SourceKind};

    use super::*;

    fn doc(id: &str, chars: u64, updated: i64, progress: Option<(u64, i64)>) -> DocumentSummary {
        DocumentSummary {
            id: id.into(),
            meta: DocumentMeta {
                title: id.into(),
                author: None,
                source: SourceKind::Paste,
                source_url: None,
                language: "en".into(),
                char_count: chars,
                word_count: 1,
                block_count: 1,
            },
            created_at_ms: 0,
            updated_at_ms: updated,
            ready: true,
            progress: progress.map(|(o, t)| Progress { char_offset: o, updated_at_ms: t }),
        }
    }

    #[test]
    fn continue_listening_picks_the_latest_unfinished() {
        let docs = vec![
            doc("fresh", 1000, 50, None),
            doc("old", 1000, 1, Some((100, 10))),
            doc("recent", 1000, 1, Some((200, 20))),
            doc("finished", 1000, 1, Some((990, 30))),
        ];
        assert_eq!(in_progress(&docs).map(|d| d.id.as_str()), Some("recent"));
        assert_eq!(in_progress(&docs[..1]), None);
    }

    #[test]
    fn local_progress_overlays_when_newer_and_sorting_follows_activity() {
        let docs = vec![doc("a", 1000, 5, Some((10, 5))), doc("b", 1000, 6, None)];
        let mut local = HashMap::new();
        local.insert("a".to_owned(), Progress { char_offset: 500, updated_at_ms: 100 });
        let out = overlay(docs, &local);
        assert_eq!(out[0].id, "a");
        assert_eq!(out[0].progress.map(|p| p.char_offset), Some(500));
        assert_eq!(out[1].progress, None);
    }
}
