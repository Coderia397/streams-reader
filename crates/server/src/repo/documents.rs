use nolabs_core::proto::api::{DocumentMeta, DocumentSummary, Progress};

use super::row::{self, Row};
use super::{Repo, sql};
use crate::ports::PortError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRow {
    pub id: String,
    pub meta: DocumentMeta,
    pub body_bytes: u64,
    pub ready: bool,
    pub progress: Option<Progress>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl DocumentRow {
    fn from_row(r: &Row) -> Result<Self, PortError> {
        let progress = match (row::opt_u64(r, "progress_offset")?, row::opt_i64(r, "progress_at_ms")?) {
            (Some(char_offset), Some(updated_at_ms)) => Some(Progress { char_offset, updated_at_ms }),
            _ => None,
        };
        Ok(DocumentRow {
            id: row::str(r, "id")?,
            meta: row::json(r, "meta_json")?,
            body_bytes: row::u64(r, "body_bytes")?,
            ready: row::bool(r, "ready")?,
            progress,
            created_at_ms: row::i64(r, "created_at_ms")?,
            updated_at_ms: row::i64(r, "updated_at_ms")?,
        })
    }

    pub fn summary(&self) -> DocumentSummary {
        DocumentSummary {
            id: self.id.clone(),
            meta: self.meta.clone(),
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
            ready: self.ready,
            progress: self.progress,
        }
    }
}

impl Repo<'_> {
    pub async fn insert_document(&self, doc: &DocumentRow) -> Result<(), PortError> {
        let meta = serde_json::to_string(&doc.meta).map_err(|e| PortError::new(e.to_string()))?;
        self.run(
            sql::DOC_INSERT,
            vec![
                doc.id.as_str().into(),
                meta.into(),
                doc.body_bytes.into(),
                doc.created_at_ms.into(),
                doc.updated_at_ms.into(),
            ],
        )
        .await?;
        Ok(())
    }

    pub async fn get_document(&self, id: &str) -> Result<Option<DocumentRow>, PortError> {
        self.all(sql::DOC_GET, vec![id.into()]).await?.first().map(DocumentRow::from_row).transpose()
    }

    pub async fn list_documents(&self) -> Result<Vec<DocumentRow>, PortError> {
        self.all(sql::DOC_LIST, vec![]).await?.iter().map(DocumentRow::from_row).collect()
    }

    /// Returns false when the document does not exist.
    pub async fn set_document_ready(&self, id: &str, now_ms: i64) -> Result<bool, PortError> {
        Ok(self.run(sql::DOC_SET_READY, vec![now_ms.into(), id.into()]).await? > 0)
    }

    pub async fn delete_document(&self, id: &str) -> Result<bool, PortError> {
        Ok(self.run(sql::DOC_DELETE, vec![id.into()]).await? > 0)
    }

    /// Returns false when the document is missing or already has newer progress.
    pub async fn set_progress(&self, id: &str, progress: Progress) -> Result<bool, PortError> {
        let changed = self
            .run(
                sql::DOC_SET_PROGRESS,
                vec![
                    progress.char_offset.into(),
                    progress.updated_at_ms.into(),
                    id.into(),
                    progress.updated_at_ms.into(),
                ],
            )
            .await?;
        Ok(changed > 0)
    }
}
