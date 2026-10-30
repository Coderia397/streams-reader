//! Documents: metadata rows in D1, bodies in R2. A body upload is two-phase: the row is created with `ready = false`,
//! the Worker streams the PUT body to R2 with the declared length, then the row becomes ready. A crash in between
//! leaves a row that is not ready, which the app can upload to again.

use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::{CreateDocument, DocumentMeta, DocumentSummary, Library, Progress};

use crate::app::App;
use crate::error::{conflict, too_large};
use crate::repo::DocumentRow;

/// Largest document body accepted (normalised text as JSON; a long novel is about 2 MB).
pub const MAX_BODY_BYTES: u64 = 32 * 1024 * 1024;

/// Where the Worker must stream an upload, and the exact length it must have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadTarget {
    pub object_key: String,
    pub length: u64,
}

pub fn body_key(id: &str) -> String {
    format!("docs/{id}/body.json")
}

fn validate_meta(meta: &DocumentMeta) -> Result<(), ApiError> {
    let title = meta.title.trim();
    if title.is_empty() || title.chars().count() > 500 {
        return Err(ApiError::bad_request("title must have 1 to 500 characters"));
    }
    if meta.author.as_ref().is_some_and(|a| a.chars().count() > 500) {
        return Err(ApiError::bad_request("author is too long"));
    }
    if meta.source_url.as_ref().is_some_and(|u| u.len() > 4096) {
        return Err(ApiError::bad_request("source_url is too long"));
    }
    if meta.language.is_empty() || meta.language.len() > 35 {
        return Err(ApiError::bad_request("language must be a BCP 47 tag"));
    }
    Ok(())
}

/// Check that a streamed upload declares exactly the expected length (R2 needs a known length up front).
pub fn check_length(content_length: Option<u64>, expected: u64) -> Result<(), ApiError> {
    match content_length {
        Some(n) if n == expected => Ok(()),
        Some(n) => {
            Err(ApiError::bad_request(format!("Content-Length {n} does not match the declared {expected} bytes")))
        }
        None => Err(ApiError::bad_request("Content-Length is required")),
    }
}

impl App {
    pub async fn create_document(&self, req: CreateDocument) -> Result<DocumentSummary, ApiError> {
        validate_meta(&req.meta)?;
        if req.body_bytes == 0 {
            return Err(ApiError::bad_request("body_bytes must be positive"));
        }
        if req.body_bytes > MAX_BODY_BYTES {
            return Err(too_large(format!("documents are limited to {MAX_BODY_BYTES} bytes")));
        }
        let now = self.now_ms();
        let doc = DocumentRow {
            id: self.new_id()?,
            meta: req.meta,
            body_bytes: req.body_bytes,
            ready: false,
            progress: None,
            created_at_ms: now,
            updated_at_ms: now,
        };
        self.repo().insert_document(&doc).await?;
        Ok(doc.summary())
    }

    pub async fn library(&self) -> Result<Library, ApiError> {
        let documents = self.repo().list_documents().await?.iter().map(DocumentRow::summary).collect();
        Ok(Library { documents })
    }

    async fn document(&self, id: &str) -> Result<DocumentRow, ApiError> {
        self.repo().get_document(id).await?.ok_or_else(|| ApiError::not_found("no such document"))
    }

    pub async fn get_document(&self, id: &str) -> Result<DocumentSummary, ApiError> {
        Ok(self.document(id).await?.summary())
    }

    /// Validate a body upload before the Worker streams it.
    pub async fn begin_document_body(&self, id: &str, content_length: Option<u64>) -> Result<UploadTarget, ApiError> {
        let doc = self.document(id).await?;
        if doc.ready {
            return Err(conflict("the body of this document was already uploaded"));
        }
        check_length(content_length, doc.body_bytes)?;
        Ok(UploadTarget { object_key: body_key(id), length: doc.body_bytes })
    }

    /// Mark the body uploaded after R2 accepted it.
    pub async fn finish_document_body(&self, id: &str) -> Result<(), ApiError> {
        if self.repo().set_document_ready(id, self.now_ms()).await? {
            Ok(())
        } else {
            Err(ApiError::not_found("document deleted during upload"))
        }
    }

    /// R2 key of a ready document's body.
    pub async fn document_body_key(&self, id: &str) -> Result<String, ApiError> {
        let doc = self.document(id).await?;
        if !doc.ready {
            return Err(conflict("the document body has not been uploaded yet"));
        }
        Ok(body_key(id))
    }

    /// Delete the row first (the document disappears at once), then every object under `docs/<id>/`.
    pub async fn delete_document(&self, id: &str) -> Result<(), ApiError> {
        if !self.repo().delete_document(id).await? {
            return Err(ApiError::not_found("no such document"));
        }
        let keys = self.blob.list_prefix(&format!("docs/{id}/")).await?;
        self.blob.delete_many(&keys).await?;
        Ok(())
    }

