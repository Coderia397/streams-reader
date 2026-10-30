use super::*;
use crate::text::{reading_text, sentences};

fn para(text: &str) -> Block {
    Block { kind: BlockKind::Paragraph, text: text.to_owned() }
}

fn heading(text: &str) -> Block {
    Block { kind: BlockKind::Heading, text: text.to_owned() }
}

fn greedy(min_chars: u32, max_chars: u32) -> SegmentProfile {
    SegmentProfile { min_chars, max_chars, paragraph_mode: false }
}

fn paragraphs(min_chars: u32, max_chars: u32) -> SegmentProfile {
    SegmentProfile { min_chars, max_chars, paragraph_mode: true }
}

fn texts(segments: &[Segment]) -> Vec<&str> {
    segments.iter().map(|s| s.text.as_str()).collect()
}

#[test]
fn engine_profiles_match_the_architecture() {
    assert_eq!(SegmentProfile::for_engine(Engine::Kokoro), SegmentProfile::KOKORO);
    assert_eq!(SegmentProfile::for_engine(Engine::Ming), SegmentProfile::MING);
    assert_eq!(SegmentProfile::KOKORO.merge_limit(), 400);
    assert_eq!(SegmentProfile::MING.merge_limit(), 625);
}

#[test]
fn packs_whole_sentences_greedily() {
    let blocks = [para("Aa aa. Bb bb. Cc cc. Dd dd.")];
    // Each sentence is 6 chars; two with the space between are 13.
    let segs = segment(&blocks, greedy(0, 13));
    assert_eq!(texts(&segs), ["Aa aa. Bb bb.", "Cc cc. Dd dd."]);
    let segs = segment(&blocks, greedy(0, 12));
    assert_eq!(texts(&segs), ["Aa aa.", "Bb bb.", "Cc cc.", "Dd dd."]);
    let segs = segment(&blocks, greedy(0, 100));
    assert_eq!(texts(&segs), ["Aa aa. Bb bb. Cc cc. Dd dd."]);
}

#[test]
fn merges_a_short_trailing_remainder_within_five_quarters() {
    let blocks = [para("Aaaaaaaaa. Bbbbbbbbb. Cc.")];
    // Packing at 21 gives ["Aaaaaaaaa. Bbbbbbbbb.", "Cc."]; the 3-char tail merges because 25 <= 21 * 5 / 4 = 26.
    assert_eq!(texts(&segment(&blocks, greedy(5, 21))), ["Aaaaaaaaa. Bbbbbbbbb. Cc."]);
    // A tail at least `min_chars` long stays alone.
    assert_eq!(texts(&segment(&blocks, greedy(3, 21))), ["Aaaaaaaaa. Bbbbbbbbb.", "Cc."]);
    // At 20 the last two sentences pack together, so there is no short tail.
    assert_eq!(texts(&segment(&blocks, greedy(5, 20))), ["Aaaaaaaaa.", "Bbbbbbbbb. Cc."]);
    // Too long to merge: the 5-char tail would make 27 > 26.
    let blocks = [para("Aaaaaaaaa. Bbbbbbbbb. Cccc.")];
    assert_eq!(texts(&segment(&blocks, greedy(6, 21))), ["Aaaaaaaaa. Bbbbbbbbb.", "Cccc."]);
}

#[test]
fn never_merges_across_blocks() {
    let blocks = [para("Long sentence here."), para("Hi.")];
    let segs = segment(&blocks, greedy(10, 40));
    assert_eq!(texts(&segs), ["Long sentence here.", "Hi."]);
    assert_eq!(segs[1].block, 1);
}

#[test]
fn splits_an_overlong_sentence_after_a_clause_mark_in_the_second_half() {
    let blocks = [para("one two three four, five six seven eight nine ten.")];
    let segs = segment(&blocks, greedy(0, 30));
    assert_eq!(texts(&segs), ["one two three four,", "five six seven eight nine ten."]);
}

#[test]
fn splits_an_overlong_sentence_at_the_last_space_when_no_late_clause_mark() {
    let blocks = [para("ab, cd ef gh ij kl mn op qr st uv wx yz.")];
    let segs = segment(&blocks, greedy(0, 16));
    // The comma sits in the first half of the 16-char window, so the last space before the limit wins.
    assert_eq!(texts(&segs), ["ab, cd ef gh ij", "kl mn op qr st", "uv wx yz."]);
    assert!(segs.iter().all(|s| s.text.chars().count() <= 16));
}

