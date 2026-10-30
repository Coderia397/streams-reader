//! Listening-oriented clean-up of one block of imported text. Each pass is a small function so the order is explicit:
//! hyphenated line ends must be joined while newlines still exist; citation markers, brackets and footnote marks are
//! cleaned before URLs are rewritten, so a removed marker can never splice two pieces into a new URL on a second pass
//! and the URL pass never sees a bracket; whitespace is collapsed again last because removals leave gaps.

/// Clean one block of imported text for listening:
/// - collapse whitespace runs (including newlines) to single spaces, trim;
/// - join words hyphenated across a line end (`"exam-\nple"` → `"example"`) before collapsing;
/// - drop numeric citation markers (`[12]`, `[3, 4]`, `[5–7]`) and superscript digits used as footnote marks;
/// - turn every other `[`/`]` into `(`/`)` so stray text can never become a Ming paralinguistic tag;
/// - replace bare URLs (`http(s)://…`, `www.…`) with their host without `www.`;
/// - remove zero-width and control characters; keep curly quotes, dashes and ellipses (they shape prosody).
///
/// A hyphen is only joined when a letter precedes it and a lowercase letter follows the line break; before an uppercase
/// letter the hyphen stays and only the break goes (`"Jean-\nPaul"` → `"Jean-Paul"`). Compounds split at a line end
/// (`"well-\nknown"`) are joined, which is the common case in PDFs.
/// A superscript digit run is a footnote mark when it follows a word of three or more letters, closing punctuation or
/// whitespace; after a digit or a one- or two-letter unit (`10²`, `km²`, `mc²`) it is kept as an exponent.
/// The output is idempotent: `normalise(&normalise(s)) == normalise(s)`.
pub fn normalise(text: &str) -> String {
    let joined = join_hyphenated_line_ends(text);
    let visible = strip_invisible(&joined);
    let collapsed = collapse_whitespace(&visible);
    let cleaned = clean_marks(&collapsed);
    let urls = replace_urls(&cleaned);
    collapse_whitespace(&urls)
}

/// Hyphen-like chars that can end a line inside a word. The soft hyphen only ever marks such a break.
const LINE_END_HYPHENS: [char; 3] = ['-', '\u{2010}', '\u{ad}'];

fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

