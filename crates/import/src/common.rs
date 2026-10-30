//! Helpers shared by every importer: block accumulation, title fallback and the final normalisation pass.

use nolabs_core::proto::api::{Block, BlockKind};
use nolabs_core::text::normalise_blocks;

use crate::limits::MAX_TEXT_BYTES;
use crate::{ImportError, Imported};

/// Spoken in place of a code block, so the listener knows something was left out.
pub(crate) const CODE_SKIPPED: &str = "Code block skipped.";
/// Spoken in place of a data table.
pub(crate) const TABLE_SKIPPED: &str = "Table skipped.";
/// Title used when nothing better can be found.
pub(crate) const UNTITLED: &str = "Untitled";

/// A first line up to this many chars can serve as the title.
const SHORT_LINE_CHARS: usize = 80;
/// A title derived from a long first block is cut near this many chars.
const DERIVED_TITLE_CHARS: usize = 60;

/// Collects text for the block being built and the finished blocks before it.
#[derive(Debug, Default)]
pub(crate) struct BlockBuffer {
    blocks: Vec<Block>,
    text: String,
}

impl BlockBuffer {
    pub(crate) fn push_str(&mut self, s: &str) {
        self.text.push_str(s);
    }

    pub(crate) fn push_char(&mut self, c: char) {
        self.text.push(c);
    }

    /// Ends the current block as `kind`; whitespace-only text is discarded.
    pub(crate) fn flush(&mut self, kind: BlockKind) {
        if self.text.trim().is_empty() {
            self.text.clear();
        } else {
            self.blocks.push(Block { kind, text: std::mem::take(&mut self.text) });
        }
    }

    /// Adds a finished block after the pending text, which is flushed as `pending_kind` first.
    pub(crate) fn push_block(&mut self, pending_kind: BlockKind, kind: BlockKind, text: impl Into<String>) {
        self.flush(pending_kind);
        self.blocks.push(Block { kind, text: text.into() });
    }

    pub(crate) fn into_blocks(mut self, pending_kind: BlockKind) -> Vec<Block> {
        self.flush(pending_kind);
        self.blocks
    }
}

/// Rejects text inputs above [`MAX_TEXT_BYTES`].
pub(crate) fn check_text_size(len: usize) -> Result<(), ImportError> {
    if len > MAX_TEXT_BYTES { Err(ImportError::TooLarge("text is larger than 64 MB")) } else { Ok(()) }
}

/// Collapses whitespace runs to single spaces and trims.
pub(crate) fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A cleaned, non-empty title candidate.
pub(crate) fn clean_title(s: &str) -> Option<String> {
    let t = collapse_whitespace(&s.replace(['\u{200B}', '\u{FEFF}', '\u{00AD}'], ""));
    (!t.is_empty()).then_some(t)
}

/// The text as a title if it is a single short line.
pub(crate) fn short_line_title(line: &str) -> Option<String> {
    clean_title(line).filter(|t| t.chars().count() <= SHORT_LINE_CHARS)
}

/// A title cut from the start of a longer text: whole words up to about [`DERIVED_TITLE_CHARS`] chars plus an ellipsis.
pub(crate) fn derived_title(text: &str) -> Option<String> {
    let t = clean_title(text)?;
    if t.chars().count() <= SHORT_LINE_CHARS {
        return Some(t);
    }
    let mut out = String::new();
    for word in t.split(' ') {
        if !out.is_empty() && out.chars().count() + 1 + word.chars().count() > DERIVED_TITLE_CHARS {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.chars().count() > DERIVED_TITLE_CHARS {
        out = out.chars().take(DERIVED_TITLE_CHARS).collect();
    }
    let out = out.trim_end_matches([',', ';', ':', '.', ' ']).to_owned();
    Some(format!("{out}…"))
}

/// An author name with a leading "By" removed; `None` when nothing is left.
pub(crate) fn clean_author(s: &str) -> Option<String> {
    let t = collapse_whitespace(s);
    let t = t.strip_prefix("By ").or_else(|| t.strip_prefix("by ")).unwrap_or(&t).trim();
    (!t.is_empty()).then(|| t.to_owned())
}

/// Normalises the blocks and fills in the title. When `title` is `None` the first heading is used, else the start of
/// the first block.
pub(crate) fn finish(
    title: Option<String>,
    author: Option<String>,
    blocks: Vec<Block>,
) -> Result<Imported, ImportError> {
    let blocks = normalise_blocks(blocks);
    // A document whose only blocks are our own "skipped" placeholders has nothing to read.
    if blocks.iter().all(|b| b.text == CODE_SKIPPED || b.text == TABLE_SKIPPED) {
        return Err(ImportError::Empty);
    }
    let title = title
        .and_then(|t| clean_title(&t))
        .or_else(|| blocks.iter().find(|b| b.kind == BlockKind::Heading).and_then(|b| short_line_title(&b.text)))
        .or_else(|| blocks.first().and_then(|b| derived_title(&b.text)))
        .unwrap_or_else(|| UNTITLED.to_owned());
    let author = author.and_then(|a| clean_author(&a));
    Ok(Imported { title, author, blocks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_drops_whitespace_only_blocks() {
        let mut b = BlockBuffer::default();
        b.push_str("  \n ");
        b.flush(BlockKind::Paragraph);
        b.push_str("Hello");
        b.push_block(BlockKind::Quote, BlockKind::Heading, "Next");
        let blocks = b.into_blocks(BlockKind::Paragraph);
        assert_eq!(
            blocks,
            vec![
                Block { kind: BlockKind::Quote, text: "Hello".into() },
                Block { kind: BlockKind::Heading, text: "Next".into() },
            ]
        );
    }

    #[test]
    fn short_line_title_rejects_long_lines() {
        assert_eq!(short_line_title("  A   Title \n"), Some("A Title".into()));
        assert_eq!(short_line_title(&"word ".repeat(30)), None);
        assert_eq!(short_line_title(" \n "), None);
    }

    #[test]
    fn derived_title_cuts_at_a_word() {
        let long =
            "The quick brown fox jumps over the lazy dog, and then it keeps running across the field until night.";
        let t = derived_title(long).unwrap_or_default();
        assert!(t.ends_with('…'), "{t}");
        assert!(t.chars().count() <= DERIVED_TITLE_CHARS + 1, "{t}");
        assert!(long.starts_with(t.trim_end_matches('…')), "{t}");
        assert_eq!(derived_title("Short one"), Some("Short one".into()));
    }

    #[test]
    fn derived_title_cuts_a_single_huge_word() {
        let t = derived_title(&"x".repeat(500)).unwrap_or_default();
        assert_eq!(t.chars().count(), DERIVED_TITLE_CHARS + 1);
    }

    #[test]
    fn clean_author_strips_by() {
        assert_eq!(clean_author("By  Jane Doe "), Some("Jane Doe".into()));
        assert_eq!(clean_author("by Jane"), Some("Jane".into()));
        assert_eq!(clean_author("  "), None);
        assert_eq!(clean_author("Byron"), Some("Byron".into()));
    }

    #[test]
    fn text_size_limit() {
        assert!(check_text_size(10).is_ok());
        assert_eq!(check_text_size(MAX_TEXT_BYTES + 1), Err(ImportError::TooLarge("text is larger than 64 MB")));
    }
}
