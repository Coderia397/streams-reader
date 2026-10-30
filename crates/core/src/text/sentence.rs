//! Sentence boundaries: Unicode (UAX #29) sentence breaks, corrected for the two cases where they split English prose
//! that a reader would not: after common abbreviations and initials, and after a quoted question or exclamation that the
//! sentence continues past (`"Why?" she asked.`).

use unicode_segmentation::UnicodeSegmentation;

use super::to_u32;

/// Abbreviations that end in a full stop but rarely end a sentence, compared case-insensitively.
const ABBREVIATIONS: &[&str] =
    &["mr.", "mrs.", "ms.", "dr.", "st.", "jr.", "sr.", "vs.", "etc.", "e.g.", "i.e.", "no.", "fig."];

const CLOSING_QUOTES: &[char] = &['"', '\'', '”', '’', '»', '›'];

/// Sentence ranges `(char_start, char_end)` of one block, end exclusive, trailing whitespace excluded. Uses Unicode
/// sentence boundaries, then re-joins splits after common abbreviations (Mr., Mrs., Ms., Dr., St., Jr., Sr., vs., etc.,
/// e.g., i.e., No., Fig., single capital initials) and after a closing quote that follows `?`/`!` mid-sentence.
///
/// Leading whitespace is excluded too, so every range starts and ends on a non-whitespace char and the ranges cover
/// every non-whitespace char of `text` in order. "Mid-sentence" means the next piece starts with a lowercase letter.
pub fn sentences(text: &str) -> Vec<(u32, u32)> {
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    for (start, raw) in text.split_sentence_bound_indices() {
        let begin = start + (raw.len() - raw.trim_start().len());
        let end = start + raw.trim_end().len();
        if begin >= end {
            continue;
        }
        match pieces.last_mut() {
            Some(last) if continues(&text[last.0..last.1], &text[begin..end]) => last.1 = end,
            _ => pieces.push((begin, end)),
        }
    }
    to_char_ranges(text, &pieces)
}

/// True when the break between `prev` and `next` (both trimmed, non-empty) is not a real sentence end.
fn continues(prev: &str, next: &str) -> bool {
    ends_with_abbreviation(prev) || (ends_with_quoted_question(prev) && next.starts_with(char::is_lowercase))
}

fn ends_with_abbreviation(piece: &str) -> bool {
    let token = piece.rsplit(char::is_whitespace).next().unwrap_or_default();
    // `(e.g.` and `“Dr.` start with punctuation that is not part of the abbreviation.
    let token = token.trim_start_matches(|c: char| !c.is_alphanumeric());
    if ABBREVIATIONS.iter().any(|a| token.eq_ignore_ascii_case(a)) {
        return true;
    }
    let mut chars = token.chars();
    matches!((chars.next(), chars.next(), chars.next()), (Some(c), Some('.'), None) if c.is_uppercase())
}

fn ends_with_quoted_question(piece: &str) -> bool {
    let without_quotes = piece.trim_end_matches(CLOSING_QUOTES);
    without_quotes.len() < piece.len() && without_quotes.ends_with(['?', '!'])
}

/// Convert sorted, non-overlapping byte ranges to char ranges in one pass.
fn to_char_ranges(text: &str, ranges: &[(usize, usize)]) -> Vec<(u32, u32)> {
    let mut byte_at = 0;
    let mut char_at = 0;
    let mut to_char = |byte: usize| {
        char_at += text[byte_at..byte].chars().count();
        byte_at = byte;
        to_u32(char_at)
    };
    ranges
        .iter()
        .map(|&(start, end)| {
            let s = to_char(start);
            (s, to_char(end))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        sentences(text).into_iter().map(|(s, e)| chars[s as usize..e as usize].iter().collect()).collect()
    }

    #[test]
    fn splits_plain_sentences() {
        assert_eq!(split("One two. Three four? Five! Six"), ["One two.", "Three four?", "Five!", "Six"]);
    }

    #[test]
    fn excludes_surrounding_whitespace_and_handles_empty_text() {
        assert_eq!(sentences("  Hi there.  Bye.  "), vec![(2, 11), (13, 17)]);
        assert!(sentences("").is_empty());
        assert!(sentences(" \n ").is_empty());
    }

    #[test]
    fn offsets_are_chars_not_bytes() {
        assert_eq!(sentences("Café é. Ünïcödé."), vec![(0, 7), (8, 16)]);
        assert_eq!(split("“Où?” Là. 😀 Fin."), ["“Où?”", "Là.", "😀 Fin."]);
    }

    #[test]
    fn rejoins_after_abbreviations() {
        assert_eq!(split("Mr. Smith met Dr. Jones. They left."), ["Mr. Smith met Dr. Jones.", "They left."]);
        assert_eq!(
            split("Mrs. Ng, Ms. Lee and St. Paul. Sr. Ana and Jr. Bo."),
            ["Mrs. Ng, Ms. Lee and St. Paul.", "Sr. Ana and Jr. Bo."]
        );
        assert_eq!(split("Fruit, e.g. Apples, i.e. Red ones. Done."), ["Fruit, e.g. Apples, i.e. Red ones.", "Done."]);
        assert_eq!(split("See Fig. 3 and No. 4. Then stop."), ["See Fig. 3 and No. 4.", "Then stop."]);
        assert_eq!(split("Cats vs. Dogs etc. Are fine."), ["Cats vs. Dogs etc. Are fine."]);
        assert_eq!(split("(e.g. This) works. Yes."), ["(e.g. This) works.", "Yes."]);
        assert_eq!(split("DR. WHO came. MR. X too."), ["DR. WHO came.", "MR. X too."]);
    }

    #[test]
    fn rejoins_after_initials() {
        assert_eq!(
            split("J. R. R. Tolkien wrote it. Then he slept."),
            ["J. R. R. Tolkien wrote it.", "Then he slept."]
        );
        // Known trade-off of the initials rule: a sentence ending in a lone capital joins the next one.
        assert_eq!(split("Plan B. Then plan C."), ["Plan B. Then plan C."]);
        assert_eq!(split("It is I. You are he. We go."), ["It is I. You are he.", "We go."]);
    }

    #[test]
    fn does_not_join_after_ordinary_words() {
        assert_eq!(split("Go to bed. Now."), ["Go to bed.", "Now."]);
        assert_eq!(split("Ask Drs. Now."), ["Ask Drs.", "Now."]);
        assert_eq!(split("He was No.1 there. Then left."), ["He was No.1 there.", "Then left."]);
    }

    #[test]
    fn rejoins_after_quoted_question_continued_in_lowercase() {
        assert_eq!(split("“Who is there?” she asked. Nobody."), ["“Who is there?” she asked.", "Nobody."]);
        assert_eq!(split("\"Stop!\" he said."), ["\"Stop!\" he said."]);
        assert_eq!(split("'Really?' 'Yes.'"), ["'Really?'", "'Yes.'"]);
        assert_eq!(split("“Why?” She left."), ["“Why?”", "She left."]);
        assert_eq!(split("Why? she wondered."), ["Why?", "she wondered."]);
    }
}
