//! Splits a block's text into the pieces the reader renders: segment spans (tappable, highlightable) and the
//! whitespace between them. Rendering stays plain text; nothing is ever parsed as HTML.

use nolabs_core::text::Segment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Text between segments (normally a single space).
    Gap(String),
    Seg {
        idx: u32,
        text: String,
    },
}

/// Byte index of every char boundary, plus the end.
fn char_bytes(text: &str) -> Vec<usize> {
    text.char_indices().map(|(b, _)| b).chain(std::iter::once(text.len())).collect()
}

/// Pieces of one block. `block_offset` is the block's char offset in the reading text; `segments` are the block's
/// own segments in order (their offsets are reading-text offsets).
pub fn block_pieces(text: &str, block_offset: u32, segments: &[Segment]) -> Vec<Piece> {
    let bytes = char_bytes(text);
    let len = (bytes.len() - 1) as u32;
    let mut out = Vec::with_capacity(segments.len() * 2 + 1);
    let mut at = 0u32;
    let byte = |c: u32| bytes[c.min(len) as usize];
    for seg in segments {
        let start = seg.char_start.saturating_sub(block_offset).min(len);
        let end = seg.char_end.saturating_sub(block_offset).clamp(start, len);
        if start < at {
            continue;
        }
        if start > at {
            out.push(Piece::Gap(text[byte(at)..byte(start)].to_owned()));
        }
        out.push(Piece::Seg { idx: seg.idx, text: text[byte(start)..byte(end)].to_owned() });
        at = end;
    }
    if at < len {
        out.push(Piece::Gap(text[byte(at)..].to_owned()));
    }
    out
}

/// `(before, word, after)` around the word at char range `start..end` of a segment's text.
pub fn split_word(text: &str, start: u32, end: u32) -> (&str, &str, &str) {
    let bytes = char_bytes(text);
    let last = bytes.len() - 1;
    let s = (start as usize).min(last);
    let e = (end as usize).clamp(s, last);
    (&text[..bytes[s]], &text[bytes[s]..bytes[e]], &text[bytes[e]..])
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::Boundary;

    use super::*;

    fn seg(idx: u32, start: u32, end: u32, text: &str) -> Segment {
        Segment {
            idx,
            block: 0,
            text: text.to_owned(),
            boundary: Boundary::Sentence,
            char_start: start,
            char_end: end,
            heading: false,
        }
    }

    #[test]
    fn pieces_cover_the_block_exactly() {
        let text = "Héllo there. How are you? Fine.";
        // Block starts at offset 100 of the reading text.
        let segs = [seg(4, 100, 112, "Héllo there."), seg(5, 113, 131, "How are you? Fine.")];
        let pieces = block_pieces(text, 100, &segs);
        assert_eq!(
            pieces,
            vec![
                Piece::Seg { idx: 4, text: "Héllo there.".into() },
                Piece::Gap(" ".into()),
                Piece::Seg { idx: 5, text: "How are you? Fine.".into() },
            ]
        );
        let joined: String = pieces
            .iter()
            .map(|p| match p {
                Piece::Gap(t) | Piece::Seg { text: t, .. } => t.as_str(),
            })
            .collect();
        assert_eq!(joined, text);
    }

    #[test]
    fn leading_and_trailing_text_become_gaps() {
        let pieces = block_pieces("  ab  ", 0, &[seg(0, 2, 4, "ab")]);
        assert_eq!(
            pieces,
            vec![Piece::Gap("  ".into()), Piece::Seg { idx: 0, text: "ab".into() }, Piece::Gap("  ".into())]
        );
        assert_eq!(block_pieces("plain", 0, &[]), vec![Piece::Gap("plain".into())]);
    }

    #[test]
    fn out_of_range_segments_are_clamped() {
        let pieces = block_pieces("abc", 0, &[seg(0, 1, 99, "bc")]);
        assert_eq!(pieces, vec![Piece::Gap("a".into()), Piece::Seg { idx: 0, text: "bc".into() }]);
    }

    #[test]
    fn splits_words_by_char_offsets() {
        assert_eq!(split_word("Où est-il ?", 3, 9), ("Où ", "est-il", " ?"));
        assert_eq!(split_word("abc", 5, 9), ("abc", "", ""));
        assert_eq!(split_word("abc", 2, 1), ("ab", "", "c"));
    }
}