#[test]
fn a_space_exactly_at_the_limit_is_a_valid_cut() {
    let blocks = [para("abcd efgh")];
    assert_eq!(texts(&segment(&blocks, greedy(0, 4))), ["abcd", "efgh"]);
}

#[test]
fn cuts_hard_when_there_is_no_space() {
    let blocks = [para("abcdefghij")];
    assert_eq!(texts(&segment(&blocks, greedy(0, 4))), ["abcd", "efgh", "ij"]);
    // Falls back to an early clause mark before cutting a word.
    let blocks = [para("ab,cdefghij")];
    assert_eq!(texts(&segment(&blocks, greedy(0, 8))), ["ab,", "cdefghij"]);
}

#[test]
fn the_tail_of_a_split_sentence_packs_with_the_next_sentence() {
    let blocks = [para("aaaa bbbb cccc. Dd.")];
    assert_eq!(texts(&segment(&blocks, greedy(0, 10))), ["aaaa bbbb", "cccc. Dd."]);
}

#[test]
fn headings_are_their_own_segments() {
    let blocks = [heading("Chapter One"), para("Short."), para("Also short.")];
    let segs = segment(&blocks, greedy(0, 100));
    assert_eq!(texts(&segs), ["Chapter One", "Short.", "Also short."]);
    assert!(segs[0].heading);
    assert!(!segs[1].heading && !segs[2].heading);
}

#[test]
fn heading_text_is_not_split_into_sentences() {
    let blocks = [heading("Part 1. The Start")];
    assert_eq!(texts(&segment(&blocks, greedy(0, 10))), ["Part 1.", "The Start"]);
    assert_eq!(texts(&segment(&blocks, greedy(0, 100))), ["Part 1. The Start"]);
}

#[test]
fn boundaries_follow_block_structure() {
    let blocks = [
        heading("Title"),
        para("One. Two."),
        Block { kind: BlockKind::Quote, text: "Quote.".into() },
        para("   "),
        heading("Next"),
        Block { kind: BlockKind::ListItem, text: "Item.".into() },
    ];
    let segs = segment(&blocks, greedy(0, 6));
    let got: Vec<(&str, Boundary)> = segs.iter().map(|s| (s.text.as_str(), s.boundary)).collect();
    assert_eq!(
        got,
        [
            ("Title", Boundary::Section),
            ("One.", Boundary::Sentence),
            ("Two.", Boundary::Paragraph),
            // The whitespace-only block is skipped, so a heading follows.
            ("Quote.", Boundary::Section),
            ("Next", Boundary::Section),
            ("Item.", Boundary::Paragraph),
        ]
    );
    let blocks_of: Vec<u32> = segs.iter().map(|s| s.block).collect();
    assert_eq!(blocks_of, [0, 1, 1, 2, 4, 5]);
    let idx: Vec<u32> = segs.iter().map(|s| s.idx).collect();
    assert_eq!(idx, [0, 1, 2, 3, 4, 5]);
}

#[test]
fn split_heading_pieces_continue_with_sentence_boundaries() {
    let segs = segment(&[heading("aaaa bbbb")], greedy(0, 4));
    let got: Vec<(&str, Boundary, bool)> = segs.iter().map(|s| (s.text.as_str(), s.boundary, s.heading)).collect();
    assert_eq!(got, [("aaaa", Boundary::Sentence, true), ("bbbb", Boundary::Section, true)]);
}

#[test]
fn offsets_point_into_the_reading_text() {
    let blocks = [heading(" Café "), para("  Ünï. Çø ðé.  ")];
    let segs = segment(&blocks, greedy(0, 6));
    let reading: Vec<char> = reading_text(&blocks).chars().collect();
    assert_eq!(texts(&segs), ["Café", "Ünï.", "Çø ðé."]);
    for s in &segs {
        let slice: String = reading[s.char_start as usize..s.char_end as usize].iter().collect();
        assert_eq!(slice, s.text);
    }
    assert_eq!((segs[0].char_start, segs[0].char_end), (1, 5));
    assert_eq!((segs[1].char_start, segs[1].char_end), (10, 14));
}

