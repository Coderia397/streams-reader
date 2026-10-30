//! Document importers. Every importer returns plain-text blocks already passed through
//! `nolabs_core::text::normalise_blocks`; no markup survives import.

use nolabs_core::proto::api::Block;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    pub title: String,
    pub author: Option<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    #[error("there is no readable text in this file")]
    Empty,
    #[error("too large: {0}")]
    TooLarge(&'static str),
    #[error("could not read the file: {0}")]
    Malformed(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfLine {
    pub text: String,
    pub y: f32,
    pub height: f32,
}

pub fn from_text(text: &str, title_hint: Option<&str>) -> Result<Imported, ImportError> {
    let _ = (text, title_hint);
    todo!("implemented by the import agent")
}

pub fn from_markdown(md: &str, title_hint: Option<&str>) -> Result<Imported, ImportError> {
    let _ = (md, title_hint);
    todo!("implemented by the import agent")
}

pub fn from_html(html: &str, url: Option<&str>) -> Result<Imported, ImportError> {
    let _ = (html, url);
    todo!("implemented by the import agent")
}

pub fn from_epub(bytes: &[u8]) -> Result<Imported, ImportError> {
    let _ = bytes;
    todo!("implemented by the import agent")
}

pub fn from_pdf_pages(pages: &[Vec<PdfLine>], title_hint: Option<&str>) -> Result<Imported, ImportError> {
    let _ = (pages, title_hint);
    todo!("implemented by the import agent")
}
