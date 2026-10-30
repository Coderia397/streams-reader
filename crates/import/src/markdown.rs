//! Markdown (CommonMark plus GFM tables, footnotes, strikethrough and task lists) to blocks.
//!
//! Code blocks and tables are not readable aloud; each becomes one short paragraph saying it was skipped. Images,
//! footnote definitions and references are dropped. Raw HTML blocks go through the HTML block walker, so `<p>` or
//! `<blockquote>` written as HTML still read as text and `<script>` never does. YAML (`---`) or TOML (`+++`) front
//! matter provides `title` and `author` when present.

use dom_query::Document;
use nolabs_core::proto::api::{Block, BlockKind};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::common::{BlockBuffer, CODE_SKIPPED, TABLE_SKIPPED, check_text_size, clean_title, finish, short_line_title};
use crate::html::tree_of;
use crate::tree_blocks::blocks_from_tree;
use crate::{ImportError, Imported};

#[derive(Debug, Default)]
pub(crate) struct Extracted {
    pub title: Option<String>,
    pub author: Option<String>,
    pub blocks: Vec<Block>,
}

pub(crate) fn import(md: &str, title_hint: Option<&str>) -> Result<Imported, ImportError> {
    let ex = extract(md, title_hint)?;
    finish(ex.title, ex.author, ex.blocks)
}

/// Where the parser is, as far as reading goes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Reading,
    /// Inside an element whose text is not read; the count tracks nested starts so the matching end is found.
    Skipping(usize),
    /// Inside front matter; its raw text is collected.
    Metadata,
    /// Inside a raw HTML block; its source is collected.
    Html,
}

/// Event state machine. Kinds of loose text are a stack: Paragraph at top level, Quote inside `>`, ListItem in items.
struct Walker {
    buf: BlockBuffer,
    kinds: Vec<BlockKind>,
    mode: Mode,
    in_heading: bool,
    heading_text: String,
    first_h1: Option<String>,
    metadata: String,
    html: String,
}

impl Walker {
    fn new() -> Self {
        Self {
            buf: BlockBuffer::default(),
            kinds: vec![BlockKind::Paragraph],
            mode: Mode::Reading,
            in_heading: false,
            heading_text: String::new(),
            first_h1: None,
            metadata: String::new(),
            html: String::new(),
        }
    }

    fn current(&self) -> BlockKind {
        if self.in_heading { BlockKind::Heading } else { self.kinds.last().copied().unwrap_or(BlockKind::Paragraph) }
    }