fn join_hyphenated_line_ends(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if LINE_END_HYPHENS.contains(&c) && i > 0 && chars[i - 1].is_alphabetic() {
            let mut j = i + 1;
            let mut saw_break = false;
            while j < chars.len() && chars[j].is_whitespace() {
                saw_break |= is_line_break(chars[j]);
                j += 1;
            }
            let next = chars.get(j).copied();
            if saw_break && (next.is_some_and(char::is_lowercase) || (c == '\u{ad}' && next.is_some())) {
                // A real line-end break inside a word: drop the hyphen and the break.
                i = j;
                continue;
            }
            if saw_break && next.is_some_and(char::is_uppercase) {
                // `Jean-\nPaul`: a hyphenated name split at the hyphen keeps the hyphen but loses the break.
                out.push(c);
                i = j;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Invisible format characters that carry no sound: zero-width spaces and joiners, bidi controls, word joiners, the
/// byte-order mark and the soft hyphen (any left after line-end joining sit mid-word).
fn is_invisible_format(c: char) -> bool {
    matches!(
        c,
        '\u{ad}' | '\u{180e}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}' | '\u{feff}'
    )
}

/// Visible non-ASCII symbols (emoji, modifiers, variation selectors) that a zero-width joiner may glue together.
fn is_symbol(c: char) -> bool {
    !c.is_ascii() && !c.is_alphanumeric() && !c.is_whitespace() && !c.is_control() && !is_invisible_format(c)
}

/// Drop control and invisible format chars. Whitespace controls (newline, tab, …) stay for the collapse pass. A zero-width
/// joiner between two symbols is kept because it forms one emoji (`👩‍🚀`) and the text is also what is displayed.
fn strip_invisible(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let keep = |i: usize, c: char| {
        if c.is_whitespace() {
            return true;
        }
        if c == ZWJ {
            let prev = i.checked_sub(1).and_then(|p| chars.get(p));
            return prev.is_some_and(|&p| is_symbol(p)) && chars.get(i + 1).is_some_and(|&n| is_symbol(n));
        }
        !(c.is_control() || is_invisible_format(c))
    };
    chars.iter().enumerate().filter(|&(i, &c)| keep(i, c)).map(|(_, &c)| c).collect()
}

const ZWJ: char = '\u{200d}';

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ------------------------------------------------------------------------------------------------------------------ URLs

/// Chars that end a URL even without whitespace (quotes and angle brackets around links in prose).
fn ends_url(c: char) -> bool {
    c.is_whitespace() || matches!(c, '<' | '>' | '"' | '“' | '”' | '‘' | '«' | '»')
}

fn replace_urls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut i = 0;
    let mut prev: Option<char> = None;
    while let Some(c) = text[i..].chars().next() {
        if prev.is_none_or(can_precede_url)
            && let Some((len, host)) = match_url(&text[i..])
        {
            out.push_str(&text[copied..i]);
            out.push_str(&host);
            i += len;
            copied = i;
            prev = text[..i].chars().next_back();
            continue;
        }
        prev = Some(c);
        i += c.len_utf8();
    }
    out.push_str(&text[copied..]);
    out
}

/// Letters (including internationalised names), ASCII digits, `-`, `_` and `.`.
fn is_host_char(c: char) -> bool {
    c.is_alphabetic() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.')
}

/// A URL starts at a word start. Host chars, `/` and `@` do not start a word, so a host such as `a.www.b.c` is never
/// matched again on a second pass.
fn can_precede_url(c: char) -> bool {
    !(c.is_alphanumeric() || is_host_char(c) || matches!(c, '/' | '@'))
}

/// If `tail` starts with a URL, its byte length and its host.
fn match_url(tail: &str) -> Option<(usize, String)> {
    let starts_with = |prefix: &str| tail.get(..prefix.len()).is_some_and(|h| h.eq_ignore_ascii_case(prefix));
    let host_start = if starts_with("https://") {
        8
    } else if starts_with("http://") {
        7
    } else if starts_with("www.") {
        0
    } else {
        return None;
    };
    let end = tail.find(ends_url).unwrap_or(tail.len());
    let url = trim_url_end(&tail[..end]);
    if host_start == 0 && url.len() <= "www.".len() {
        return None;
    }
    let host = extract_host(url.get(host_start..)?)?;
    Some((url.len(), host))
}

/// Strip sentence punctuation that follows a URL in prose. A closing parenthesis or brace stays when the URL opened it
/// (`…/Rust_(language)`); square brackets are already parentheses by now.
fn trim_url_end(mut url: &str) -> &str {
    while let Some(c) = url.chars().next_back() {
        let unbalanced = |open: char| url.matches(open).count() < url.matches(c).count();
        let strip = match c {
            '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '’' | '…' | '*' => true,
            ')' => unbalanced('('),
            '}' => unbalanced('{'),
            _ => false,
        };
        if !strip {
            break;
        }
        url = &url[..url.len() - c.len_utf8()];
    }
    url
}

/// Host of the part of a URL after its scheme: no path, query, fragment, credentials, port or leading `www.`.
/// `None` when nothing speakable is left, in which case the text is kept as written.
fn extract_host(rest: &str) -> Option<String> {
    let authority = rest.split(['/', '?', '#', '\\']).next().unwrap_or_default();
    let authority = authority.rsplit('@').next().unwrap_or_default();
    // A host name is letters, digits, `-`, `_` and `.`; anything else (a port, a parenthesis) ends it. IPv6 literals
    // never get here: their brackets became parentheses in the earlier pass, so they are left as written.
    let mut host = &authority[..authority.find(|c: char| !is_host_char(c)).unwrap_or(authority.len())];
    // Strip every leading `www.` so a second pass over the output finds nothing left to strip.
    while host.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("www.")) && host[4..].contains('.') {
        host = &host[4..];
    }
    let host = host.trim_end_matches('.');
    // A host starts with a letter or digit: `http://.x` is not a link.
    host.starts_with(char::is_alphanumeric).then(|| host.to_owned())
}

// --------------------------------------------------------------------------------------- citations, brackets, footnotes

const CITATION_SEPARATORS: [char; 5] = [',', ';', '-', '–', '—'];

fn is_superscript_digit(c: char) -> bool {
    matches!(c, '⁰' | '¹' | '²' | '³' | '⁴'..='⁹')
}

/// Drop citation markers and footnote superscripts, and turn the remaining square brackets into parentheses.
fn clean_marks(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '[' => match citation_end(&chars, i) {
                Some(end) => {
                    // `word [12].` reads as `word.`: the space before the marker goes with it.
                    out.truncate(out.trim_end().len());
                    i = end;
                    continue;
                }
                None => out.push('('),
            },
            ']' => out.push(')'),
            c if is_superscript_digit(c) => {
                let end = i + chars[i..].iter().take_while(|&&d| is_superscript_digit(d)).count();
                if !is_footnote_context(&out) {
                    out.extend(&chars[i..end]);
                }
                i = end;
                continue;
            }
            c => out.push(c),
        }
        i += 1;
    }
    out
}

