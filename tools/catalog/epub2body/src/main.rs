//! `epub2body <in.epub> <out.json> [<in.epub> <out.json> ...]`
//!
//! Imports each EPUB with `nolabs_import::from_epub` (the importer the web app runs), writes the body exactly as the
//! app uploads it (`serde_json::to_vec(&DocumentBody)`) and prints one JSON line of metadata per book to stdout, with
//! the same counts as the app's `import_flow::meta::build_meta`. Exits 1 if any book failed; the others are still
//! written.

use std::process::ExitCode;

use nolabs_core::proto::api::{Block, BlockKind, DOCUMENT_BODY_VERSION, DocumentBody};
use nolabs_core::text::{reading_text, word_count};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize, PartialEq, Eq)]
struct Meta {
    title: String,
    author: Option<String>,
    char_count: u64,
    word_count: u64,
    block_count: u32,
    heading_count: u32,
    body_bytes: u64,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct Line<'a> {
    input: &'a str,
    output: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<Meta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// The body bytes and their metadata.
fn convert(epub: &[u8]) -> Result<(Vec<u8>, Meta), String> {
    let imported = nolabs_import::from_epub(epub).map_err(|e| e.to_string())?;
    let body = DocumentBody { v: DOCUMENT_BODY_VERSION, blocks: imported.blocks };
    let bytes = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
    let meta = meta_of(&imported.title, imported.author.as_deref(), &body.blocks, &bytes);
    Ok((bytes, meta))
}

fn meta_of(title: &str, author: Option<&str>, blocks: &[Block], bytes: &[u8]) -> Meta {
    let text = reading_text(blocks);
    Meta {
        title: title.split_whitespace().collect::<Vec<_>>().join(" "),
        author: author.map(str::trim).filter(|a| !a.is_empty()).map(str::to_owned),
        char_count: text.chars().count() as u64,
        word_count: word_count(&text) as u64,
        block_count: blocks.len() as u32,
        heading_count: blocks.iter().filter(|b| b.kind == BlockKind::Heading).count() as u32,
        body_bytes: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(bytes)),
    }
}

fn run_one(input: &str, output: &str) -> Result<Meta, String> {
    let epub = std::fs::read(input).map_err(|e| format!("read {input}: {e}"))?;
    let (bytes, meta) = convert(&epub)?;
    std::fs::write(output, &bytes).map_err(|e| format!("write {output}: {e}"))?;
    Ok(meta)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.len() % 2 != 0 {
        eprintln!("usage: epub2body <in.epub> <out.json> [<in.epub> <out.json> ...]");
        return ExitCode::from(2);
    }
    let mut failed = false;
    for pair in args.chunks(2) {
        let (input, output) = (pair[0].as_str(), pair[1].as_str());
        let line = match run_one(input, output) {
            Ok(meta) => Line { input, output, meta: Some(meta), error: None },
            Err(e) => {
                failed = true;
                Line { input, output, meta: None, error: Some(e) }
            }
        };
        match serde_json::to_string(&line) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("{input}: {e}");
                failed = true;
            }
        }
    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(kind: BlockKind, text: &str) -> Block {
        Block { kind, text: text.into() }
    }

    #[test]
    fn meta_counts_the_reading_text_like_the_app() {
        let blocks = vec![block(BlockKind::Heading, "Hi"), block(BlockKind::Paragraph, "One two three.")];
        let m = meta_of("  A   Title ", Some("  "), &blocks, b"{}");
        assert_eq!(m.title, "A Title");
        assert_eq!(m.author, None);
        // "Hi" + "\n\n" + "One two three."
        assert_eq!(m.char_count, 2 + 2 + 14);
        assert_eq!(m.word_count, 4);
        assert_eq!(m.block_count, 2);
        assert_eq!(m.heading_count, 1);
        assert_eq!(m.body_bytes, 2);
        assert_eq!(m.sha256, "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a");
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(convert(b"not a zip").is_err());
    }
}
