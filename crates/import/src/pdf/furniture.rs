//! Page furniture: running headers, footers and page numbers. They sit on the first or last lines of a page and are
//! never part of the reading text.

use std::collections::HashMap;

use super::Line;

/// How many lines at the top and at the bottom of a page can be furniture.
const EDGE_LINES: usize = 3;
/// A header must repeat on at least this many pages.
const MIN_REPEATS: usize = 3;
/// …and on at least this share of the pages between its first and last occurrence. Running headers appear on every
/// page (or every other page when they alternate); chapter headings at the top of a page repeat far more sparsely.
const MIN_DENSITY: f32 = 0.4;
/// Positions closer than this (page units) count as the same position.
const POSITION_BUCKET: f32 = 4.0;
/// Lines this much taller than body text are headings, not furniture.
const TALL_FACTOR: f32 = 1.2;

/// Removes repeating headers/footers and bare page numbers. `body_height` is the document's typical line height.
pub(super) fn strip(pages: Vec<Vec<Line>>, body_height: f32) -> Vec<Vec<Line>> {
    let tall = |l: &Line| body_height > 0.0 && l.height > body_height * TALL_FACTOR;

    // Pages on which each (text shape, position) occurs among the edge lines.
    let mut seen: HashMap<(String, i64), Vec<usize>> = HashMap::new();
    for (p, lines) in pages.iter().enumerate() {
        for (i, line) in lines.iter().enumerate() {
            if is_edge(i, lines.len()) && !tall(line) {
                let pages_seen = seen.entry(furniture_key(line)).or_default();
                if pages_seen.last() != Some(&p) {
                    pages_seen.push(p);
                }
            }
        }
    }
    let repeating = |line: &Line| {
        seen.get(&furniture_key(line)).is_some_and(|ps| {
            let (Some(first), Some(last)) = (ps.first(), ps.last()) else { return false };
            let span = (last - first + 1) as f32;
            ps.len() >= MIN_REPEATS && ps.len() as f32 >= span * MIN_DENSITY
        })
    };

    pages
        .into_iter()
        .map(|lines| {
            let n = lines.len();
            lines
                .into_iter()
                .enumerate()
                .filter(|(i, line)| !(is_edge(*i, n) && !tall(line) && (repeating(line) || is_page_number(&line.text))))
                .map(|(_, line)| line)
                .collect()
        })
        .collect()
}

fn is_edge(i: usize, len: usize) -> bool {
    i < EDGE_LINES || i + EDGE_LINES >= len
}

/// Text with digit runs folded to `#` (page numbers change from page to page) and the position bucket.
fn furniture_key(line: &Line) -> (String, i64) {
    let mut shape = String::with_capacity(line.text.len());
    for c in line.text.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_digit() {
            if !shape.ends_with('#') {
                shape.push('#');
            }
        } else if !c.is_whitespace() {
            shape.push(c);
        }
    }
    (shape, (line.y / POSITION_BUCKET).round() as i64)
}

/// `12`, `- 12 -`, `[12]`, `Page 12`, `p. 12`, `12 / 300`, `Page 3 of 10`, `xiv`.
pub(super) fn is_page_number(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    let t = t.trim_matches(|c: char| "-–—|·•[](){} ".contains(c));
    let t = ["page ", "pg. ", "pg ", "p. "].iter().find_map(|p| t.strip_prefix(p)).unwrap_or(t).trim();
    if t.is_empty() {
        return false;
    }
    let parts: Vec<&str> = if let Some((a, b)) = t.split_once(" of ") {
        vec![a, b]
    } else if let Some((a, b)) = t.split_once('/') {
        vec![a, b]
    } else {
        vec![t]
    };
    parts.iter().all(|p| {
        let p = p.trim();
        let digits = !p.is_empty() && p.len() <= 5 && p.chars().all(|c| c.is_ascii_digit());
        let roman = parts.len() == 1 && p.len() <= 7 && is_roman(p);
        digits || roman
    })
}