/// End (exclusive) of a numeric citation such as `[12]`, `[3, 4]` or `[5–7; 9]` starting at `open`.
fn citation_end(chars: &[char], open: usize) -> Option<usize> {
    let skip_spaces = |mut j: usize| {
        while chars.get(j).is_some_and(|c| *c == ' ') {
            j += 1;
        }
        j
    };
    let digits = |j: usize| {
        let n = chars[j.min(chars.len())..].iter().take_while(|c| c.is_ascii_digit()).count();
        (n > 0).then_some(j + n)
    };
    let mut j = digits(skip_spaces(open + 1))?;
    loop {
        j = skip_spaces(j);
        match chars.get(j) {
            Some(']') => return Some(j + 1),
            Some(c) if CITATION_SEPARATORS.contains(c) => j = digits(skip_spaces(j + 1))?,
            _ => return None,
        }
    }
}

/// Decides from the text already emitted whether a superscript digit run that follows it is a footnote mark. Using the
/// output (not the input) keeps the result the same on a second pass after earlier marks were removed.
fn is_footnote_context(before: &str) -> bool {
    let Some(prev) = before.chars().next_back() else { return true };
    if prev.is_whitespace() || matches!(prev, '.' | ',' | ';' | ':' | '!' | '?' | ')' | '"' | '\'' | '”' | '’' | '»')
    {
        return true;
    }
    if prev.is_alphabetic() {
        return before.chars().rev().take_while(|c| c.is_alphabetic()).take(3).count() >= 3;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn check(input: &str, expected: &str) {
        assert_eq!(normalise(input), expected, "input: {input:?}");
    }

    #[test]
    fn collapses_and_trims_whitespace() {
        check("  a \t b\n\n c\r\n ", "a b c");
        check("\u{a0}non\u{2003}breaking\u{3000}", "non breaking");
        check("", "");
        check(" \n\t ", "");
    }

    #[test]
    fn joins_words_hyphenated_across_line_ends() {
        check("exam-\nple", "example");
        check("exam-\r\n   ple text", "example text");
        check("exam\u{ad}\nple", "example");
        check("Exam\u{ad}\nPle", "ExamPle");
        check("exam\u{2010}\nple", "example");
        check("well-\nknown", "wellknown");
    }

    #[test]
    fn keeps_hyphens_that_are_not_line_end_breaks() {
        check("well-known", "well-known");
        check("Jean-\nPaul", "Jean-Paul");
        check("pages 3-\n4", "pages 3- 4");
        check("a - \nb", "a - b");
        check("exam- ple", "exam- ple");
    }

    #[test]
    fn removes_invisible_and_control_chars() {
        check("zero\u{200b}width\u{feff}", "zerowidth");
        check("bell\u{7}\u{1b}[0m", "bell(0m");
        check("rtl\u{202e}mark\u{2066}", "rtlmark");
        check("soft\u{ad}hyphen", "softhyphen");
    }

    #[test]
    fn keeps_prosody_punctuation() {
        check("“Wait…” — she said, ‘no’.", "“Wait…” — she said, ‘no’.");
        check("emoji 👩‍🚀 and ❤️‍🔥 stay", "emoji 👩‍🚀 and ❤️‍🔥 stay");
        check("a\u{200d}b", "ab");
    }

    #[test]
    fn drops_numeric_citations() {
        check("as shown [12].", "as shown.");
        check("as shown[3, 4] before", "as shown before");
        check("range [5–7] and [1-2; 9] done", "range and done");
        check("[12] Leading", "Leading");
        check("a [1][2] b", "a b");
        check("spaced [ 3 , 4 ] ok", "spaced ok");
    }

    #[test]
    fn turns_other_brackets_into_parentheses() {
        check("[laughs] hello", "(laughs) hello");
        check("an [aside] here", "an (aside) here");
        check("[12a] and [] and [1,]", "(12a) and () and (1,)");
        check("unclosed [12 text", "unclosed (12 text");
        check("stray ] bracket", "stray ) bracket");
    }

    #[test]
    fn drops_footnote_superscripts_and_keeps_exponents() {
        check("A claim¹ and another.²³", "A claim and another.");
        check("“Quoted”⁴ text", "“Quoted” text");
        check("¹ See page four.", "See page four.");
        check("an area of 10 m² or 4 km²", "an area of 10 m² or 4 km²");
        check("E = mc² and 2¹⁰", "E = mc² and 2¹⁰");
        check("word[1]²", "word");
    }

    #[test]
    fn replaces_urls_with_their_host() {
        check("see https://www.example.com/a/b?c=d#e now", "see example.com now");
        check("see http://Example.org.", "see Example.org.");
        check("(www.rust-lang.org/learn)", "(rust-lang.org)");
        check("HTTPS://WWW.EXAMPLE.COM", "EXAMPLE.COM");
        check("https://user:pw@host.io:8080/x, then", "host.io, then");
        check("https://en.wikipedia.org/wiki/Rust_(language) is", "en.wikipedia.org is");
        check("\"https://a.example/x\" quoted", "\"a.example\" quoted");
        check("https://[::1]:80/x", "https://(::1):80/x");
        check("www.www.example.com", "example.com");
        check("www.a.b[x]www.c.d", "a.b");
        check("https://a.www.b.c/", "a.www.b.c");
        check("www.a.b¹www.c.d", "a.b");
        check("x.www.a.b", "x.www.a.b");
        check("http://.¹https://www.ex.com/p?q", "http://.https://www.ex.com/p?q");
        check("word[x]http://[3, 4]https://www.ex.com/p?q", "word(x)https");
        check("http://-x.com", "http://-x.com");
        check("http://[www.a.b", "http://(a.b");
        check("http://[x]a.b", "http://(x)a.b");
        check("www.com", "www.com");
    }

    #[test]
    fn leaves_non_urls_alone() {
        check("awww.so cute", "awww.so cute");
        check("xhttps://a.b", "xhttps://a.b");
        check("http:// alone", "http:// alone");
        check("www. is not a host", "www. is not a host");
        check("Thewww.", "Thewww.");
        check("a.¹www.ex.com", "a.www.ex.com");
        check("x¹www.ex.com", "x¹www.ex.com");
    }

    #[test]
    fn citations_inside_urls_are_harmless() {
        check("https://ex.com/[1] and text [2]", "ex.com and text");
    }

    #[test]
    fn is_idempotent_on_examples() {
        for s in ["word [1]² x", "see www.www.a.com.", "exam-\nple [x] ¹ y", "http:// alone", "m² [3]"] {
            let once = normalise(s);
            assert_eq!(normalise(&once), once, "input: {s:?}");
        }
    }

    mod props {
        use super::super::normalise;
        use proptest::prelude::*;

        fn messy_text() -> impl Strategy<Value = String> {
            let pieces = prop::sample::select(vec![
                "word",
                "Word",
                " ",
                "  ",
                "\n",
                "-\n",
                "\u{ad}",
                "[12]",
                "[3, 4]",
                "[x]",
                "[",
                "]",
                "¹",
                "²",
                ".",
                ",",
                "?",
                "!",
                "“",
                "”",
                "…",
                "—",
                "https://www.ex.com/p?q",
                "www.a.b",
                "http://",
                "\u{200b}",
                "\u{7}",
                "é",
                "km",
                "10",
                "(",
                ")",
                "'",
            ]);
            prop::collection::vec(pieces, 0..40).prop_map(|v| v.concat())
        }

        proptest! {
            #[test]
            fn output_is_clean_and_idempotent(s in messy_text()) {
                let out = normalise(&s);
                prop_assert_eq!(normalise(&out), out.clone());
                prop_assert_eq!(out.trim(), out.as_str());
                prop_assert!(!out.contains("  "));
                const GONE: [char; 5] = ['[', ']', '\n', '\u{200b}', '\u{ad}'];
                prop_assert!(!out.contains(GONE), "{:?}", out);
                prop_assert!(!out.chars().any(char::is_control));
            }

            #[test]
            fn arbitrary_input_never_panics(s in any::<String>()) {
                let out = normalise(&s);
                prop_assert_eq!(normalise(&out), out);
            }
        }
    }
}
