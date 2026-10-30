//! EPUB content documents (XHTML) to an element tree. quick-xml is used rather than an HTML parser because XHTML
//! self-closing tags (`<script src="x"/>`, `<a id="p12"/>`) would swallow the rest of the chapter under HTML rules.
//! Parsing is lenient: mismatched end tags are tolerated, and an XML error ends the document with what was read.

use quick_xml::Reader;
use quick_xml::events::Event;

use super::xml::{attributes, local_name, resolve_ref};
use crate::dom::{Element, Node};
use crate::limits::MAX_TREE_DEPTH;

/// The document as a tree under a synthetic root.
pub(crate) fn parse(xhtml: &str) -> Element {
    let mut reader = Reader::from_str(xhtml);
    let config = reader.config_mut();
    config.check_end_names = false;
    config.allow_unmatched_ends = true;

    // Open elements; `stack[0]` is the root. Elements opened beyond MAX_TREE_DEPTH are not pushed, only counted, so
    // their text lands in the deepest kept element.
    let mut stack: Vec<Element> = vec![Element::new("#root")];
    let mut overflow = 0usize;

    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(ev) => ev,
        };
        match event {
            Event::Start(e) => {
                if stack.len() > MAX_TREE_DEPTH {
                    overflow += 1;
                } else {
                    stack.push(Element { name: local_name(&e), attrs: attributes(&e), children: Vec::new() });
                }
            }
            Event::Empty(e) => {
                let el = Element { name: local_name(&e), attrs: attributes(&e), children: Vec::new() };
                append(&mut stack, Node::Element(el));
            }
            Event::End(e) => {
                if overflow > 0 {
                    overflow -= 1;
                    continue;
                }
                let name = e.local_name().as_ref().to_ascii_lowercase();
                // Close up to the matching open element; an end tag with no open match is ignored.
                if let Some(pos) = stack.iter().rposition(|el| el.name == name).filter(|&p| p > 0) {
                    while stack.len() > pos {
                        close_top(&mut stack);
                    }
                }
            }
            Event::Text(t) => append_text(&mut stack, &t.xml10_content()),
            Event::CData(t) => append_text(&mut stack, &t.xml10_content()),
            Event::GeneralRef(r) => {
                if let Some(s) = resolve_ref(&r) {
                    append_text(&mut stack, &s);
                }
            }
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) | Event::Eof => {}
        }
    }
    while stack.len() > 1 {
        close_top(&mut stack);
    }
    stack.pop().unwrap_or_else(|| Element::new("#root"))
}

fn close_top(stack: &mut Vec<Element>) {
    if stack.len() > 1
        && let Some(el) = stack.pop()
    {
        append(stack, Node::Element(el));
    }
}

fn append(stack: &mut [Element], node: Node) {
    if let Some(top) = stack.last_mut() {
        top.children.push(node);
    }
}

/// Appends text, merging with a preceding text node (entity references split text into several events).
fn append_text(stack: &mut [Element], s: &str) {
    let Some(top) = stack.last_mut() else { return };
    if let Some(Node::Text(prev)) = top.children.last_mut() {
        prev.push_str(s);
    } else {
        top.children.push(Node::Text(s.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::BlockKind;

    use super::*;
    use crate::tree_blocks::blocks_from_tree;

    fn blocks(xhtml: &str) -> Vec<(BlockKind, String)> {
        blocks_from_tree(&parse(xhtml))
            .into_iter()
            .map(|b| (b.kind, b.text.split_whitespace().collect::<Vec<_>>().join(" ")))
            .collect()
    }

    #[test]
    fn self_closing_tags_do_not_swallow_content() {
        let x = r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Ch 1</title><script src="x.js"/><link rel="stylesheet" href="s.css"/></head>
<body><h1>Chapter&#160;One</h1><p><a id="p1"/>It was a <i>dark</i>&nbsp;and stormy night&mdash;truly.</p>
<p>Second<br/>line.</p></body></html>"#;
        assert_eq!(
            blocks(x),
            vec![
                (BlockKind::Heading, "Chapter One".into()),
                (BlockKind::Paragraph, "It was a dark and stormy night—truly.".into()),
                (BlockKind::Paragraph, "Second line.".into()),
            ]
        );
    }

    #[test]
    fn epub_semantics_are_skipped() {
        let x = r##"<html><body>
<section epub:type="chapter"><p>Text<a epub:type="noteref" href="#n1">1</a>.</p>
<aside epub:type="footnote" id="n1"><p>Note text.</p></aside>
<span epub:type="pagebreak" title="5">5</span></section>
<nav epub:type="toc"><ol><li>Contents</li></ol></nav></body></html>"##;
        assert_eq!(blocks(x), vec![(BlockKind::Paragraph, "Text.".into())]);
    }

    #[test]
    fn mismatched_and_truncated_markup_is_tolerated() {
        let x = "<html><body><p>One <b>bold</p><p>Two</i></p><p>Three <em>cut off";
        assert_eq!(
            blocks(x),
            vec![
                (BlockKind::Paragraph, "One bold".into()),
                (BlockKind::Paragraph, "Two".into()),
                (BlockKind::Paragraph, "Three cut off".into()),
            ]
        );
    }

    #[test]
    fn deep_nesting_keeps_text() {
        let depth = MAX_TREE_DEPTH * 3;
        let x = format!("<body>{}<p>Deep.</p>{}</body>", "<div>".repeat(depth), "</div>".repeat(depth));
        let out = blocks(&x);
        assert_eq!(out, vec![(BlockKind::Paragraph, "Deep.".into())]);
    }

    #[test]
    fn cdata_and_unknown_entities() {
        let x = "<body><p><![CDATA[a < b]]> &unknown; ok</p></body>";
        assert_eq!(blocks(x), vec![(BlockKind::Paragraph, "a < b ok".into())]);
    }
}
