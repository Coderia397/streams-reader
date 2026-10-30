//! Splitting blocks into the units the engines render. Works on char ranges inside each block, then shifts them by the
//! block's offset in the reading text, so segment text is always an exact slice of what the reader shows.

use crate::proto::api::{Block, BlockKind, Boundary, Engine};

use super::{block_offsets, sentences};

/// Segment size limits per engine. Kokoro rushes above ~400 tokens, so its segments stay short; Ming keeps prosody
/// across a paragraph, so its segments are whole paragraphs where possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentProfile {
    pub min_chars: u32,
    pub max_chars: u32,
    /// Prefer whole paragraphs (split only when longer than `max_chars`, at the sentence boundary nearest the middle).
    pub paragraph_mode: bool,
}

impl SegmentProfile {
    pub const KOKORO: SegmentProfile = SegmentProfile { min_chars: 120, max_chars: 320, paragraph_mode: false };
    pub const MING: SegmentProfile = SegmentProfile { min_chars: 80, max_chars: 500, paragraph_mode: true };

    pub fn for_engine(engine: Engine) -> SegmentProfile {
        match engine {
            Engine::Kokoro => Self::KOKORO,
            Engine::Ming => Self::MING,
        }
    }

    /// The most a short trailing remainder may grow its predecessor to.
    fn merge_limit(self) -> u32 {
        self.max_chars.saturating_mul(5) / 4
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Position in the document's segment list for this profile.
    pub idx: u32,
    pub block: u32,
    pub text: String,
    /// What follows this segment: `Section` after a heading block or before one, `Paragraph` at the end of any other
    /// block, otherwise `Sentence`.
    pub boundary: Boundary,
    /// Char range inside [`super::reading_text`].
    pub char_start: u32,
    pub char_end: u32,
    pub heading: bool,
}

/// Char range `(start, end)` inside one block, end exclusive.
type Span = (u32, u32);

fn len(span: Span) -> u32 {
    span.1 - span.0
}

/// Split blocks into speakable segments. Rules:
/// - never split inside a sentence unless one sentence alone exceeds `max_chars`; then split at the last `;`, `:`, `,`
///   or space before the limit;
/// - never cross a block boundary; headings are always their own segment;
/// - greedily pack whole sentences up to `max_chars`; a trailing remainder shorter than `min_chars` is merged into the
///   previous segment of the same block when the merge stays within `max_chars * 5 / 4`;
/// - in `paragraph_mode`, a block that fits in `max_chars` is one segment; a longer one splits at the sentence boundary
///   nearest its middle, recursively.
///
/// Segment text is the exact slice of the block text (trimmed), so `reading_text[char_start..char_end] == text`.
///
/// Details the rules leave open: a heading is never packed with other text but is split like a long sentence if it
/// alone exceeds `max_chars`; when splitting a long sentence, a `;`, `:` or `,` in the second half of the window wins
/// over a later space, and a run with no space at all is cut hard at the limit; blocks that are empty after trimming
/// produce no segments and are skipped when deciding whether a heading follows.
pub fn segment(blocks: &[Block], profile: SegmentProfile) -> Vec<Segment> {
    let spans: Vec<Vec<Span>> = blocks.iter().map(|b| block_spans(b, profile)).collect();

    // Kind of the next block that produced segments, for each block.
    let mut next_kind = vec![None; blocks.len()];
    let mut following = None;
    for i in (0..blocks.len()).rev() {
        next_kind[i] = following;
        if !spans[i].is_empty() {
            following = Some(blocks[i].kind);
        }
    }

    let mut out = Vec::new();
    for (bi, ((block, block_spans), offset)) in blocks.iter().zip(&spans).zip(block_offsets(blocks)).enumerate() {
        let heading = block.kind == BlockKind::Heading;
        let ends_section = heading || next_kind[bi] == Some(BlockKind::Heading);
        let chars: Vec<char> = block.text.chars().collect();
        for (si, &(start, end)) in block_spans.iter().enumerate() {
            let boundary = if si + 1 < block_spans.len() {
                Boundary::Sentence
            } else if ends_section {
                Boundary::Section
            } else {
                Boundary::Paragraph
            };
            out.push(Segment {
                idx: super::to_u32(out.len()),
                block: super::to_u32(bi),
                text: chars[start as usize..end as usize].iter().collect(),
                boundary,
                char_start: offset + start,
                char_end: offset + end,
                heading,
            });
        }
    }
    out
}

/// Segment spans of one block, relative to the block.
fn block_spans(block: &Block, profile: SegmentProfile) -> Vec<Span> {
    let chars: Vec<char> = block.text.chars().collect();
    let max = profile.max_chars.max(1);
    let spans = if block.kind == BlockKind::Heading {
        trimmed(&chars, (0, super::to_u32(chars.len()))).map(|s| split_long(&chars, s, max)).unwrap_or_default()
    } else {
        let sentences = sentences(&block.text);
        if profile.paragraph_mode {
            let mut spans = Vec::new();
            split_paragraph(&chars, &sentences, max, &mut spans);
            spans
        } else {
            let units: Vec<Span> = sentences.iter().flat_map(|&s| split_long(&chars, s, max)).collect();
            pack(&units, max)
        }
    };
    merge_short_tail(spans, profile.min_chars, profile.merge_limit())
}

/// Greedy packing: extend the current segment by whole units while it stays within `max`.
fn pack(units: &[Span], max: u32) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut current: Option<Span> = None;
    for &unit in units {
        current = match current {
            Some((start, _)) if unit.1 - start <= max => Some((start, unit.1)),
            Some(done) => {
                spans.push(done);
                Some(unit)
            }
            None => Some(unit),
        };
    }
    spans.extend(current);
    spans
}

