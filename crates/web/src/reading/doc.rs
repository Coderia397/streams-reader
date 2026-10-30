//! A document prepared for reading and listening with one voice: its blocks, the segments for that voice's engine,
//! chapters (headings) and display chunks. Every position is a char offset into
//! [`nolabs_core::text::reading_text`], the same unit the Worker stores progress in.

use nolabs_core::proto::api::{Block, BlockKind, WordTiming};
use nolabs_core::text::{Segment, block_offsets, segment_at};

use crate::config::CHARS_PER_SECOND;

/// Display chunks stay under this many chars, so a window of a few chunks renders in one frame on a phone.
const CHUNK_CHARS: u32 = 6_000;
/// A heading starts a new chunk once the current one has at least this many chars (keeps chapters together).
const CHUNK_MIN_AT_HEADING: u32 = 1_500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub title: String,
    pub block: u32,
    /// First segment of the heading block.
    pub segment: u32,
}

/// A run of consecutive blocks rendered together: `first_block..end_block`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    pub first_block: u32,
    pub end_block: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocModel {
    pub blocks: Vec<Block>,
    pub block_offsets: Vec<u32>,
    pub segments: Vec<Segment>,
    /// Segment range `start..end` of each block.
    pub block_segments: Vec<(u32, u32)>,
    pub chapters: Vec<Chapter>,
    pub chunks: Vec<Chunk>,
    pub total_chars: u32,
}

impl DocModel {
    /// `segments` must come from `nolabs_core::text::segment(&blocks, profile)`.
    pub fn new(blocks: Vec<Block>, segments: Vec<Segment>) -> DocModel {
        let block_offsets = block_offsets(&blocks);
        let total_chars =
            blocks.last().map_or(0, |b| block_offsets.last().copied().unwrap_or(0) + b.text.chars().count() as u32);
        let block_segments = block_segment_ranges(blocks.len(), &segments);
        let chapters = blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == BlockKind::Heading)
            .map(|(i, b)| Chapter {
                title: b.text.clone(),
                block: i as u32,
                segment: block_segments.get(i).map_or(0, |r| r.0),
            })
            .collect();
        let chunks = chunk_blocks(&blocks);
        DocModel { blocks, block_offsets, segments, block_segments, chapters, chunks, total_chars }
    }

    pub fn segment_count(&self) -> u32 {
        self.segments.len() as u32
    }

    /// Char offset of a listening position: the segment start, moved to the current word when timings exist.
    pub fn position_offset(&self, segment: u32, word: Option<&WordTiming>) -> u32 {
        let Some(seg) = self.segments.get(segment as usize) else { return self.total_chars };
        match word {
            Some(w) => (seg.char_start + w.2).min(seg.char_end),
            None => seg.char_start,
        }
    }

    /// Segment to start from for a saved char offset.
    pub fn segment_for_offset(&self, offset: u32) -> u32 {
        segment_at(&self.segments, offset).unwrap_or(0) as u32
    }

    /// 0.0–1.0 through the document.
    pub fn fraction_at(&self, offset: u32) -> f64 {
        if self.total_chars == 0 { 0.0 } else { (f64::from(offset) / f64::from(self.total_chars)).clamp(0.0, 1.0) }
    }

    /// Segment under a scrubber position.
    pub fn segment_for_fraction(&self, fraction: f64) -> u32 {
        let offset = (fraction.clamp(0.0, 1.0) * f64::from(self.total_chars)).round() as u32;
        self.segment_for_offset(offset)
    }

    /// Estimated listening time left from `offset` at `rate`.
    pub fn seconds_left(&self, offset: u32, rate: f64) -> f64 {
        seconds_for_chars(u64::from(self.total_chars.saturating_sub(offset)), rate)
    }

    pub fn block_of_segment(&self, segment: u32) -> u32 {
        self.segments.get(segment as usize).map_or(0, |s| s.block)
    }

    pub fn chunk_of_block(&self, block: u32) -> usize {
        self.chunks.partition_point(|c| c.end_block <= block).min(self.chunks.len().saturating_sub(1))
    }

    pub fn chunk_of_segment(&self, segment: u32) -> usize {
        self.chunk_of_block(self.block_of_segment(segment))
    }

    /// Index of the chapter containing `segment` (the last heading at or before it).
    pub fn chapter_at(&self, segment: u32) -> Option<usize> {
        let i = self.chapters.partition_point(|c| c.segment <= segment);
        i.checked_sub(1)
    }

    /// Segments of one block.
    pub fn segments_of_block(&self, block: u32) -> &[Segment] {
        match self.block_segments.get(block as usize) {
            Some(&(start, end)) => &self.segments[start as usize..end as usize],
            None => &[],
        }
    }
}

/// Listening time for `chars` at `rate` (estimate).
pub fn seconds_for_chars(chars: u64, rate: f64) -> f64 {
    chars as f64 / (CHARS_PER_SECOND * rate.max(0.1))
}

fn block_segment_ranges(block_count: usize, segments: &[Segment]) -> Vec<(u32, u32)> {
    let mut ranges = Vec::with_capacity(block_count);
    let mut i = 0usize;
    for block in 0..block_count as u32 {
        let start = i;
        while i < segments.len() && segments[i].block == block {
            i += 1;
        }
        ranges.push((start as u32, i as u32));
    }
    ranges
}

