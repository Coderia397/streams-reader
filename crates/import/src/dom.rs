//! A minimal element tree that both the HTML importer (html5ever via dom_query) and the EPUB importer (XHTML via
//! quick-xml) build, so one walker ([`crate::tree_blocks`]) turns either into blocks with the same rules.

/// One node of the tree. Comments, doctypes and processing instructions are dropped while building.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Element {
    /// Lower-case local name (`p`, `h1`, `aside`).
    pub name: String,
    /// Attributes by lower-case qualified name (`class`, `epub:type`).
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Self::default() }
    }

    pub(crate) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// True when the whitespace-separated attribute value contains `token` (case-insensitive).
    pub(crate) fn attr_has_token(&self, name: &str, token: &str) -> bool {
        self.attr(name).is_some_and(|v| v.split_whitespace().any(|t| t.eq_ignore_ascii_case(token)))
    }

    /// Structural semantics from EPUB 3 `epub:type` and DPUB-ARIA `role`, e.g. `footnote` or `doc-footnote`.
    pub(crate) fn has_semantic(&self, token: &str) -> bool {
        self.attr_has_token("epub:type", token)
            || self.attr("role").is_some_and(|v| {
                v.split_whitespace().any(|t| t.strip_prefix("doc-").is_some_and(|t| t.eq_ignore_ascii_case(token)))
            })
    }

    /// Concatenated text of every descendant, in document order.
    pub(crate) fn text_content(&self) -> String {
        let mut out = String::new();
        let mut stack: Vec<&Node> = self.children.iter().rev().collect();
        while let Some(node) = stack.pop() {
            match node {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) => stack.extend(e.children.iter().rev()),
            }
        }
        out
    }

    /// True if any descendant element has one of `names`.
    pub(crate) fn contains_element(&self, names: &[&str]) -> bool {
        let mut stack: Vec<&Node> = self.children.iter().collect();
        while let Some(node) = stack.pop() {
            if let Node::Element(e) = node {
                if names.contains(&e.name.as_str()) {
                    return true;
                }
                stack.extend(e.children.iter());
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(name: &str, attrs: &[(&str, &str)], children: Vec<Node>) -> Node {
        Node::Element(Element {
            name: name.into(),
            attrs: attrs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect(),
            children,
        })
    }

    #[test]
    fn text_content_is_in_document_order() {
        let tree = Element {
            name: "div".into(),
            attrs: vec![],
            children: vec![
                Node::Text("a".into()),
                el("b", &[], vec![Node::Text("b".into()), el("i", &[], vec![Node::Text("c".into())])]),
                Node::Text("d".into()),
            ],
        };
        assert_eq!(tree.text_content(), "abcd");
        assert!(tree.contains_element(&["i"]));
        assert!(!tree.contains_element(&["table"]));
    }

    #[test]
    fn semantics_from_epub_type_or_role() {
        let Node::Element(a) = el("aside", &[("epub:type", "rearnote footnote")], vec![]) else { return };
        assert!(a.has_semantic("footnote"));
        let Node::Element(b) = el("aside", &[("role", "doc-footnote")], vec![]) else { return };
        assert!(b.has_semantic("footnote"));
        assert!(!b.has_semantic("toc"));
    }
}
