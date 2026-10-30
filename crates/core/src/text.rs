//! Text normalisation and segmentation. The app runs these at import and before rendering; the output text is exactly
//! what is shown, spoken and hashed, so highlighting offsets line up without a mapping table.
//!
//! Offsets are char (Unicode scalar) indices. The document's reading text is its block texts joined by `"\n\n"`.

use crate::proto::api::{Block, BlockKind, Boundary, Engine};

/// Bump when [`normalise`] output changes; part of every cache key.
pub const NORMALISER_VERSION: u32 = 1;
/// Bump when [`segment`] output changes; part of every cache key.
pub const SEGMENTER_VERSION: u32 = 1;
const BLOCK_SEPARATOR: &str = "\n\n";

/// Clean one block of imported text for listening:
/// - collapse whitespace runs (including newlines) to single spaces, trim;
/// - join words hyphenated across a line end (`"exam-\nple"` → `"example"`) before collapsing;
/// - drop numeric citation markers (`[12]`, `[3, 4]`, `[5–7]`) and superscript digits used as footnote marks;
/// - turn every other `[`/`]` into `(`/`)` so stray text can never become a Ming paralinguistic tag;
/// - replace bare URLs (`http(s)://…`, `www.…`) with their host without `www.`;
/// - remove zero-width and control characters; keep curly quotes, dashes and ellipses (they shape prosody).
pub fn normalise(text: &str) -> String {
    let _ = text;
    todo!("implemented in the core crate")
}

/// Normalise every block and drop the ones left empty.
pub fn normalise_blocks(blocks: Vec<Block>) -> Vec<Block> {
    blocks
        .into_iter()
        .filter_map(|b| {
            let text = normalise(&b.text);
            (!text.is_empty()).then_some(Block { kind: b.kind, text })
        })
        .collect()
}

/// The full reading text: block texts joined by a blank line.
pub fn reading_text(blocks: &[Block]) -> String {
    blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join(BLOCK_SEPARATOR)
}

/// Char offset of each block's first char inside [`reading_text`].
pub fn block_offsets(blocks: &[Block]) -> Vec<u32> {
    let mut out = Vec::with_capacity(blocks.len());
    let mut at = 0u32;
    for b in blocks {
        out.push(at);
        at += b.text.chars().count() as u32 + BLOCK_SEPARATOR.chars().count() as u32;
    }
    out
}

/// Sentence ranges `(char_start, char_end)` of one block, end exclusive, trailing whitespace excluded. Uses Unicode
/// sentence boundaries, then re-joins splits after common abbreviations (Mr., Mrs., Ms., Dr., St., Jr., Sr., vs., etc.,
/// e.g., i.e., No., Fig., single capital initials) and after a closing quote that follows `?`/`!` mid-sentence.
pub fn sentences(text: &str) -> Vec<(u32, u32)> {
    let _ = text;
    todo!("implemented in the core crate")
}

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
    /// Char range inside [`reading_text`].
    pub char_start: u32,
    pub char_end: u32,
    pub heading: bool,
}

/// Split blocks into speakable segments. Rules:
/// - never split inside a sentence unless one sentence alone exceeds `max_chars`; then split at the last `;`, `:`, `,`
///   or space before the limit;
/// - never cross a block boundary; headings are always their own segment;
/// - greedily pack whole sentences up to `max_chars`; a trailing remainder shorter than `min_chars` is merged into the
///   previous segment of the same block when the merge stays within `max_chars * 5 / 4`;
/// - in `paragraph_mode`, a block that fits in `max_chars` is one segment; a longer one splits at the sentence boundary
///   nearest its middle, recursively.
/// Segment text is the exact slice of the block text (trimmed), so `reading_text[char_start..char_end] == text`.
pub fn segment(blocks: &[Block], profile: SegmentProfile) -> Vec<Segment> {
    let _ = (blocks, profile, BlockKind::Heading);
    todo!("implemented in the core crate")
}

/// Index of the segment containing `char_offset`, or the last segment starting before it.
pub fn segment_at(segments: &[Segment], char_offset: u32) -> Option<usize> {
    if segments.is_empty() {
        return None;
    }
    let i = segments.partition_point(|s| s.char_start <= char_offset);
    Some(i.saturating_sub(1))
}

/// Words separated by Unicode word boundaries that contain a letter or digit.
pub fn word_count(text: &str) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    text.unicode_words().count()
}
