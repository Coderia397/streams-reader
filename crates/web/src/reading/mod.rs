//! Pure reading logic: positions inside a document, the windowed view of long books, highlight pieces and estimates.
//! Nothing here touches the browser, so it is unit tested natively.

pub mod doc;
pub mod pieces;
pub mod resume;
pub mod window;

#[cfg(test)]
mod integration_tests {
    //! The pure reading pieces against the real core segmenter.

    use nolabs_core::proto::api::{Block, BlockKind};
    use nolabs_core::text::{SegmentProfile, segment, sentences};

    use super::doc::DocModel;
    use super::pieces::{Piece, block_pieces};
    use super::resume::resume_offset;

    fn blocks() -> Vec<Block> {
        let para = "The rain had stopped. Mr. Grey walked to the station, slowly. Was anyone there? Nobody answered. \
                    He waited for a long while, listening to the city breathe, and then he turned and went home again.";
        vec![
            Block { kind: BlockKind::Heading, text: "Chapter One".into() },
            Block { kind: BlockKind::Paragraph, text: para.repeat(3) },
            Block { kind: BlockKind::Quote, text: "A short quote.".into() },
            Block { kind: BlockKind::Heading, text: "Chapter Two".into() },
            Block { kind: BlockKind::Paragraph, text: "The end came quietly.".into() },
        ]
    }

    #[test]
    fn pieces_rebuild_every_block_for_both_engines() {
        for profile in [SegmentProfile::KOKORO, SegmentProfile::MING] {
            let blocks = blocks();
            let segments = segment(&blocks, profile);
            let model = DocModel::new(blocks, segments);
            assert_eq!(model.block_segments.len(), model.blocks.len());
            for (i, b) in model.blocks.iter().enumerate() {
                let i = i as u32;
                let segs = model.segments_of_block(i);
                assert!(!segs.is_empty(), "block {i} has no segments");
                let rebuilt: String = block_pieces(&b.text, model.block_offsets[i as usize], segs)
                    .iter()
                    .map(|p| match p {
                        Piece::Gap(t) | Piece::Seg { text: t, .. } => t.as_str(),
                    })
                    .collect();
                assert_eq!(rebuilt, b.text);
            }
            assert_eq!(model.chapters.len(), 2);
            assert_eq!(model.segment_for_offset(0), 0);
        }
    }

    #[test]
    fn resume_lands_on_a_sentence_start_before_the_saved_offset() {
        let blocks = blocks();
        let model = DocModel::new(blocks.clone(), segment(&blocks, SegmentProfile::KOKORO));
        let saved = model.block_offsets[1] + 60;
        let back = resume_offset(&model.blocks, &model.block_offsets, saved, sentences);
        assert!(back < saved);
        assert!(back >= model.block_offsets[1]);
        let local = back - model.block_offsets[1];
        assert!(sentences(&model.blocks[1].text).iter().any(|s| s.0 == local));
    }
}