    fn event(&mut self, event: Event<'_>) {
        match &self.mode {
            Mode::Skipping(depth) => {
                self.mode = match event {
                    Event::Start(_) => Mode::Skipping(depth + 1),
                    Event::End(_) if *depth <= 1 => Mode::Reading,
                    Event::End(_) => Mode::Skipping(depth - 1),
                    _ => return,
                };
                return;
            }
            Mode::Metadata => {
                match event {
                    Event::Text(t) => self.metadata.push_str(&t),
                    Event::End(TagEnd::MetadataBlock(_)) => self.mode = Mode::Reading,
                    _ => {}
                }
                return;
            }
            Mode::Html => {
                match event {
                    Event::Html(h) | Event::Text(h) => self.html.push_str(&h),
                    Event::End(TagEnd::HtmlBlock) => self.end_html_block(),
                    _ => {}
                }
                return;
            }
            Mode::Reading => {}
        }
        let current = self.current();
        match event {
            Event::Start(tag) => self.start(tag, current),
            Event::End(tag) => self.end(tag, current),
            Event::Text(text) | Event::Code(text) => {
                if self.in_heading {
                    self.heading_text.push_str(&text);
                }
                self.buf.push_str(&text);
            }
            Event::SoftBreak | Event::HardBreak => self.buf.push_char('\n'),
            Event::Rule => self.buf.flush(current),
            // Inline tags such as `<sup>` are dropped; the text between them is still read.
            Event::Html(_)
            | Event::InlineHtml(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::FootnoteReference(_)
            | Event::TaskListMarker(_) => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>, current: BlockKind) {
        match tag {
            Tag::Paragraph => self.buf.flush(current),
            Tag::Heading { .. } => {
                self.buf.flush(current);
                self.in_heading = true;
                self.heading_text.clear();
            }
            Tag::BlockQuote(_) => {
                self.buf.flush(current);
                self.kinds.push(BlockKind::Quote);
            }
            Tag::Item => {
                self.buf.flush(current);
                self.kinds.push(BlockKind::ListItem);
            }
            Tag::CodeBlock(_) => {
                self.buf.push_block(current, BlockKind::Paragraph, CODE_SKIPPED);
                self.mode = Mode::Skipping(1);
            }
            Tag::Table(_) => {
                self.buf.push_block(current, BlockKind::Paragraph, TABLE_SKIPPED);
                self.mode = Mode::Skipping(1);
            }
            Tag::Image { .. } | Tag::FootnoteDefinition(_) => self.mode = Mode::Skipping(1),
            Tag::MetadataBlock(_) => self.mode = Mode::Metadata,
            Tag::HtmlBlock => {
                self.buf.flush(current);
                self.html.clear();
                self.mode = Mode::Html;
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd, current: BlockKind) {
        match tag {
            TagEnd::Paragraph => self.buf.flush(current),
            TagEnd::Heading(level) => {
                if level == HeadingLevel::H1 && self.first_h1.is_none() {
                    self.first_h1 = short_line_title(&self.heading_text);
                }
                self.buf.flush(BlockKind::Heading);
                self.in_heading = false;
            }
            TagEnd::BlockQuote(_) | TagEnd::Item => {
                self.buf.flush(current);
                if self.kinds.len() > 1 {
                    self.kinds.pop();
                }
            }
            _ => {}
        }
    }

    fn end_html_block(&mut self) {
        self.mode = Mode::Reading;
        let current = self.current();
        let doc = Document::from(self.html.as_str());
        for b in blocks_from_tree(&tree_of(&doc.root())) {
            // Loose HTML text takes the kind of the Markdown context (a list item, a quote); real blocks keep theirs.
            let kind = if b.kind == BlockKind::Paragraph { current } else { b.kind };
            self.buf.push_block(current, kind, b.text);
        }
        self.html.clear();
    }
}

pub(crate) fn extract(md: &str, title_hint: Option<&str>) -> Result<Extracted, ImportError> {
    check_text_size(md.len())?;
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS
        | Options::ENABLE_GFM;

    let mut w = Walker::new();
    for event in Parser::new_ext(md, options) {
        w.event(event);
    }
    let (meta_title, author) = front_matter(&w.metadata);
    let title = meta_title.or(w.first_h1).or_else(|| title_hint.and_then(clean_title));
    Ok(Extracted { title, author, blocks: w.buf.into_blocks(BlockKind::Paragraph) })
}

/// `title` and `author` from YAML (`title: X`) or TOML (`title = "X"`) front matter. Only scalar values are read.
fn front_matter(meta: &str) -> (Option<String>, Option<String>) {
    let mut title = None;
    let mut author = None;
    for line in meta.lines() {
        let Some((key, value)) = line.split_once(':').or_else(|| line.split_once('=')) else { continue };
        // `["A"]` (TOML/YAML flow list) reads as its first entry's text.
        let raw = value.trim();
        let value = match raw.strip_prefix('[') {
            Some(list) => list.trim_end_matches(']').split(',').next().unwrap_or_default(),
            None => raw,
        };
        let value = value.trim().trim_matches(|c| c == '"' || c == '\'').trim();
        if value.is_empty() {
            continue;
        }
        match key.trim().to_ascii_lowercase().as_str() {
            "title" if title.is_none() => title = clean_title(value),
            "author" | "authors" | "creator" if author.is_none() => author = clean_title(value),
            _ => {}
        }
    }
    (title, author)
}

#[cfg(test)]
mod tests {
    use super::*;
    use BlockKind::{Heading, ListItem, Paragraph, Quote};

    fn run(md: &str) -> Vec<(BlockKind, String)> {
        let ex = extract(md, None).unwrap_or_default();
        ex.blocks.into_iter().map(|b| (b.kind, b.text.split_whitespace().collect::<Vec<_>>().join(" "))).collect()
    }

    #[test]
    fn headings_paragraphs_quotes_lists() {
        let md = "# Title\n\nSome *emphasis* and `code` here.\nWrapped line.\n\n> Quoted\n> text.\n\n- one\n- two\n\n1. Loose\n\n2. List\n";
        assert_eq!(
            run(md),
            vec![
                (Heading, "Title".into()),
                (Paragraph, "Some emphasis and code here. Wrapped line.".into()),
                (Quote, "Quoted text.".into()),
                (ListItem, "one".into()),
                (ListItem, "two".into()),
                (ListItem, "Loose".into()),
                (ListItem, "List".into()),
            ]
        );
    }

    #[test]
    fn code_blocks_and_tables_are_replaced() {
        let md =
            "Intro.\n\n```rust\nfn main() {}\n```\n\n    indented code\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nOutro.\n";
        assert_eq!(
            run(md),
            vec![
                (Paragraph, "Intro.".into()),
                (Paragraph, CODE_SKIPPED.into()),
                (Paragraph, CODE_SKIPPED.into()),
                (Paragraph, TABLE_SKIPPED.into()),
                (Paragraph, "Outro.".into()),
            ]
        );
    }

    #[test]
    fn images_and_footnotes_are_dropped() {
        let md = "Text with a note[^1] and ![a cat](cat.png) image.\n\n[^1]: The footnote body.\n";
        assert_eq!(run(md), vec![(Paragraph, "Text with a note and image.".into())]);
    }

    #[test]
    fn links_keep_their_text() {
        assert_eq!(run("See [the docs](https://x.y) now."), vec![(Paragraph, "See the docs now.".into())]);
    }

    #[test]
    fn nested_quote_in_list() {
        let md = "- item\n\n  > quoted in item\n- next\n";
        assert_eq!(
            run(md),
            vec![(ListItem, "item".into()), (Quote, "quoted in item".into()), (ListItem, "next".into())]
        );
    }

    #[test]
    fn html_blocks_are_read_without_markup() {
        let md = "<div>\n<p>Raw <b>html</b> para.</p>\n<script>alert(1)</script>\n</div>\n\nAfter.\n";
        assert_eq!(run(md), vec![(Paragraph, "Raw html para.".into()), (Paragraph, "After.".into())]);
    }

    #[test]
    fn smart_punctuation() {
        assert_eq!(run("\"Don't\" -- wait..."), vec![(Paragraph, "“Don’t” – wait…".into())]);
    }

    #[test]
    fn title_priority_front_matter_then_h1_then_hint() {
        let md = "---\ntitle: \"Front Title\"\nauthor: Ann Writer\n---\n\n# Heading Title\n\nBody.\n";
        let ex = extract(md, Some("file.md")).unwrap_or_default();
        assert_eq!(ex.title.as_deref(), Some("Front Title"));
        assert_eq!(ex.author.as_deref(), Some("Ann Writer"));
        assert_eq!(ex.blocks.len(), 2, "front matter must not be read: {:?}", ex.blocks);

        let ex = extract("## Sub\n\n# Main\n\nBody.", Some("hint")).unwrap_or_default();
        assert_eq!(ex.title.as_deref(), Some("Main"));

        let ex = extract("Body only.", Some(" hint ")).unwrap_or_default();
        assert_eq!(ex.title.as_deref(), Some("hint"));
    }

    #[test]
    fn toml_front_matter() {
        assert_eq!(
            front_matter("title = \"T, too\"\nauthors = [\"A\", \"B\"]\nother = 1"),
            (Some("T, too".into()), Some("A".into()))
        );
    }

    #[test]
    fn unbalanced_input_does_not_panic() {
        let _ = run("> > > - ```\n| |\n");
        let _ = run("[^x]:\n\n![](");
    }
}
