//! Rebuilds headings and paragraphs from positioned lines.
//!
//! PDFs have no paragraph markup, only lines. A new paragraph starts after a vertical gap clearly larger than the
//! usual line spacing, or after a short line that ends a sentence (books mark paragraphs by indent, not spacing).
//! Lines noticeably taller than body text are headings. A paragraph continues across a page or column break unless
//! the line before the break ends it.

use nolabs_core::proto::api::{Block, BlockKind};

use super::Line;

/// A gap this many times the usual line spacing separates paragraphs.
const PARAGRAPH_GAP_FACTOR: f32 = 1.5;
/// Lines at least this many times the body height are headings…
const HEADING_HEIGHT_FACTOR: f32 = 1.25;
/// …when no longer than this.
const HEADING_MAX_CHARS: usize = 120;
/// A line ending a sentence and shorter than this share of a typical line ends its paragraph.
const SHORT_LINE_SHARE: f32 = 0.75;
/// Consecutive heading lines further apart than this many times their height are separate headings.
const HEADING_JOIN_FACTOR: f32 = 1.8;

/// Document-wide spacing statistics.
struct Metrics {
    body_height: f32,
    /// Usual distance between consecutive body lines.
    line_gap: f32,
    /// Sign of `y` change from one line to the next in reading order (pdf.js user space: -1, top-down: +1).
    direction: f32,
    /// Median length of a body line, in chars.
    typical_chars: f32,
}

pub(super) fn blocks(pages: &[Vec<Line>], body_height: f32) -> Vec<Block> {
    let m = metrics(pages, body_height);
    let mut out: Vec<Block> = Vec::new();
    let mut text = String::new();
    let mut kind = BlockKind::Paragraph;
    let mut prev: Option<&Line> = None;
    let mut prev_page = 0usize;

    for (p, lines) in pages.iter().enumerate() {
        for line in lines {
            let line_kind = if is_heading(line, &m) { BlockKind::Heading } else { BlockKind::Paragraph };
            if let Some(before) = prev
                && breaks(before, line, p == prev_page, &m)
            {
                push(&mut out, kind, std::mem::take(&mut text));
            }
            if text.is_empty() {
                kind = line_kind;
                text.push_str(&line.text);
            } else {
                join_line(&mut text, &line.text);
            }
            prev = Some(line);
            prev_page = p;
        }
    }
    push(&mut out, kind, text);
    out
}

fn push(out: &mut Vec<Block>, kind: BlockKind, text: String) {
    if !text.trim().is_empty() {
        out.push(Block { kind, text });
    }
}

fn metrics(pages: &[Vec<Line>], body_height: f32) -> Metrics {
    let is_body = |l: &Line| body_height <= 0.0 || (l.height - body_height).abs() <= body_height * 0.2;
    let mut gaps = Vec::new();
    let mut signed = Vec::new();
    for lines in pages {
        for pair in lines.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let dy = b.y - a.y;
            let limit = if body_height > 0.0 { body_height * 3.0 } else { f32::INFINITY };
            if is_body(a) && is_body(b) && dy != 0.0 && dy.abs() < limit {
                gaps.push(dy.abs());
                signed.push(dy);
            }
        }
    }
    let direction = median(&mut signed).map_or(-1.0, f32::signum);
    let line_gap = median(&mut gaps).unwrap_or(body_height * 1.2);
    let mut lengths: Vec<f32> =
        pages.iter().flatten().filter(|l| is_body(l)).map(|l| l.text.chars().count() as f32).collect();
    let typical_chars = median(&mut lengths).unwrap_or(0.0);
    Metrics { body_height, line_gap, direction, typical_chars }
}

pub(super) fn median(values: &mut [f32]) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f32::total_cmp);
    values.get(values.len() / 2).copied()
}

fn is_heading(line: &Line, m: &Metrics) -> bool {
    m.body_height > 0.0
        && line.height >= m.body_height * HEADING_HEIGHT_FACTOR
        && line.text.chars().count() <= HEADING_MAX_CHARS
}

/// Whether `line` starts a new block after `prev`.
fn breaks(prev: &Line, line: &Line, same_page: bool, m: &Metrics) -> bool {
    let (prev_heading, heading) = (is_heading(prev, m), is_heading(line, m));
    if prev_heading != heading {
        return true;
    }
    let dy = line.y - prev.y;
    if heading {
        return !same_page || dy.abs() > prev.height.max(line.height) * HEADING_JOIN_FACTOR;
    }
    if ends_paragraph(prev, m) {
        return true;
    }
    // Next page, or back up to the top of the next column: the paragraph carries on.
    if !same_page || dy * m.direction <= 0.0 {
        return false;
    }
    m.line_gap > 0.0 && dy.abs() > m.line_gap * PARAGRAPH_GAP_FACTOR
}

