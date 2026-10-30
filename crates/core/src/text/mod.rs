//! Text normalisation and segmentation. The app runs these at import and before rendering; the output text is exactly
//! what is shown, spoken and hashed, so highlighting offsets line up without a mapping table.
//!
//! Offsets are char (Unicode scalar) indices. The document's reading text is its block texts joined by `"\n\n"`.

mod normaliser;
mod segmenter;
mod sentence;

use crate::proto::api::Block;

pub use normaliser::normalise;
pub use segmenter::{Segment, SegmentProfile, segment};
pub use sentence::sentences;

/// Bump when [`normalise`] output changes; part of every cache key.
pub const NORMALISER_VERSION: u32 = 1;
/// Bump when [`segment`] output changes; part of every cache key.
pub const SEGMENTER_VERSION: u32 = 1;
const BLOCK_SEPARATOR: &str = "\n\n";

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

/// Char count as `u32`. Offsets are `u32` on the wire; no block or document comes near 4 G chars, so saturating is only
/// a guard against wrapping, never a real case.
fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::api::BlockKind;

    fn block(kind: BlockKind, text: &str) -> Block {
        Block { kind, text: text.to_owned() }
    }

    #[test]
    fn normalise_blocks_drops_empty_blocks() {
        let blocks = vec![
            block(BlockKind::Heading, "  Title\n"),
            block(BlockKind::Paragraph, " [12] \u{200b} "),
            block(BlockKind::Paragraph, "Body  text."),
        ];
        assert_eq!(
            normalise_blocks(blocks),
            vec![block(BlockKind::Heading, "Title"), block(BlockKind::Paragraph, "Body text.")]
        );
    }

    #[test]
    fn reading_text_and_offsets_agree() {
        let blocks =
            vec![block(BlockKind::Heading, "Één"), block(BlockKind::Paragraph, "two"), block(BlockKind::Quote, "x")];
        let text = reading_text(&blocks);
        assert_eq!(text, "Één\n\ntwo\n\nx");
        let chars: Vec<char> = text.chars().collect();
        for (b, off) in blocks.iter().zip(block_offsets(&blocks)) {
            let start = off as usize;
            let got: String = chars[start..start + b.text.chars().count()].iter().collect();
            assert_eq!(got, b.text);
        }
    }

    #[test]
    fn segment_at_finds_containing_or_previous_segment() {
        let blocks = vec![block(BlockKind::Paragraph, "One. Two."), block(BlockKind::Paragraph, "Three.")];
        let segs = segment(&blocks, SegmentProfile { min_chars: 0, max_chars: 6, paragraph_mode: false });
        assert_eq!(segs.len(), 3);
        assert_eq!(segment_at(&segs, 0), Some(0));
        assert_eq!(segment_at(&segs, 4), Some(0));
        assert_eq!(segment_at(&segs, 5), Some(1));
        assert_eq!(segment_at(&segs, 10), Some(1));
        assert_eq!(segment_at(&segs, 11), Some(2));
        assert_eq!(segment_at(&segs, 999), Some(2));
        assert_eq!(segment_at(&[], 0), None);
    }

    #[test]
    fn word_count_ignores_punctuation() {
        assert_eq!(word_count("Hello, world — it's 3 o'clock…"), 5);
        assert_eq!(word_count(""), 0);
    }
}
