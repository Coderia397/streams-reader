//! The import pipeline: parse (in the import worker, `worker.rs`) → insert the Supabase row (client id, `ready = false`) → cache the body locally → PUT
//! it to the Worker → queue the first segments. A failed upload removes the row again, so no half-imported document
//! stays in the library.

use leptos::task::spawn_local;
use nolabs_core::ids::id_from_bytes;
use nolabs_core::proto::api::{DocumentRecord, SourceKind};
use nolabs_web_import::{Fault, Job, Parsed as Imported};

use super::meta::{body, build_meta};
use super::source::{FileKind, ImportSource, file_kind, title_from_filename};
use super::worker;
use crate::api::{ClientError, documents, fetch};
use crate::db;
use crate::interop::pdf;
use crate::model::DocumentSummary;
use crate::provider::oneshot;
use crate::storage::bodies;
use crate::util::dom;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Fetching,
    Reading,
    Uploading,
}

impl Stage {
    pub fn label(self) -> String {
        match self {
            Stage::Fetching => t!("import-stage-fetching"),
            Stage::Reading => t!("import-stage-reading"),
            Stage::Uploading => t!("import-stage-uploading"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportFailure {
    #[error("{0:?}")]
    Parse(Fault),
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error("{0}")]
    Unsupported(String),
}

impl ImportFailure {
    pub fn friendly(&self) -> String {
        match self {
            ImportFailure::Parse(Fault::Empty) => t!("import-error-empty"),
            ImportFailure::Parse(Fault::TooLarge(what)) => t!("import-error-too-large", detail = what.clone()),
            ImportFailure::Parse(Fault::Malformed(_)) => t!("import-error-damaged"),
            ImportFailure::Parse(Fault::Unsupported(what)) => t!("import-error-unsupported", detail = what.clone()),
            ImportFailure::Parse(Fault::Worker(_)) => t!("import-error-worker"),
            ImportFailure::Client(e) => e.friendly(),
            ImportFailure::Unsupported(msg) => msg.clone(),
        }
    }
}

struct Parsed {
    imported: Imported,
    source: SourceKind,
    source_url: Option<String>,
}

/// Parses in the import worker.
async fn run(job: Job, bytes: Option<Vec<u8>>) -> Result<Imported, ImportFailure> {
    worker::parse(job, bytes).await.map_err(ImportFailure::Parse)
}

async fn parse(src: ImportSource, on_stage: &dyn Fn(Stage)) -> Result<Parsed, ImportFailure> {
    match src {
        ImportSource::Paste { text, title } => {
            on_stage(Stage::Reading);
            let imported = run(Job::Text { text, title }, None).await?;
            Ok(Parsed { imported, source: SourceKind::Paste, source_url: None })
        }
        ImportSource::Link { url } => {
            on_stage(Stage::Fetching);
            worker::warm();
            let page = fetch::page(&url).await?;
            on_stage(Stage::Reading);
            let imported = run(Job::Html { html: page.html, url: Some(page.final_url.clone()) }, None).await?;
            Ok(Parsed { imported, source: SourceKind::Url, source_url: Some(page.final_url) })
        }
        ImportSource::File { name, bytes } => {
            on_stage(Stage::Reading);
            let hint = title_from_filename(&name);
            let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
            let (job, bytes, source) = match file_kind(&name) {
                Some(FileKind::Epub) => (Job::Epub, Some(bytes), SourceKind::Epub),
                Some(FileKind::Text) => (Job::Text { text: text(&bytes), title: Some(hint) }, None, SourceKind::Text),
                Some(FileKind::Markdown) => {
                    (Job::Markdown { text: text(&bytes), title: Some(hint) }, None, SourceKind::Markdown)
                }
                Some(FileKind::Html) => (Job::Html { html: text(&bytes), url: None }, None, SourceKind::Html),
                Some(FileKind::Pdf) => {
                    worker::warm();
                    let pdf = pdf::extract(&bytes)
                        .await
                        .map_err(|e| ImportFailure::Unsupported(t!("import-error-pdf", detail = e.to_string())))?;
                    let title = pdf.title.unwrap_or(hint);
                    (Job::PdfPages { pages: pdf.pages, title: Some(title) }, None, SourceKind::Pdf)
                }
                None => {
                    return Err(ImportFailure::Unsupported(t!("import-error-file-type")));
                }
            };
            let imported = run(job, bytes).await?;
            Ok(Parsed { imported, source, source_url: None })
        }
    }
}

/// A new 16-char document id (`nolabs_core::ids`) from 10 random bytes.
fn new_id() -> Option<String> {
    let mut bytes = [0u8; 10];
    dom::window()?.crypto().ok()?.get_random_values_with_u8_array(&mut bytes).ok()?;
    Some(id_from_bytes(bytes))
}

/// Imports and uploads; `voice` is the default voice used to pre-render the opening.
pub async fn import(
    src: ImportSource,
    voice: String,
    on_stage: &dyn Fn(Stage),
) -> Result<DocumentSummary, ImportFailure> {
    let Parsed { imported, source, source_url } = parse(src, on_stage).await?;
    on_stage(Stage::Uploading);
    let meta = build_meta(&imported, source, source_url);
    let doc_body = body(imported.blocks);
    let bytes = serde_json::to_vec(&doc_body).map_err(|e| ClientError::Decode(e.to_string()))?;
    let id = new_id().ok_or_else(|| ClientError::Browser("no random source".into()))?;
    let summary = db::library::insert(&DocumentRecord::new(id, meta, bytes.len() as u64)).await?;
    // Cached before the upload, so a row left behind (tab closed mid-upload) can be retried from the library.
    bodies::save(&summary.id, &doc_body).await;
    if let Err(e) = documents::put_body(&summary.id, &bytes).await {
        discard(&summary.id).await;
        return Err(e.into());
    }
    let blocks = doc_body.blocks;
    spawn_local(async move {
        oneshot::prerender(&voice, &blocks).await;
    });
    // The inserted row predates the upload; the Worker has set `ready` by now.
    Ok(DocumentSummary { ready: true, ..summary })
}

/// Best effort: removes the row (and anything the Worker stored) and then the local copy after a failed upload. If
/// the delete fails too, the local copy stays, and the library shows the row as an unfinished upload with Retry
/// (which re-sends that copy) and Delete.
async fn discard(id: &str) {
    if documents::delete(id).await.is_ok() {
        bodies::remove(id).await;
    }
}
