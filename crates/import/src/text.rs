//! Plain text and pasted text. Paragraphs are separated by blank lines; single line breaks inside a paragraph are kept
//! for the normaliser, which joins hyphenated line ends and collapses the rest to spaces.
//!
//! Two heuristics recover structure that plain text cannot express:
//! - text pasted from web pages often separates paragraphs with a single newline and no blank line; a chunk whose
//!   lines (other than the last) run past [`HARD_WRAP_MAX_CHARS`] is not hard-wrapped, so each line is a paragraph;
//! - a lone short line without closing punctuation ("Chapter 3", "The Storm") followed by more text is a heading.

use nolabs_core::proto::api::{Block, BlockKind};

use crate::common::{check_text_size, clean_title, finish, short_line_title};
use crate::{ImportError, Imported};

/// Hard-wrapped text (email, Project Gutenberg) wraps at 60–80 chars; longer lines are whole paragraphs.
const HARD_WRAP_MAX_CHARS: usize = 100;
/// Longest lone line treated as a heading.
const HEADING_MAX_CHARS: usize = 60;

#[derive(Debug, Default)]
pub(crate) struct Extracted {
    pub title: Option<String>,
    pub blocks: Vec<Block>,
}

pub(crate) fn import(text: &str, title_hint: Option<&str>) -> Result<Imported, ImportError> {
    let ex = extract(text, title_hint)?;
    finish(ex.title, None, ex.blocks)
}

pub(crate) fn extract(text: &str, title_hint: Option<&str>) -> Result<Extracted, ImportError> {
    check_text_size(text.len())?;
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let text = text.replace("\r\n", "\n").replace(['\r', '\u{2028}'], "\n").replace('\u{2029}', "\n\n");

    let paragraphs = paragraphs(&text);
    let count = paragraphs.len();
    let blocks: Vec<Block> = paragraphs
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let kind = if i + 1 < count && looks_like_heading(&p) { BlockKind::Heading } else { BlockKind::Paragraph };
            Block { kind, text: p }
        })
        .collect();

    let title = title_hint
        .and_then(clean_title)
        .or_else(|| text.lines().find(|l| !l.trim().is_empty()).and_then(short_line_title));
    Ok(Extracted { title, blocks })
}

/// Paragraph texts in order: chunks between blank lines, with unwrapped chunks split per line.
fn paragraphs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chunk: Vec<&str> = Vec::new();
    for line in text.split('\n') {
        if line.trim().is_empty() {
            push_chunk(&mut chunk, &mut out);
        } else {
            chunk.push(line);
        }
    }
    push_chunk(&mut chunk, &mut out);
    out
}

fn push_chunk(chunk: &mut Vec<&str>, out: &mut Vec<String>) {
    if chunk.is_empty() {
        return;
    }
    let init = &chunk[..chunk.len() - 1];
    let unwrapped = init.iter().any(|l| l.trim().chars().count() > HARD_WRAP_MAX_CHARS);
    if unwrapped {
        out.extend(chunk.iter().map(|l| l.trim().to_owned()));
    } else {
        out.push(chunk.iter().map(|l| l.trim()).collect::<Vec<_>>().join("\n"));
    }
    chunk.clear();
}

/// One short line that starts like a title and ends without sentence punctuation.
fn looks_like_heading(p: &str) -> bool {
    if p.contains('\n') || p.chars().count() > HEADING_MAX_CHARS {
        return false;
    }
    let starts_ok = p.chars().next().is_some_and(|c| c.is_uppercase() || c.is_ascii_digit());
    let ends_ok = p.chars().last().is_some_and(|c| c.is_alphanumeric() || c == ')');
    starts_ok && ends_ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use BlockKind::{Heading, Paragraph};

    fn run(text: &str, hint: Option<&str>) -> (Option<String>, Vec<(BlockKind, String)>) {
        let ex = extract(text, hint).unwrap_or_default();
        (ex.title, ex.blocks.into_iter().map(|b| (b.kind, b.text)).collect())
    }

    #[test]
    fn splits_on_blank_lines_and_keeps_wraps() {
        let (title, blocks) = run("First para line one\nline two.\n\n  \t\nSecond para.\r\n\r\nThird.", None);
        assert_eq!(title.as_deref(), Some("First para line one"));
        assert_eq!(
            blocks,
            vec![
                (Paragraph, "First para line one\nline two.".into()),
                (Paragraph, "Second para.".into()),
                (Paragraph, "Third.".into()),
            ]
        );
    }

    #[test]
    fn title_hint_wins() {
        let (title, _) = run("Some text.", Some("  My   Notes "));
        assert_eq!(title.as_deref(), Some("My Notes"));
        let (title, _) = run("Some text.", Some("   "));
        assert_eq!(title.as_deref(), Some("Some text."));
    }

    #[test]
    fn long_first_line_is_not_a_title() {
        let long = "word ".repeat(40);
        let (title, blocks) = run(&long, None);
        assert_eq!(title, None);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn single_newline_paragraphs_from_the_web_are_split() {
        let a = "a".repeat(150);
        let b = "b".repeat(120);
        let (_, blocks) = run(&format!("{a}\n{b}\nshort tail"), None);
        assert_eq!(blocks, vec![(Paragraph, a), (Paragraph, b), (Paragraph, "short tail".into())]);
    }

    #[test]
    fn lone_short_lines_become_headings() {
        let (title, blocks) = run("The Storm\n\nIt was a dark night.\n\nChapter 2\n\nMorning came.\n\nThe End", None);
        assert_eq!(title.as_deref(), Some("The Storm"));
        assert_eq!(
            blocks,
            vec![
                (Heading, "The Storm".into()),
                (Paragraph, "It was a dark night.".into()),
                (Heading, "Chapter 2".into()),
                (Paragraph, "Morning came.".into()),
                // The last block has nothing after it to head.
                (Paragraph, "The End".into()),
            ]
        );
    }

    #[test]
    fn dialogue_and_sentences_are_not_headings() {
        assert!(!looks_like_heading("“Yes,” she said."));
        assert!(!looks_like_heading("lowercase start"));
        assert!(!looks_like_heading("Two\nlines"));
        assert!(!looks_like_heading("Ends with colon:"));
        assert!(looks_like_heading("Part One (1923)"));
    }

    #[test]
    fn bom_and_unicode_separators() {
        let (_, blocks) = run("\u{FEFF}One.\u{2029}Two.", None);
        assert_eq!(blocks, vec![(Paragraph, "One.".into()), (Paragraph, "Two.".into())]);
    }

    #[test]
    fn whitespace_only_is_empty() {
        let (title, blocks) = run(" \n\n\t ", None);
        assert_eq!(title, None);
        assert!(blocks.is_empty());
    }
}