/// Groups blocks into display chunks of at most [`CHUNK_CHARS`], breaking early at headings.
pub fn chunk_blocks(blocks: &[Block]) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut first = 0u32;
    let mut chars = 0u32;
    for (i, b) in blocks.iter().enumerate() {
        let i = i as u32;
        let len = b.text.chars().count() as u32;
        let heading_break = b.kind == BlockKind::Heading && chars >= CHUNK_MIN_AT_HEADING;
        let size_break = chars > 0 && chars + len > CHUNK_CHARS;
        if i > first && (heading_break || size_break) {
            chunks.push(Chunk { first_block: first, end_block: i });
            first = i;
            chars = 0;
        }
        chars += len;
    }
    if (first as usize) < blocks.len() {
        chunks.push(Chunk { first_block: first, end_block: blocks.len() as u32 });
    }
    chunks
}

#[cfg(test)]
pub(crate) mod tests {
    use nolabs_core::proto::api::Boundary;

    use super::*;

    pub fn block(kind: BlockKind, text: &str) -> Block {
        Block { kind, text: text.to_owned() }
    }

    /// One segment per block, covering the whole block (enough for position maths).
    pub fn one_segment_per_block(blocks: &[Block]) -> Vec<Segment> {
        let offsets = block_offsets(blocks);
        blocks
            .iter()
            .enumerate()
            .map(|(i, b)| Segment {
                idx: i as u32,
                block: i as u32,
                text: b.text.clone(),
                boundary: Boundary::Paragraph,
                char_start: offsets[i],
                char_end: offsets[i] + b.text.chars().count() as u32,
                heading: b.kind == BlockKind::Heading,
            })
            .collect()
    }

    fn model() -> DocModel {
        let blocks = vec![
            block(BlockKind::Heading, "One"),
            block(BlockKind::Paragraph, "Hello there."),
            block(BlockKind::Heading, "Two"),
            block(BlockKind::Paragraph, "Last words."),
        ];
        let segs = one_segment_per_block(&blocks);
        DocModel::new(blocks, segs)
    }

    #[test]
    fn totals_offsets_and_chapters() {
        let m = model();
        // "One" 3 + 2 + "Hello there." 12 + 2 + "Two" 3 + 2 + "Last words." 11
        assert_eq!(m.block_offsets, vec![0, 5, 19, 24]);
        assert_eq!(m.total_chars, 35);
        assert_eq!(m.chapters.len(), 2);
        assert_eq!(m.chapters[1], Chapter { title: "Two".into(), block: 2, segment: 2 });
        assert_eq!(m.block_segments, vec![(0, 1), (1, 2), (2, 3), (3, 4)]);
        assert_eq!(m.segments_of_block(1).len(), 1);
        assert!(m.segments_of_block(9).is_empty());
    }

    #[test]
    fn positions_and_fractions() {
        let m = model();
        assert_eq!(m.position_offset(1, None), 5);
        assert_eq!(m.position_offset(1, Some(&WordTiming(0, 100, 6, 11))), 11);
        assert_eq!(m.position_offset(1, Some(&WordTiming(0, 100, 600, 700))), 17);
        assert_eq!(m.position_offset(99, None), 35);
        assert_eq!(m.segment_for_offset(20), 2);
        assert_eq!(m.segment_for_fraction(0.0), 0);
        assert_eq!(m.segment_for_fraction(1.0), 3);
        assert!((m.fraction_at(35) - 1.0).abs() < f64::EPSILON);
        assert_eq!(m.chapter_at(1), Some(0));
        assert_eq!(m.chapter_at(3), Some(1));
    }

    #[test]
    fn time_left_scales_with_rate() {
        let m = model();
        let at_1x = m.seconds_left(5, 1.0);
        assert!((at_1x - 30.0 / CHARS_PER_SECOND).abs() < 1e-9);
        assert!((m.seconds_left(5, 2.0) - at_1x / 2.0).abs() < 1e-9);
    }

    #[test]
    fn chunks_break_by_size_and_at_headings() {
        let long = "x".repeat(2_000);
        let blocks: Vec<Block> = (0..10).map(|_| block(BlockKind::Paragraph, &long)).collect();
        let chunks = chunk_blocks(&blocks);
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0], Chunk { first_block: 0, end_block: 3 });
        assert_eq!(chunks.last().map(|c| c.end_block), Some(10));

        let mut blocks = vec![block(BlockKind::Paragraph, &long), block(BlockKind::Heading, "H")];
        blocks.push(block(BlockKind::Paragraph, "short"));
        let chunks = chunk_blocks(&blocks);
        assert_eq!(chunks, vec![Chunk { first_block: 0, end_block: 1 }, Chunk { first_block: 1, end_block: 3 }]);
        assert!(chunk_blocks(&[]).is_empty());
    }

    #[test]
    fn chunk_lookup() {
        let long = "x".repeat(2_000);
        let blocks: Vec<Block> = (0..10).map(|_| block(BlockKind::Paragraph, &long)).collect();
        let segs = one_segment_per_block(&blocks);
        let m = DocModel::new(blocks, segs);
        assert_eq!(m.chunk_of_block(0), 0);
        assert_eq!(m.chunk_of_block(3), 1);
        assert_eq!(m.chunk_of_segment(9), 3);
        assert_eq!(m.chunk_of_block(500), 3);
    }
}
