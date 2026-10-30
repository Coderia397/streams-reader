//! PDF text from pdf.js (lines with positions, one list per page) to blocks: clean the lines, drop page furniture,
//! then rebuild headings and paragraphs from line spacing.

mod furniture;
mod layout;

use nolabs_core::proto::api::{Block, BlockKind};

use crate::common::{check_text_size, clean_title, finish, short_line_title};
use crate::limits::MAX_PDF_PAGES;
use crate::{ImportError, Imported, PdfLine};

/// A cleaned line: trimmed, ligatures expanded, finite coordinates.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Line {
    pub text: String,
    pub y: f32,
    pub height: f32,
}

#[derive(Debug, Default)]
pub(crate) struct Extracted {
    pub title: Option<String>,
    pub blocks: Vec<Block>,
}

pub(crate) fn import(pages: &[Vec<PdfLine>], title_hint: Option<&str>) -> Result<Imported, ImportError> {
    let ex = extract(pages, title_hint)?;
    finish(ex.title, None, ex.blocks)
}

pub(crate) fn extract(pages: &[Vec<PdfLine>], title_hint: Option<&str>) -> Result<Extracted, ImportError> {
    if pages.len() > MAX_PDF_PAGES {
        return Err(ImportError::TooLarge("the PDF has more than 20,000 pages"));
    }
    check_text_size(pages.iter().flatten().map(|l| l.text.len()).sum())?;

    let pages: Vec<Vec<Line>> = pages.iter().map(|p| clean_page(p)).collect();
    let mut heights: Vec<f32> = pages.iter().flatten().map(|l| l.height).filter(|h| *h > 0.0).collect();
    let body_height = layout::median(&mut heights).unwrap_or(0.0);
    let pages = furniture::strip(pages, body_height);
    let blocks = layout::blocks(&pages, body_height);

    let title = title_hint
        .and_then(clean_title)
        .or_else(|| blocks.iter().take_while(|b| b.kind == BlockKind::Heading).find_map(|b| short_line_title(&b.text)));
    Ok(Extracted { title, blocks })
}

/// Trims lines, drops empty ones, expands typographic ligatures and repairs non-finite positions (a line without a
/// usable position takes the previous line's, so it joins that line's paragraph).
fn clean_page(lines: &[PdfLine]) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    let mut last_y = 0.0f32;
    for l in lines {
        let text = expand_ligatures(l.text.trim());
        if text.is_empty() {
            continue;
        }
        let y = if l.y.is_finite() { l.y } else { last_y };
        let height = if l.height.is_finite() && l.height > 0.0 { l.height } else { 0.0 };
        last_y = y;
        out.push(Line { text, y, height });
    }
    out
}

/// pdf.js returns the ligature code points that fonts use (`ﬁ`, `ﬂ`); they are read as nonsense if left in.
fn expand_ligatures(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'ﬀ' => out.push_str("ff"),
            'ﬁ' => out.push_str("fi"),
            'ﬂ' => out.push_str("fl"),
            'ﬃ' => out.push_str("ffi"),
            'ﬄ' => out.push_str("ffl"),
            'ﬅ' | 'ﬆ' => out.push_str("st"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn l(text: &str, y: f32, height: f32) -> PdfLine {
        PdfLine { text: text.into(), y, height }
    }

    /// A small three-page "paper": running header, page numbers, a title, two paragraphs crossing a page break.
    fn sample() -> Vec<Vec<PdfLine>> {
        let body = [
            "Lighthouses were once tended by keepers who lived on",
            "site and climbed the tower every evening to light the",
            "lamp. Their logbooks record weather, ships and the",
            "small events of isolated lives.",
            "Automation arrived in the second half of the century",
            "and the last keepers left their posts within a few dec-",
            "ades, taking with them a way of life that had lasted",
        ];
        let mut p1 = vec![l("Journal of Coastal History", 800.0, 8.0), l("The Last Keepers", 740.0, 20.0)];
        p1.extend(body[..4].iter().enumerate().map(|(i, t)| l(t, 700.0 - 14.0 * i as f32, 10.0)));
        p1.push(l(body[4], 630.0, 10.0));
        p1.push(l(body[5], 616.0, 10.0));
        p1.push(l("1", 40.0, 8.0));
        let p2 = vec![
            l("Journal of Coastal History", 800.0, 8.0),
            l(body[6], 760.0, 10.0),
            l("for two centuries on this ﬁnal coast.", 746.0, 10.0),
            l("- 2 -", 40.0, 8.0),
        ];
        let p3 = vec![
            l("Journal of Coastal History", 800.0, 8.0),
            l("Notes and sources follow on later pages of this issue.", 760.0, 10.0),
            l("3", 40.0, 8.0),
        ];
        vec![p1, p2, p3]
    }

    #[test]
    fn rebuilds_paragraphs_and_drops_furniture() -> TestResult {
        let ex = extract(&sample(), None)?;
        assert_eq!(ex.title.as_deref(), Some("The Last Keepers"));
        let kinds: Vec<BlockKind> = ex.blocks.iter().map(|b| b.kind).collect();
        assert_eq!(kinds, vec![BlockKind::Heading, BlockKind::Paragraph, BlockKind::Paragraph, BlockKind::Paragraph]);
        let all: String = ex.blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n");
        assert!(!all.contains("Journal of Coastal History"), "{all}");
        assert!(!all.contains("- 2 -"), "{all}");
        assert!(ex.blocks[1].text.ends_with("isolated lives."), "{:?}", ex.blocks[1]);
        assert!(ex.blocks[2].text.contains("few decades, taking"), "{:?}", ex.blocks[2]);
        assert!(ex.blocks[2].text.ends_with("on this final coast."), "{:?}", ex.blocks[2]);
        Ok(())
    }

    #[test]
    fn title_hint_wins() -> TestResult {
        let ex = extract(&sample(), Some("paper.pdf"))?;
        assert_eq!(ex.title.as_deref(), Some("paper.pdf"));
        Ok(())
    }

    #[test]
    fn empty_and_broken_lines() -> TestResult {
        let pages = vec![vec![l("  ", 10.0, 10.0), l("Text", f32::NAN, f32::INFINITY), l("more", 0.0, -1.0)]];
        let ex = extract(&pages, None)?;
        assert_eq!(ex.blocks.len(), 1);
        assert_eq!(ex.blocks[0].text, "Text more");
        assert!(extract(&[], None)?.blocks.is_empty());
        Ok(())
    }

    #[test]
    fn too_many_pages() {
        let pages = vec![Vec::new(); MAX_PDF_PAGES + 1];
        assert!(matches!(extract(&pages, None), Err(ImportError::TooLarge(_))));
    }

    #[test]
    fn ligatures() {
        assert_eq!(expand_ligatures("ﬁne ﬂow eﬃcient"), "fine flow efficient");
    }
}
