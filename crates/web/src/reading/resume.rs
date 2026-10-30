//! Where to resume: one sentence before the saved position, so the listener hears the end of the thought they were in
//! (the saved offset is often mid-sentence, and audio picked up mid-thought is hard to follow).

use nolabs_core::proto::api::Block;

/// Char offset of the start of the sentence before the one containing `saved`. Crosses into the previous block when
/// `saved` is in a block's first sentence. `sentences` is `nolabs_core::text::sentences` (injected for tests).
pub fn resume_offset(
    blocks: &[Block],
    block_offsets: &[u32],
    saved: u32,
    sentences: impl Fn(&str) -> Vec<(u32, u32)>,
) -> u32 {
    if blocks.is_empty() || saved == 0 {
        return 0;
    }
    let b = block_offsets.partition_point(|&o| o <= saved).saturating_sub(1).min(blocks.len() - 1);
    let local = saved - block_offsets[b].min(saved);
    let sents = sentences(&blocks[b].text);
    let i = sents.partition_point(|s| s.0 <= local).saturating_sub(1);
    if i > 0 {
        return block_offsets[b] + sents[i - 1].0;
    }
    if b == 0 {
        return 0;
    }
    let prev = sentences(&blocks[b - 1].text);
    block_offsets[b - 1] + prev.last().map_or(0, |s| s.0)
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::BlockKind;
    use nolabs_core::text::block_offsets;

    use super::*;

    /// Splits after ". " — enough for tests, independent of the core segmenter.
    fn simple_sentences(text: &str) -> Vec<(u32, u32)> {
        let chars: Vec<char> = text.chars().collect();
        let mut out = Vec::new();
        let mut start = 0usize;
        for i in 0..chars.len() {
            let end_here = chars[i] == '.' && (i + 1 == chars.len() || chars[i + 1] == ' ');
            if end_here {
                out.push((start as u32, (i + 1) as u32));
                start = i + 2;
            }
        }
        if start < chars.len() {
            out.push((start as u32, chars.len() as u32));
        }
        out
    }

    fn blocks() -> Vec<Block> {
        vec![
            Block { kind: BlockKind::Paragraph, text: "One. Two. Three.".into() },
            Block { kind: BlockKind::Paragraph, text: "Four. Five.".into() },
        ]
    }

    #[test]
    fn steps_back_one_sentence_in_the_same_block() {
        let b = blocks();
        let offs = block_offsets(&b);
        // Inside "Three." (starts at 10) → start of "Two." (5).
        assert_eq!(resume_offset(&b, &offs, 12, simple_sentences), 5);
        // Exactly at "Two." → "One.".
        assert_eq!(resume_offset(&b, &offs, 5, simple_sentences), 0);
    }

    #[test]
    fn crosses_into_the_previous_block() {
        let b = blocks();
        let offs = block_offsets(&b);
        assert_eq!(offs, vec![0, 18]);
        // Inside "Four." → "Three." of block 0.
        assert_eq!(resume_offset(&b, &offs, 19, simple_sentences), 10);
        // Inside "Five." → "Four.".
        assert_eq!(resume_offset(&b, &offs, 25, simple_sentences), 18);
    }

    #[test]
    fn start_and_empty_documents() {
        let b = blocks();
        let offs = block_offsets(&b);
        assert_eq!(resume_offset(&b, &offs, 0, simple_sentences), 0);
        assert_eq!(resume_offset(&b, &offs, 2, simple_sentences), 0);
        assert_eq!(resume_offset(&[], &[], 40, simple_sentences), 0);
        // Past the end clamps to the last block.
        assert_eq!(resume_offset(&b, &offs, 10_000, simple_sentences), 18);
    }
}