#[test]
fn empty_input_gives_no_segments() {
    assert!(segment(&[], SegmentProfile::KOKORO).is_empty());
    assert!(segment(&[para(""), heading("  ")], SegmentProfile::MING).is_empty());
}

#[test]
fn zero_max_chars_does_not_loop() {
    let segs = segment(&[para("ab cd")], greedy(0, 0));
    assert_eq!(texts(&segs), ["a", "b", "c", "d"]);
}

#[test]
fn paragraph_mode_keeps_a_fitting_block_whole() {
    let blocks = [para("One. Two. Three. Four.")];
    assert_eq!(texts(&segment(&blocks, paragraphs(0, 22))), ["One. Two. Three. Four."]);
}

#[test]
fn paragraph_mode_splits_at_the_sentence_boundary_nearest_the_middle() {
    // Sentences start at 0, 7, 14, 27; the middle of 0..33 is 16, so the boundary at 14 wins.
    let blocks = [para("Aaaaa. Bbbbb. Ccccccccccc. Ddddd.")];
    assert_eq!(texts(&segment(&blocks, paragraphs(0, 20))), ["Aaaaa. Bbbbb.", "Ccccccccccc. Ddddd."]);
}

#[test]
fn paragraph_mode_recurses_and_splits_single_long_sentences() {
    let blocks = [para("Aaaa. Bbbb. Cccc. Dddd. Eeee eeee eeee eeee.")];
    let segs = segment(&blocks, paragraphs(0, 11));
    assert_eq!(texts(&segs), ["Aaaa. Bbbb.", "Cccc. Dddd.", "Eeee eeee", "eeee eeee."]);
}

#[test]
fn paragraph_mode_merges_a_short_trailing_piece() {
    let blocks = [para("Aaaaaaaaaaaaaaaaaaa. Bb.")];
    // Split gives [20, 3]; 24 <= 20 * 5 / 4 = 25, so the tail merges back.
    assert_eq!(texts(&segment(&blocks, paragraphs(5, 20))), ["Aaaaaaaaaaaaaaaaaaa. Bb."]);
}

#[test]
fn kokoro_and_ming_profiles_on_realistic_text() {
    let sentence = "The quick brown fox jumps over the lazy dog while the farmer watches from the porch.";
    let text = [sentence; 12].join(" ");
    let blocks = [heading("Chapter 1"), para(&text)];
    for profile in [SegmentProfile::KOKORO, SegmentProfile::MING] {
        let segs = segment(&blocks, profile);
        assert_eq!(segs[0].text, "Chapter 1");
        for s in &segs[1..] {
            let n = s.text.chars().count() as u32;
            assert!(n <= profile.max_chars, "{n} chars in {profile:?}");
            assert!(s.text.ends_with('.'), "split inside a sentence: {:?}", s.text);
        }
    }
}

mod props {
    use super::*;
    use crate::text::block_offsets;
    use proptest::prelude::*;

    fn block_text() -> impl Strategy<Value = String> {
        let pieces = prop::sample::select(vec![
            "a",
            "word",
            "Word",
            "extraordinarily",
            "Mr.",
            "e.g.",
            "J.",
            "x",
            "é",
            "😀",
            "—",
            "“",
            "”",
            "\"",
            "1.5",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            " ",
            " ",
            " ",
            " ",
            "  ",
            ", ",
            "; ",
            ": ",
            ". ",
            "? ",
            "! ",
            "?” ",
            "\n",
            "\t",
            "...",
        ]);
        prop::collection::vec(pieces, 0..60).prop_map(|v| v.concat())
    }

    fn blocks() -> impl Strategy<Value = Vec<Block>> {
        let kind = prop::sample::select(vec![
            BlockKind::Heading,
            BlockKind::Paragraph,
            BlockKind::Paragraph,
            BlockKind::Quote,
            BlockKind::ListItem,
        ]);
        prop::collection::vec((kind, block_text()).prop_map(|(kind, text)| Block { kind, text }), 0..6)
    }

    fn profile() -> impl Strategy<Value = SegmentProfile> {
        (0u32..60, 1u32..120, any::<bool>()).prop_map(|(min_chars, max_chars, paragraph_mode)| SegmentProfile {
            min_chars,
            max_chars,
            paragraph_mode,
        })
    }

