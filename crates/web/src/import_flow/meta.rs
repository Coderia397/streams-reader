//! Document metadata and the exact body bytes that are uploaded.

use nolabs_core::proto::api::{Block, DOCUMENT_BODY_VERSION, DocumentBody, DocumentMeta, SourceKind};
use nolabs_core::text::{reading_text, word_count};
use nolabs_import::Imported;

const MAX_TITLE_CHARS: usize = 200;

pub fn clean_title(title: &str) -> String {
    let collapsed = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let t: String = collapsed.chars().take(MAX_TITLE_CHARS).collect();
    if t.is_empty() { "Untitled".to_owned() } else { t }
}

pub fn build_meta(imported: &Imported, source: SourceKind, source_url: Option<String>) -> DocumentMeta {
    let text = reading_text(&imported.blocks);
    DocumentMeta {
        title: clean_title(&imported.title),
        author: imported.author.as_deref().map(str::trim).filter(|a| !a.is_empty()).map(str::to_owned),
        source,
        source_url,
        language: "en".to_owned(),
        char_count: text.chars().count() as u64,
        word_count: word_count(&text) as u64,
        block_count: imported.blocks.len() as u32,
    }
}

pub fn body(blocks: Vec<Block>) -> DocumentBody {
    DocumentBody { v: DOCUMENT_BODY_VERSION, blocks }
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::BlockKind;

    use super::*;

    #[test]
    fn meta_counts_the_reading_text() {
        let imported = Imported {
            title: "  A   Title \n".into(),
            author: Some("  ".into()),
            blocks: vec![
                Block { kind: BlockKind::Heading, text: "Hi".into() },
                Block { kind: BlockKind::Paragraph, text: "Two words.".into() },
            ],
        };
        let m = build_meta(&imported, SourceKind::Paste, None);
        assert_eq!(m.title, "A Title");
        assert_eq!(m.author, None);
        assert_eq!(m.char_count, 2 + 2 + 10);
        assert_eq!(m.word_count, 3);
        assert_eq!(m.block_count, 2);
        assert_eq!(m.language, "en");
    }

    #[test]
    fn titles_are_trimmed_and_capped() {
        assert_eq!(clean_title(""), "Untitled");
        assert_eq!(clean_title(&"x".repeat(500)).chars().count(), MAX_TITLE_CHARS);
    }

    #[test]
    fn body_has_the_current_version() {
        assert_eq!(body(vec![]).v, DOCUMENT_BODY_VERSION);
    }
}
