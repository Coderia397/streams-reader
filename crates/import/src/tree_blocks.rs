//! Turns an element tree ([`crate::dom`]) into blocks. Shared by the HTML and EPUB importers.
//!
//! Rules: `h1`–`h6` become headings, `blockquote` quotes, `li` list items, every other block-level element ends the
//! current paragraph. Navigation, scripts, styles, media, footnotes and note references are skipped. Code (`pre`) and
//! data tables (with `th`, `thead` or `caption`) become a short "skipped" paragraph; layout tables are read cell by cell.

use nolabs_core::proto::api::{Block, BlockKind};

use crate::common::{BlockBuffer, CODE_SKIPPED, TABLE_SKIPPED};
use crate::dom::{Element, Node};

/// Elements never read aloud, with all their content.
const SKIPPED_ELEMENTS: &[&str] = &[
    "script",
    "style",
    "noscript",
    "template",
    "head",
    "title",
    "nav",
    "svg",
    "math",
    "iframe",
    "object",
    "embed",
    "video",
    "audio",
    "canvas",
    "img",
    "picture",
    "map",
    "button",
    "input",
    "select",
    "textarea",
    "form",
    "figcaption",
    "rt",
    "rp",
    "meta",
    "link",
    "base",
];

/// `epub:type` values (and their `doc-` ARIA roles) whose content is not part of the reading flow.
const SKIPPED_SEMANTICS: &[&str] = &[
    "footnote",
    "footnotes",
    "endnote",
    "endnotes",
    "rearnote",
    "rearnotes",
    "note",
    "noteref",
    "toc",
    "landmarks",
    "page-list",
    "pagebreak",
];

/// Class names marking footnote lists in HTML generated from Markdown or wikis.
const SKIPPED_CLASSES: &[&str] = &["footnotes", "footnote", "references", "reflist", "mw-references-wrap"];

/// Elements that end the current paragraph before and after their content.
const CONTAINER_ELEMENTS: &[&str] = &[
    "p", "div", "section", "article", "main", "header", "footer", "aside", "figure", "address", "center", "details",
    "summary", "fieldset", "legend", "ul", "ol", "dl", "dt", "dd", "menu", "table", "thead", "tbody", "tfoot", "tr",
    "td", "th", "caption", "body", "html", "hgroup", "search",
];

/// A footnote marker in `<sup>` is at most this many chars ("12", "[3]", "*", "a,b").
const SUP_MARKER_CHARS: usize = 6;

/// Blocks of every element below `root`, in document order (not yet normalised).
pub(crate) fn blocks_from_tree(root: &Element) -> Vec<Block> {
    let mut buf = BlockBuffer::default();
    walk_children(root, BlockKind::Paragraph, &mut buf);
    buf.into_blocks(BlockKind::Paragraph)
}

fn walk_children(el: &Element, kind: BlockKind, buf: &mut BlockBuffer) {
    for child in &el.children {
        match child {
            Node::Text(t) => buf.push_str(t),
            Node::Element(e) => walk_element(e, kind, buf),
        }
    }
}

fn walk_element(el: &Element, kind: BlockKind, buf: &mut BlockBuffer) {
    if is_skipped(el) {
        return;
    }
    let name = el.name.as_str();
    match name {
        "br" => buf.push_char('\n'),
        "hr" => buf.flush(kind),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => block(el, kind, BlockKind::Heading, buf),
        "blockquote" => {
            let inner = if kind == BlockKind::Heading { kind } else { BlockKind::Quote };
            block(el, kind, inner, buf);
        }
        "li" => {
            let inner = if kind == BlockKind::Heading { kind } else { BlockKind::ListItem };
            block(el, kind, inner, buf);
        }
        "pre" => buf.push_block(kind, BlockKind::Paragraph, CODE_SKIPPED),
        "table" if is_data_table(el) => buf.push_block(kind, BlockKind::Paragraph, TABLE_SKIPPED),
        "q" => {
            buf.push_char('“');
            walk_children(el, kind, buf);
            buf.push_char('”');
        }
        _ if CONTAINER_ELEMENTS.contains(&name) => block(el, kind, kind, buf),
        _ => walk_children(el, kind, buf),
    }
}

