//! The import worker's message contract, shared by the app (`nolabs-web`) and the worker (bin
//! `nolabs-import-worker`, feature `worker`).
//!
//! Parsing HTML, EPUB, Markdown and PDF text needs `nolabs-import` (an HTML parser with its entity table, a zip
//! reader, an XML reader), which is most of the importer's weight. The app does not link it: it posts a [`Job`] to a
//! Web Worker that loads only when something is imported, and gets a [`Reply`] back.
//!
//! On the wire every message is a plain object. App → worker: `{ id, job, bytes? }`, where `job` is the JSON of a
//! [`Job`] and `bytes` an optional transferred `Uint8Array` (EPUB files). Worker → app: `{ ready: true }` once, when
//! its handler is installed, then `{ id, reply }` per job, where `reply` is the JSON of a [`Reply`].

use nolabs_core::proto::api::Block;
use serde::{Deserialize, Serialize};

#[cfg(feature = "worker")]
pub mod job;

/// Message field names.
pub mod field {
    pub const ID: &str = "id";
    pub const JOB: &str = "job";
    pub const BYTES: &str = "bytes";
    pub const REPLY: &str = "reply";
    pub const READY: &str = "ready";
}

/// One line of PDF text (from pdf.js in the page) with its position; see `nolabs_import::PdfLine`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfLine {
    pub text: String,
    pub y: f32,
    pub height: f32,
}

/// What to parse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Job {
    Text {
        text: String,
        title: Option<String>,
    },
    Markdown {
        text: String,
        title: Option<String>,
    },
    /// A web page or an HTML file; `url` resolves relative links.
    Html {
        html: String,
        url: Option<String>,
    },
    /// The file travels beside the job as `bytes`.
    Epub,
    PdfPages {
        pages: Vec<Vec<PdfLine>>,
        title: Option<String>,
    },
}

/// A parsed document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parsed {
    pub title: String,
    pub author: Option<String>,
    pub blocks: Vec<Block>,
}

/// Why parsing failed; mirrors `nolabs_import::ImportError`, plus the worker failing itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum Fault {
    Empty,
    TooLarge(String),
    Malformed(String),
    Unsupported(String),
    /// The worker could not start, crashed or sent something unreadable.
    Worker(String),
}

pub type Reply = Result<Parsed, Fault>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jobs_and_replies_survive_json() {
        let job = Job::PdfPages {
            pages: vec![vec![PdfLine { text: "Hello".into(), y: 10.5, height: 12.0 }]],
            title: Some("T".into()),
        };
        let json = serde_json::to_string(&job).unwrap_or_default();
        assert!(json.contains("\"kind\":\"pdf_pages\""));
        assert_eq!(serde_json::from_str::<Job>(&json).ok(), Some(job));

        let reply: Reply = Err(Fault::TooLarge("text is larger than 64 MB".into()));
        let json = serde_json::to_string(&reply).unwrap_or_default();
        assert_eq!(serde_json::from_str::<Reply>(&json).ok(), Some(reply));
    }
}
