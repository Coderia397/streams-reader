//! Runs one [`Job`] with `nolabs-import` (inside the worker; native in tests).

use nolabs_import::{ImportError, Imported};

use crate::{Fault, Job, Parsed, PdfLine, Reply};

impl From<ImportError> for Fault {
    fn from(e: ImportError) -> Fault {
        match e {
            ImportError::Empty => Fault::Empty,
            ImportError::TooLarge(what) => Fault::TooLarge(what.to_owned()),
            ImportError::Malformed(why) => Fault::Malformed(why),
            ImportError::Unsupported(what) => Fault::Unsupported(what),
        }
    }
}

fn parsed(i: Imported) -> Parsed {
    Parsed { title: i.title, author: i.author, blocks: i.blocks }
}

fn pdf_line(l: PdfLine) -> nolabs_import::PdfLine {
    nolabs_import::PdfLine { text: l.text, y: l.y, height: l.height }
}

/// Parses `job`; `bytes` is the file for [`Job::Epub`] and ignored otherwise.
pub fn run(job: Job, bytes: &[u8]) -> Reply {
    let imported = match job {
        Job::Text { text, title } => nolabs_import::from_text(&text, title.as_deref()),
        Job::Markdown { text, title } => nolabs_import::from_markdown(&text, title.as_deref()),
        Job::Html { html, url } => nolabs_import::from_html(&html, url.as_deref()),
        Job::Epub => nolabs_import::from_epub(bytes),
        Job::PdfPages { pages, title } => {
            let pages: Vec<Vec<nolabs_import::PdfLine>> =
                pages.into_iter().map(|p| p.into_iter().map(pdf_line).collect()).collect();
            nolabs_import::from_pdf_pages(&pages, title.as_deref())
        }
    };
    imported.map(parsed).map_err(Fault::from)
}

/// The JSON reply for a JSON job, as the worker posts it. Never fails: an unreadable job becomes a [`Fault`].
pub fn run_json(job: &str, bytes: &[u8]) -> String {
    let reply: Reply = match serde_json::from_str::<Job>(job) {
        Ok(job) => run(job, bytes),
        Err(e) => Err(Fault::Worker(format!("unreadable job: {e}"))),
    };
    serde_json::to_string(&reply).unwrap_or_else(|e| format!(r#"{{"Err":{{"kind":"worker","detail":"{e}"}}}}"#))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_html_and_errors_go_through() {
        let r = run(Job::Text { text: "Title line\n\nA paragraph of text.".into(), title: None }, &[]);
        assert!(r.as_ref().is_ok_and(|p| !p.blocks.is_empty()), "{r:?}");

        let html = "<html><head><title>Page</title></head><body><article><p>Some words to read aloud here, \
                    enough of them to count as an article body for the extractor.</p></article></body></html>";
        let r = run(Job::Html { html: html.into(), url: Some("https://example.com/a".into()) }, &[]);
        assert!(r.is_ok(), "{r:?}");

        assert_eq!(run(Job::Text { text: "   ".into(), title: None }, &[]), Err(Fault::Empty));
        assert!(matches!(run(Job::Epub, b"not a zip"), Err(Fault::Malformed(_) | Fault::Unsupported(_))));
    }

    #[test]
    fn json_round_trip_and_bad_jobs() {
        let reply = run_json(r#"{"kind":"text","text":"Hello there.","title":"Hi"}"#, &[]);
        let back: Reply = serde_json::from_str(&reply).unwrap_or(Err(Fault::Empty));
        assert_eq!(back.map(|p| p.title).ok().as_deref(), Some("Hi"));

        let reply = run_json("{not json", &[]);
        assert!(matches!(serde_json::from_str::<Reply>(&reply), Ok(Err(Fault::Worker(_)))));
    }
}