/// Flushes pending text as `outer`, reads the element as `inner`, then flushes it as `inner`.
fn block(el: &Element, outer: BlockKind, inner: BlockKind, buf: &mut BlockBuffer) {
    buf.flush(outer);
    walk_children(el, inner, buf);
    buf.flush(inner);
}

fn is_skipped(el: &Element) -> bool {
    SKIPPED_ELEMENTS.contains(&el.name.as_str())
        || el.attr("hidden").is_some()
        || el.attr("aria-hidden").is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
        || el.attr("style").is_some_and(hides_element)
        || SKIPPED_SEMANTICS.iter().any(|s| el.has_semantic(s))
        || SKIPPED_CLASSES.iter().any(|c| el.attr_has_token("class", c))
        || (el.name == "sup" && is_footnote_marker(el))
}

fn hides_element(style: &str) -> bool {
    let compact: String = style.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    compact.contains("display:none") || compact.contains("visibility:hidden")
}

/// A `<sup>` holding a link, or only digits and note symbols, is a footnote marker.
fn is_footnote_marker(sup: &Element) -> bool {
    let text = sup.text_content();
    let text = text.trim();
    if text.chars().count() > SUP_MARKER_CHARS {
        return false;
    }
    sup.contains_element(&["a"]) || text.chars().all(|c| c.is_ascii_digit() || "[](),*†‡§¶ ".contains(c))
}