    /// Save reading progress. Older progress than what is stored is ignored (not an error).
    pub async fn put_progress(&self, id: &str, progress: Progress) -> Result<(), ApiError> {
        if self.repo().set_progress(id, progress).await? {
            return Ok(());
        }
        self.document(id).await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestEnv, run};
    use nolabs_core::proto::ErrorCode;
    use nolabs_core::proto::api::SourceKind;

    fn create(title: &str, bytes: u64) -> CreateDocument {
        CreateDocument {
            meta: DocumentMeta {
                title: title.into(),
                author: None,
                source: SourceKind::Paste,
                source_url: None,
                language: "en".into(),
                char_count: 100,
                word_count: 20,
                block_count: 2,
            },
            body_bytes: bytes,
        }
    }

    #[test]
    fn create_upload_and_read_back() {
        let env = TestEnv::new();
        run(async {
            let doc = env.app.create_document(create("A book", 42)).await.unwrap();
            assert!(!doc.ready);
            assert_eq!(env.app.document_body_key(&doc.id).await.unwrap_err().code, ErrorCode::Conflict);
            assert_eq!(env.app.begin_document_body(&doc.id, None).await.unwrap_err().code, ErrorCode::BadRequest);
            assert!(env.app.begin_document_body(&doc.id, Some(41)).await.is_err());
            let target = env.app.begin_document_body(&doc.id, Some(42)).await.unwrap();
            assert_eq!(target, UploadTarget { object_key: format!("docs/{}/body.json", doc.id), length: 42 });
            env.advance(5);
            env.app.finish_document_body(&doc.id).await.unwrap();
            let got = env.app.get_document(&doc.id).await.unwrap();
            assert!(got.ready);
            assert_eq!(got.updated_at_ms, env.now());
            assert_eq!(env.app.document_body_key(&doc.id).await.unwrap(), target.object_key);
            assert_eq!(env.app.begin_document_body(&doc.id, Some(42)).await.unwrap_err().code, ErrorCode::Conflict);
        });
    }

    #[test]
    fn validation() {
        let env = TestEnv::new();
        run(async {
            assert_eq!(env.app.create_document(create("  ", 1)).await.unwrap_err().code, ErrorCode::BadRequest);
            assert_eq!(env.app.create_document(create("x", 0)).await.unwrap_err().code, ErrorCode::BadRequest);
            let e = env.app.create_document(create("x", MAX_BODY_BYTES + 1)).await.unwrap_err();
            assert_eq!(e.code, ErrorCode::PayloadTooLarge);
            assert_eq!(env.app.get_document("0000000000000000").await.unwrap_err().code, ErrorCode::NotFound);
        });
    }

    #[test]
    fn library_orders_by_last_activity_and_progress_is_last_writer_by_time() {
        let env = TestEnv::new();
        run(async {
            let a = env.app.create_document(create("A", 1)).await.unwrap();
            env.advance(1000);
            let b = env.app.create_document(create("B", 1)).await.unwrap();
            let lib = env.app.library().await.unwrap();
            assert_eq!(lib.documents.iter().map(|d| d.meta.title.as_str()).collect::<Vec<_>>(), ["B", "A"]);

            let p = Progress { char_offset: 10, updated_at_ms: env.now() + 5 };
            env.app.put_progress(&a.id, p).await.unwrap();
            env.app.put_progress(&a.id, Progress { char_offset: 3, updated_at_ms: env.now() }).await.unwrap();
            let lib = env.app.library().await.unwrap();
            assert_eq!(lib.documents[0].id, a.id);
            assert_eq!(lib.documents[0].progress, Some(p));
            assert_eq!(lib.documents[1].id, b.id);

            let e = env.app.put_progress("0000000000000000", p).await.unwrap_err();
            assert_eq!(e.code, ErrorCode::NotFound);
        });
    }

    #[test]
    fn delete_removes_row_and_objects() {
        let env = TestEnv::new();
        run(async {
            let a = env.app.create_document(create("A", 1)).await.unwrap();
            env.blob.insert(&body_key(&a.id), "{}");
            env.blob.insert("docs/other/body.json", "{}");
            env.app.delete_document(&a.id).await.unwrap();
            assert_eq!(env.blob.keys(), vec!["docs/other/body.json".to_owned()]);
            assert_eq!(env.app.get_document(&a.id).await.unwrap_err().code, ErrorCode::NotFound);
            assert_eq!(env.app.delete_document(&a.id).await.unwrap_err().code, ErrorCode::NotFound);
        });
    }
}