    fn check_invariants(blocks: &[Block], profile: SegmentProfile, segs: &[Segment]) -> Result<(), TestCaseError> {
        let reading: Vec<char> = reading_text(blocks).chars().collect();
        let offsets = block_offsets(blocks);
        let max = profile.max_chars.max(1);
        let mut covered = vec![false; reading.len()];
        let mut prev_end = 0;
        for (i, s) in segs.iter().enumerate() {
            let b = s.block as usize;
            let block = &blocks[b];
            let (start, end) = (s.char_start as usize, s.char_end as usize);
            prop_assert_eq!(s.idx as usize, i);
            prop_assert!(s.char_start >= prev_end, "segments overlap or are out of order");
            prev_end = s.char_end;
            // Exact slice of the reading text, trimmed, inside its own block.
            let slice: String = reading[start..end].iter().collect();
            prop_assert_eq!(&slice, &s.text);
            prop_assert!(!s.text.is_empty());
            prop_assert_eq!(s.text.trim(), s.text.as_str());
            prop_assert!(offsets[b] <= s.char_start);
            prop_assert!(s.char_end as usize <= offsets[b] as usize + block.text.chars().count());
            // Headings alone: heading segments come only from heading blocks and hold nothing else.
            prop_assert_eq!(s.heading, block.kind == BlockKind::Heading);
            // Size: within max, except a merged tail, which is the block's last segment and within 5/4 of max.
            let n = s.char_end - s.char_start;
            let last_of_block = segs.get(i + 1).is_none_or(|next| next.block != s.block);
            prop_assert!(n <= max || (last_of_block && n <= profile.merge_limit().max(max)), "{n} chars");
            prop_assert_eq!(s.boundary == Boundary::Sentence, !last_of_block);
            if last_of_block {
                let next_heading = segs.get(i + 1).is_some_and(|next| next.heading);
                let expected = if s.heading || next_heading { Boundary::Section } else { Boundary::Paragraph };
                prop_assert_eq!(s.boundary, expected);
            }
            covered[start..end].iter_mut().for_each(|c| *c = true);
        }
        // Coverage: every non-whitespace char is spoken, in order.
        for (p, c) in reading.iter().enumerate() {
            prop_assert!(c.is_whitespace() || covered[p], "char {p} ({c:?}) is in no segment");
        }
        for (b, block) in blocks.iter().enumerate() {
            let in_block: Vec<&Segment> = segs.iter().filter(|s| s.block as usize == b).collect();
            let off = offsets[b];
            if block.kind != BlockKind::Heading {
                // A sentence that fits is never split.
                for (s, e) in sentences(&block.text) {
                    if e - s <= max {
                        prop_assert!(
                            in_block.iter().any(|seg| seg.char_start <= off + s && off + e <= seg.char_end),
                            "sentence {s}..{e} was split"
                        );
                    }
                }
            }
            let trimmed = block.text.trim().chars().count() as u32;
            if profile.paragraph_mode && trimmed > 0 && trimmed <= max {
                prop_assert_eq!(in_block.len(), 1, "a fitting paragraph was split");
            }
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        #[test]
        fn segmentation_invariants_hold(blocks in blocks(), profile in profile()) {
            let segs = segment(&blocks, profile);
            check_invariants(&blocks, profile, &segs)?;
            prop_assert_eq!(segment(&blocks, profile), segs, "segmentation is not deterministic");
        }

        #[test]
        fn invariants_hold_for_arbitrary_unicode(
            texts in prop::collection::vec(any::<String>(), 0..4),
            profile in profile(),
        ) {
            let blocks: Vec<Block> = texts.into_iter().map(|text| Block { kind: BlockKind::Paragraph, text }).collect();
            let segs = segment(&blocks, profile);
            check_invariants(&blocks, profile, &segs)?;
        }

        #[test]
        fn engine_profiles_hold_on_normalised_text(blocks in blocks(), ming in any::<bool>()) {
            let blocks = crate::text::normalise_blocks(blocks);
            let profile = if ming { SegmentProfile::MING } else { SegmentProfile::KOKORO };
            check_invariants(&blocks, profile, &segment(&blocks, profile))?;
        }
    }
}