fn is_data_table(table: &Element) -> bool {
    table.contains_element(&["th", "thead", "caption"])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny builder: `e("p", &[], vec![t("x")])`.
    fn e(name: &str, attrs: &[(&str, &str)], children: Vec<Node>) -> Node {
        Node::Element(Element {
            name: name.into(),
            attrs: attrs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect(),
            children,
        })
    }

    fn t(s: &str) -> Node {
        Node::Text(s.into())
    }

    fn run(children: Vec<Node>) -> Vec<(BlockKind, String)> {
        let root = Element { name: "body".into(), attrs: vec![], children };
        blocks_from_tree(&root)
            .into_iter()
            .map(|b| (b.kind, b.text.split_whitespace().collect::<Vec<_>>().join(" ")))
            .collect()
    }

    use BlockKind::{Heading, ListItem, Paragraph, Quote};

    #[test]
    fn maps_block_elements_to_kinds() {
        let out = run(vec![
            e("h1", &[], vec![t("Title")]),
            e("p", &[], vec![t("One "), e("em", &[], vec![t("two")]), t(" three.")]),
            e("blockquote", &[], vec![e("p", &[], vec![t("Quoted.")]), t("Loose quote.")]),
            e("ul", &[], vec![e("li", &[], vec![t("First")]), e("li", &[], vec![e("p", &[], vec![t("Second")])])]),
            t("Trailing text."),
        ]);
        assert_eq!(
            out,
            vec![
                (Heading, "Title".into()),
                (Paragraph, "One two three.".into()),
                (Quote, "Quoted.".into()),
                (Quote, "Loose quote.".into()),
                (ListItem, "First".into()),
                (ListItem, "Second".into()),
                (Paragraph, "Trailing text.".into()),
            ]
        );
    }

    #[test]
    fn nested_lists_split_parent_and_child_items() {
        let out = run(vec![e(
            "ol",
            &[],
            vec![e("li", &[], vec![t("Parent"), e("ul", &[], vec![e("li", &[], vec![t("Child")])]), t("Tail")])],
        )]);
        assert_eq!(out, vec![(ListItem, "Parent".into()), (ListItem, "Child".into()), (ListItem, "Tail".into())]);
    }

    #[test]
    fn skips_navigation_scripts_and_footnotes() {
        let out = run(vec![
            e("nav", &[], vec![e("li", &[], vec![t("Chapter 1")])]),
            e("script", &[], vec![t("var x = 1;")]),
            e("style", &[], vec![t("p { }")]),
            e(
                "p",
                &[],
                vec![
                    t("Body"),
                    e("a", &[("epub:type", "noteref"), ("href", "#n1")], vec![t("1")]),
                    e("sup", &[], vec![e("a", &[("href", "#n2")], vec![t("2")])]),
                    e("sup", &[], vec![t("[3]")]),
                    t(" text."),
                ],
            ),
            e("aside", &[("epub:type", "footnote")], vec![e("p", &[], vec![t("A footnote.")])]),
            e("aside", &[("role", "doc-endnote")], vec![t("An endnote.")]),
            e("section", &[("class", "footnotes")], vec![e("ol", &[], vec![e("li", &[], vec![t("Note")])])]),
            e("div", &[("hidden", "")], vec![t("Hidden.")]),
            e("div", &[("style", "display: none")], vec![t("Hidden too.")]),
            e("span", &[("aria-hidden", "true")], vec![t("Icon")]),
            e("figure", &[], vec![e("img", &[("alt", "A cat")], vec![]), e("figcaption", &[], vec![t("Caption")])]),
        ]);
        assert_eq!(out, vec![(Paragraph, "Body text.".into())]);
    }

    #[test]
    fn keeps_meaningful_superscript_text() {
        let out = run(vec![e("p", &[], vec![t("The 4"), e("sup", &[], vec![t("th of July")]), t(".")])]);
        assert_eq!(out, vec![(Paragraph, "The 4th of July.".into())]);
    }

    #[test]
    fn code_and_data_tables_are_replaced() {
        let out = run(vec![
            e("p", &[], vec![t("Before.")]),
            e("pre", &[], vec![e("code", &[], vec![t("fn main() {}")])]),
            e("table", &[], vec![e("tr", &[], vec![e("th", &[], vec![t("A")])])]),
            e("p", &[], vec![t("After.")]),
        ]);
        assert_eq!(
            out,
            vec![
                (Paragraph, "Before.".into()),
                (Paragraph, CODE_SKIPPED.into()),
                (Paragraph, TABLE_SKIPPED.into()),
                (Paragraph, "After.".into()),
            ]
        );
    }

    #[test]
    fn layout_tables_are_read_cell_by_cell() {
        let out = run(vec![e(
            "table",
            &[],
            vec![e("tr", &[], vec![e("td", &[], vec![t("Left")]), e("td", &[], vec![t("Right")])])],
        )]);
        assert_eq!(out, vec![(Paragraph, "Left".into()), (Paragraph, "Right".into())]);
    }

    #[test]
    fn line_breaks_and_rules() {
        let out = run(vec![
            e("p", &[], vec![t("Line one"), e("br", &[], vec![]), t("line two")]),
            t("Scene"),
            e("hr", &[], vec![]),
            t("break"),
        ]);
        assert_eq!(
            out,
            vec![(Paragraph, "Line one line two".into()), (Paragraph, "Scene".into()), (Paragraph, "break".into())]
        );
    }

    #[test]
    fn inline_quotes_get_quote_marks() {
        let out = run(vec![e("p", &[], vec![t("She said "), e("q", &[], vec![t("hi")]), t(".")])]);
        assert_eq!(out, vec![(Paragraph, "She said “hi”.".into())]);
    }

    #[test]
    fn headings_keep_their_kind_inside_containers() {
        let out =
            run(vec![e("blockquote", &[], vec![e("h2", &[], vec![e("div", &[], vec![t("Inner")])]), t("Quote")])]);
        assert_eq!(out, vec![(Heading, "Inner".into()), (Quote, "Quote".into())]);
    }
}
