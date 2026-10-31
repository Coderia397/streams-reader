//! Search inside a document: case-insensitive matches with a short snippet around each, mapped to the segment to
//! play from. Pure, so it is unit tested natively.

use super::doc::DocModel;

/// Characters of context shown on each side of a match.
const CONTEXT: usize = 36;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub segment: u32,
    pub before: String,
    pub matched: String,
    pub after: String,
}

/// Lowercased chars of `text`, one per original char (so indices line up; multi-char lowercase forms keep only
/// their first char, which is enough for matching).
fn folded(text: &str) -> Vec<char> {
    text.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}

/// Moves a context cut that starts mid-word (`from`, before the match at `i`) forward to just after the next
/// whitespace. A run without whitespace (one long word, or Chinese) keeps the cut.
fn word_start(chars: &[char], from: usize, i: usize) -> usize {
    if from == 0 || chars[from - 1].is_whitespace() {
        return from;
    }
    chars[from..i].iter().position(|c| c.is_whitespace()).map_or(from, |p| from + p + 1)
}

/// Moves a context cut that ends mid-word (`to`, after the match ending at `end`) back to the last whitespace.
fn word_end(chars: &[char], end: usize, to: usize) -> usize {
    if to >= chars.len() || chars[to].is_whitespace() {
        return to;
    }
    chars[end..to].iter().rposition(|c| c.is_whitespace()).map_or(to, |p| end + p)
}

/// Up to `limit` matches of `query`, in reading order. Queries shorter than two chars find nothing.
pub fn find(model: &DocModel, query: &str, limit: usize) -> Vec<Hit> {
    let q = folded(query.trim());
    if q.len() < 2 {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (b, block) in model.blocks.iter().enumerate() {
        let chars: Vec<char> = block.text.chars().collect();
        let low = folded(&block.text);
        let mut i = 0;
        while i + q.len() <= low.len() {
            if low[i..i + q.len()] != q[..] {
                i += 1;
                continue;
            }
            let offset = model.block_offsets.get(b).copied().unwrap_or(0) + i as u32;
            let end = i + q.len();
            let from = word_start(&chars, i.saturating_sub(CONTEXT), i);
            let to = word_end(&chars, end, (end + CONTEXT).min(chars.len()));
            let mut before: String = chars[from..i].iter().collect();
            if from > 0 {
                before.insert(0, '…');
            }
            let mut after: String = chars[end..to].iter().collect();
            if to < chars.len() {
                after.push('…');
            }
            hits.push(Hit {
                segment: model.segment_for_offset(offset),
                before,
                matched: chars[i..end].iter().collect(),
                after,
            });
            if hits.len() >= limit {
                return hits;
            }
            i += q.len();
        }
    }
    hits
}

/// Documents in the library matching a query on title, author or source (case-insensitive, every word must match).
pub fn matches_words(haystack: &str, query: &str) -> bool {
    let h = haystack.to_lowercase();
    query.split_whitespace().all(|w| h.contains(&w.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::BlockKind;

    use super::*;
    use crate::reading::doc::tests::{block, one_segment_per_block};

    fn model() -> DocModel {
        let blocks = vec![
            block(BlockKind::Heading, "The Station"),
            block(BlockKind::Paragraph, "Rain on the station roof. The STATION was empty."),
        ];
        let segs = one_segment_per_block(&blocks);
        DocModel::new(blocks, segs)
    }

    #[test]
    fn finds_every_match_in_order_with_context() {
        let hits = find(&model(), "station", 10);
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].segment, 0);
        assert_eq!(hits[1].segment, 1);
        assert_eq!(hits[1].before, "Rain on the ");
        assert_eq!(hits[2].matched, "STATION");
        assert_eq!(find(&model(), "station", 2).len(), 2);
        assert!(find(&model(), "x", 10).is_empty());
        assert!(find(&model(), "train", 10).is_empty());
    }

    #[test]
    fn long_context_is_trimmed() {
        let blocks = vec![block(BlockKind::Paragraph, &format!("{}needle{}", "a".repeat(80), "b".repeat(80)))];
        let segs = one_segment_per_block(&blocks);
        let hits = find(&DocModel::new(blocks, segs), "NEEDLE", 5);
        assert!(hits[0].before.starts_with('…') && hits[0].after.ends_with('…'));
        assert_eq!(hits[0].before.chars().count(), CONTEXT + 1);
    }

    #[test]
    fn snippets_start_and_end_on_word_boundaries() {
        let text = "Alice was beginning to get very much out of the way of things, and she thought it over: \
                    the White Rabbit hurried by, saying Oh dear! I shall be late! and its watch was gone.";
        let blocks = vec![block(BlockKind::Paragraph, text)];
        let segs = one_segment_per_block(&blocks);
        let model = DocModel::new(blocks, segs);
        let words: Vec<&str> = text.split_whitespace().collect();
        let hits = find(&model, "th", 50);
        assert_eq!(hits.len(), 4);
        assert!(hits.iter().any(|h| h.before.starts_with('…') && h.after.ends_with('…')));
        for h in &hits {
            let before = h.before.trim_start_matches('…');
            let after = h.after.trim_end_matches('…');
            if h.before.starts_with('…') && !before.is_empty() {
                let first = before.split_whitespace().next().unwrap_or_default();
                assert!(words.contains(&first), "snippet starts inside a word: {:?}", h.before);
            }
            if h.after.ends_with('…') && !after.is_empty() {
                let last = after.split_whitespace().last().unwrap_or_default();
                assert!(words.contains(&last), "snippet ends inside a word: {:?}", h.after);
            }
        }
    }

    #[test]
    fn word_matching() {
        assert!(matches_words("The Old Man and the Sea · Book", "old sea"));
        assert!(!matches_words("Dune", "dune messiah"));
        assert!(matches_words("anything", "  "));
    }
}