/// A well-formed lower-case Roman numeral (`xiv` yes, `civic` and `iiii` no), as used for front-matter page numbers.
fn is_roman(s: &str) -> bool {
    const NUMERALS: [(u32, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let value = |c: char| match c {
        'i' => Some(1),
        'v' => Some(5),
        'x' => Some(10),
        'l' => Some(50),
        'c' => Some(100),
        'd' => Some(500),
        'm' => Some(1000),
        _ => None,
    };
    let Some(digits) = s.chars().map(value).collect::<Option<Vec<u32>>>() else { return false };
    let mut total = 0u32;
    for (i, d) in digits.iter().enumerate() {
        match digits.get(i + 1) {
            Some(next) if next > d => total = total.saturating_sub(*d),
            _ => total += d,
        }
    }
    // Re-encode the value canonically; only canonical spellings are page numbers.
    let mut canonical = String::new();
    let mut rest = total;
    for (v, sym) in NUMERALS {
        while rest >= v {
            canonical.push_str(sym);
            rest -= v;
        }
    }
    total > 0 && canonical == s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, y: f32) -> Line {
        Line { text: text.into(), y, height: 10.0 }
    }

    /// Distinct words per page, so body lines never repeat (real body text does not).
    const WORDS: [&str; 6] = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"];

    fn body(n: usize, what: &str) -> String {
        format!("{what} {}", WORDS[n % WORDS.len()])
    }

    fn page(n: usize, body_lines: &[&str]) -> Vec<Line> {
        let mut lines = vec![line("THE BOOK TITLE", 780.0)];
        lines.extend(body_lines.iter().enumerate().map(|(i, t)| line(&body(n, t), 700.0 - 12.0 * i as f32)));
        lines.push(line(&n.to_string(), 40.0));
        lines
    }

    fn texts(pages: &[Vec<Line>]) -> Vec<Vec<String>> {
        pages.iter().map(|p| p.iter().map(|l| l.text.clone()).collect()).collect()
    }

    #[test]
    fn drops_running_headers_and_page_numbers() {
        let pages: Vec<Vec<Line>> = (1..=4).map(|n| page(n, &["Body one", "Body two", "Body three"])).collect();
        let out = strip(pages, 10.0);
        for (n, p) in texts(&out).into_iter().enumerate() {
            assert_eq!(p, vec![body(n + 1, "Body one"), body(n + 1, "Body two"), body(n + 1, "Body three")]);
        }
    }

    #[test]
    fn keeps_headers_seen_on_fewer_than_three_pages() {
        let pages: Vec<Vec<Line>> = (1..=2).map(|n| page(n, &["Body"])).collect();
        let out = strip(pages, 10.0);
        // Page numbers still go; the two-page "header" stays.
        assert_eq!(
            texts(&out),
            vec![
                vec!["THE BOOK TITLE".to_owned(), body(1, "Body")],
                vec!["THE BOOK TITLE".to_owned(), body(2, "Body")]
            ]
        );
    }

    #[test]
    fn header_with_changing_page_number_is_furniture() {
        let pages: Vec<Vec<Line>> =
            (1..=5).map(|n| vec![line(&format!("{n}  Chapter Title"), 780.0), line(&body(n, "Text"), 700.0)]).collect();
        let out = strip(pages, 10.0);
        for (i, p) in texts(&out).into_iter().enumerate() {
            assert_eq!(p, vec![body(i + 1, "Text")]);
        }
    }

    #[test]
    fn sparse_chapter_headings_are_kept() {
        // "Chapter N" at the top of pages 0, 10, 20: three repeats but far too sparse to be a running header.
        let pages: Vec<Vec<Line>> = (0..21)
            .map(|p| {
                let top = if p % 10 == 0 { format!("Chapter {}", p / 10 + 1) } else { format!("Body {p}") };
                vec![line(&top, 780.0), line("words", 700.0)]
            })
            .collect();
        let out = strip(pages, 10.0);
        assert_eq!(out[10][0].text, "Chapter 2");
    }

    #[test]
    fn tall_lines_are_never_furniture() {
        let pages: Vec<Vec<Line>> = (1..=4)
            .map(|n| vec![Line { text: "IV".into(), y: 780.0, height: 20.0 }, line(&body(n, "Body"), 700.0)])
            .collect();
        let out = strip(pages, 10.0);
        assert!(out.iter().all(|p| p.len() == 2));
    }

    #[test]
    fn page_number_shapes() {
        for yes in ["12", " - 12 - ", "[3]", "Page 12", "p. 7", "12 / 300", "Page 3 of 10", "xiv", "— 5 —"] {
            assert!(is_page_number(yes), "{yes}");
        }
        for no in ["", "12 apples", "Chapter 3", "1984 was a year", "123456", "civic duty", "civic", "iiii", "-", "dim"]
        {
            assert!(!is_page_number(no), "{no}");
        }
    }
}