/// A short line that finishes a sentence is the last line of its paragraph.
fn ends_paragraph(line: &Line, m: &Metrics) -> bool {
    let t = line.text.trim_end_matches(['"', '\'', '”', '’', ')', ']', '»', ' ']);
    let terminal = t.ends_with(['.', '!', '?', ':', '…']);
    terminal && m.typical_chars > 0.0 && (line.text.chars().count() as f32) < m.typical_chars * SHORT_LINE_SHARE
}

/// Appends a line to paragraph text, joining a word hyphenated across the line end (`exam-` + `ple` → `example`) and
/// otherwise separating with a space. Before an upper-case letter or digit the hyphen is part of the word and stays
/// (`Jean-` + `Paul` → `Jean-Paul`, `COVID-` + `19` → `COVID-19`).
pub(super) fn join_line(text: &mut String, next: &str) {
    if text.ends_with('\u{AD}') {
        text.pop();
        text.push_str(next);
        return;
    }
    let hyphenated = text.strip_suffix('-').is_some_and(|t| t.chars().last().is_some_and(char::is_alphabetic));
    match next.chars().next() {
        Some(c) if hyphenated && c.is_lowercase() => {
            text.pop();
        }
        Some(c) if hyphenated && (c.is_uppercase() || c.is_ascii_digit()) => {}
        _ => text.push(' '),
    }
    text.push_str(next);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, y: f32) -> Line {
        Line { text: text.into(), y, height: 10.0 }
    }

    fn tall(text: &str, y: f32) -> Line {
        Line { text: text.into(), y, height: 18.0 }
    }

    fn run(pages: &[Vec<Line>]) -> Vec<(BlockKind, String)> {
        blocks(pages, 10.0).into_iter().map(|b| (b.kind, b.text)).collect()
    }

    const FULL: &str = "a full line of body text that runs from margin to margin";

    #[test]
    fn gaps_split_paragraphs_and_hyphens_join() {
        let page = vec![
            tall("A Heading", 760.0),
            line("The first para-", 720.0),
            line("graph goes on and", 708.0),
            line("on.", 696.0),
            // Wider gap: a new paragraph even without a short line.
            line("Second paragraph", 670.0),
            line("ends here", 658.0),
        ];
        assert_eq!(
            run(&[page]),
            vec![
                (BlockKind::Heading, "A Heading".into()),
                (BlockKind::Paragraph, "The first paragraph goes on and on.".into()),
                (BlockKind::Paragraph, "Second paragraph ends here".into()),
            ]
        );
    }

    #[test]
    fn short_sentence_end_lines_split_without_gaps() {
        let page = vec![line(FULL, 700.0), line("short end.", 688.0), line(FULL, 676.0), line(FULL, 664.0)];
        let out = run(&[page]);
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out[0].1.ends_with("short end."));
    }

    #[test]
    fn paragraphs_continue_across_pages_and_columns() {
        let p1 = vec![line(FULL, 700.0), line("this sentence continues", 688.0)];
        let p2 = vec![line("on the next page.", 760.0)];
        let out = run(&[p1, p2]);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(out[0].1.ends_with("continues on the next page."));
    }

    #[test]
    fn works_with_top_down_coordinates() {
        let page = vec![line("One", 100.0), line("two.", 112.0), line("Three", 140.0), line("four", 152.0)];
        let out = run(&[page]);
        assert_eq!(out, vec![(BlockKind::Paragraph, "One two.".into()), (BlockKind::Paragraph, "Three four".into())]);
    }

    #[test]
    fn multi_line_headings_merge() {
        let page = vec![tall("A Long", 760.0), tall("Title", 740.0), line(FULL, 700.0)];
        let out = run(&[page]);
        assert_eq!(out[0], (BlockKind::Heading, "A Long Title".into()));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn join_rules() {
        let cases = [
            ("exam-", "ple", "example"),
            ("Jean-", "Paul", "Jean-Paul"),
            ("COVID-", "19", "COVID-19"),
            ("soft\u{AD}", "ware", "software"),
            ("one", "two", "one two"),
            ("-", "x", "- x"),
        ];
        for (a, b, want) in cases {
            let mut t = a.to_owned();
            join_line(&mut t, b);
            assert_eq!(t, want, "{a} + {b}");
        }
    }

    #[test]
    fn median_ignores_order() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&mut []), None);
    }
}