/// Paragraph mode: the whole range of sentences if it fits, else split at the sentence boundary nearest the middle.
fn split_paragraph(chars: &[char], sentences: &[Span], max: u32, out: &mut Vec<Span>) {
    let (Some(&first), Some(&last)) = (sentences.first(), sentences.last()) else { return };
    if last.1 - first.0 <= max {
        out.push((first.0, last.1));
        return;
    }
    if sentences.len() == 1 {
        out.extend(split_long(chars, first, max));
        return;
    }
    let middle = first.0 + (last.1 - first.0) / 2;
    // Boundary k sits between sentence k-1 and sentence k; ties go to the earlier boundary.
    let k = (1..sentences.len()).min_by_key(|&k| sentences[k].0.abs_diff(middle)).unwrap_or(1);
    split_paragraph(chars, &sentences[..k], max, out);
    split_paragraph(chars, &sentences[k..], max, out);
}

/// Split one trimmed span longer than `max` into trimmed pieces of at most `max` chars.
fn split_long(chars: &[char], span: Span, max: u32) -> Vec<Span> {
    let mut pieces = Vec::new();
    let (mut start, end) = span;
    while end - start > max {
        let cut = cut_point(chars, start, start + max);
        // Both ends land next to non-whitespace: `start` is non-whitespace and the span's end is trimmed.
        let Some(piece) = trimmed(chars, (start, cut)) else { break };
        pieces.push(piece);
        start = skip_whitespace(chars, cut, end);
    }
    if start < end {
        pieces.push((start, end));
    }
    pieces
}

/// Where to end a piece that starts at `start` and may not reach past `limit` (exclusive).
fn cut_point(chars: &[char], start: u32, limit: u32) -> u32 {
    let at = |i: u32| chars[i as usize];
    let half = start + (limit - start) / 2;
    // After a clause mark in the second half of the window: `…, ` → cut after the comma.
    if let Some(p) = (half..limit).rev().find(|&i| matches!(at(i), ';' | ':' | ',')) {
        return p + 1;
    }
    // Before the last space, including a space right at the limit (callers guarantee `limit` is inside the span).
    if let Some(p) = (start + 1..=limit).rev().find(|&i| at(i).is_whitespace()) {
        return p;
    }
    if let Some(p) = (start..limit).rev().find(|&i| matches!(at(i), ';' | ':' | ',')) {
        return p + 1;
    }
    limit
}

fn skip_whitespace(chars: &[char], mut at: u32, end: u32) -> u32 {
    while at < end && chars[at as usize].is_whitespace() {
        at += 1;
    }
    at
}

/// `span` without leading or trailing whitespace; `None` when nothing is left.
fn trimmed(chars: &[char], (start, mut end): Span) -> Option<Span> {
    let start = skip_whitespace(chars, start, end);
    while end > start && chars[end as usize - 1].is_whitespace() {
        end -= 1;
    }
    (start < end).then_some((start, end))
}

/// Merge a final span shorter than `min` into the one before it when the result stays within `limit`.
fn merge_short_tail(mut spans: Vec<Span>, min: u32, limit: u32) -> Vec<Span> {
    if let [.., prev, last] = spans[..]
        && len(last) < min
        && last.1 - prev.0 <= limit
    {
        spans.pop();
        if let Some(prev) = spans.last_mut() {
            prev.1 = last.1;
        }
    }
    spans
}

#[cfg(test)]
mod tests;
