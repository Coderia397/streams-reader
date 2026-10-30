//! Document importers. Every importer returns plain-text blocks already passed through
//! `nolabs_core::text::normalise_blocks`; no markup survives import.
//!
//! All parsing runs in the web app (wasm32) so the Worker never spends CPU on it; every dependency is pure Rust.
//! Inputs are untrusted: sizes are capped (see [`limits`]) and EPUB archives are checked against zip bombs.

mod common;
mod dom;
mod epub;
mod html;
pub mod limits;
mod markdown;
mod pdf;
mod text;
mod tree_blocks;

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

/// One line of PDF text as extracted by pdf.js in the web app.
///
/// Lines of a page are given in reading order. `y` is the line's vertical position in page units and `height` its font
/// height in the same units; only differences between `y` values are used, so either axis direction works as long as
/// it is the same for every page of the document.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfLine {
    pub text: String,
    pub y: f32,
    pub height: f32,
}

/// Plain text: paragraphs split on blank lines. The title is `title_hint`, else the first line when it is short.
pub fn from_text(text: &str, title_hint: Option<&str>) -> Result<Imported, ImportError> {
    text::import(text, title_hint)
}

/// CommonMark with GFM tables and footnotes. Code blocks and tables become a short "skipped" paragraph.
pub fn from_markdown(md: &str, title_hint: Option<&str>) -> Result<Imported, ImportError> {
    markdown::import(md, title_hint)
}

/// A web page: the article is found with readability (dom_smoothie), then split into blocks.
pub fn from_html(html: &str, url: Option<&str>) -> Result<Imported, ImportError> {
    html::import(html, url)
}

/// An EPUB 2 or 3 book, read in spine order.
pub fn from_epub(bytes: &[u8]) -> Result<Imported, ImportError> {
    epub::import(bytes)
}

/// Text lines of a PDF, one `Vec` per page. Running headers, footers and page numbers are dropped and paragraphs are
/// rebuilt from line spacing.
pub fn from_pdf_pages(pages: &[Vec<PdfLine>], title_hint: Option<&str>) -> Result<Imported, ImportError> {
    pdf::import(pages, title_hint)
}
