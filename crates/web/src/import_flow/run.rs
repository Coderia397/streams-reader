//! The import pipeline: parse → create → upload body → cache locally → queue the first segments.

use leptos::task::spawn_local;
use nolabs_core::proto::api::{CreateDocument, DocumentSummary, SourceKind};
use nolabs_import::{ImportError, Imported};

use super::meta::{body, build_meta};
use super::source::{FileKind, ImportSource, file_kind, title_from_filename};
use crate::api::{ClientError, documents, fetch};
use crate::interop::pdf;
use crate::provider::oneshot;
use crate::storage::bodies;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Fetching,
    Reading,
    Uploading,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Fetching => "Fetching the page…",
            Stage::Reading => "Reading…",
            Stage::Uploading => "Adding to your library…",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportFailure {
    #[error(transparent)]
    Parse(#[from] ImportError),
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error("{0}")]
    Unsupported(String),
}

impl ImportFailure {
    pub fn friendly(&self) -> String {
        match self {
            ImportFailure::Parse(ImportError::Empty) => "There's no readable text in that.".to_owned(),
            ImportFailure::Parse(ImportError::TooLarge(what)) => format!("That's too large ({what})."),
            ImportFailure::Parse(ImportError::Malformed(_)) => {
                "That file looks damaged and couldn't be read.".to_owned()
            }
            ImportFailure::Parse(ImportError::Unsupported(what)) => format!("Not supported: {what}."),
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

async fn parse(src: ImportSource, on_stage: &dyn Fn(Stage)) -> Result<Parsed, ImportFailure> {
    match src {
        ImportSource::Paste { text, title } => {
            on_stage(Stage::Reading);
            let imported = nolabs_import::from_text(&text, title.as_deref())?;
            Ok(Parsed { imported, source: SourceKind::Paste, source_url: None })
        }
        ImportSource::Link { url } => {
            on_stage(Stage::Fetching);
            let page = fetch::page(&url).await?;
            on_stage(Stage::Reading);
            let imported = nolabs_import::from_html(&page.html, Some(&page.final_url))?;
            Ok(Parsed { imported, source: SourceKind::Url, source_url: Some(page.final_url) })
        }
        ImportSource::File { name, bytes } => {
            on_stage(Stage::Reading);
            let hint = title_from_filename(&name);
            let text = || String::from_utf8_lossy(&bytes).into_owned();
            let (imported, source) = match file_kind(&name) {
                Some(FileKind::Epub) => (nolabs_import::from_epub(&bytes)?, SourceKind::Epub),
                Some(FileKind::Text) => (nolabs_import::from_text(&text(), Some(&hint))?, SourceKind::Text),
                Some(FileKind::Markdown) => (nolabs_import::from_markdown(&text(), Some(&hint))?, SourceKind::Markdown),
                Some(FileKind::Html) => (nolabs_import::from_html(&text(), None)?, SourceKind::Html),
                Some(FileKind::Pdf) => {
                    let pdf = pdf::extract(&bytes)
                        .await
                        .map_err(|e| ImportFailure::Unsupported(format!("Couldn't read that PDF ({e}).")))?;
                    let title = pdf.title.unwrap_or(hint);
                    (nolabs_import::from_pdf_pages(&pdf.pages, Some(&title))?, SourceKind::Pdf)
                }
                None => {
                    return Err(ImportFailure::Unsupported(
                        "That file type isn't supported. Try EPUB, PDF, TXT, Markdown or HTML.".to_owned(),
                    ));
                }
            };
            Ok(Parsed { imported, source, source_url: None })
        }
    }
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
    let summary = documents::create(&CreateDocument { meta, body_bytes: bytes.len() as u64 }).await?;
    documents::put_body(&summary.id, &bytes).await?;
    bodies::save(&summary.id, &doc_body).await;
    let blocks = doc_body.blocks;
    spawn_local(async move {
        oneshot::prerender(&voice, &blocks).await;
    });
    // The create answer predates the upload; the body is in now.
    Ok(DocumentSummary { ready: true, ..summary })
}
