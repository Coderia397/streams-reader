//! Web pages: readability (dom_smoothie, a port of Mozilla's Readability.js) finds the article, then the shared tree
//! walker turns it into blocks. When readability cannot find an article (very short pages), the whole `<body>` is read.

use dom_query::{Document, NodeRef};
use dom_smoothie::{Config, Readability, ReadabilityError};
use nolabs_core::proto::api::{Block, BlockKind};

use crate::common::{check_text_size, clean_author, clean_title, finish};
use crate::dom::{Element, Node};
use crate::limits::{MAX_HTML_ELEMENTS, MAX_TREE_DEPTH};
use crate::tree_blocks::blocks_from_tree;
use crate::{ImportError, Imported};

/// What the page yields before normalisation.
#[derive(Debug, Default)]
pub(crate) struct Extracted {
    pub title: Option<String>,
    pub author: Option<String>,
    pub blocks: Vec<Block>,
}

pub(crate) fn import(html: &str, url: Option<&str>) -> Result<Imported, ImportError> {
    let ex = extract(html, url)?;
    let title = ex.title.or_else(|| url.and_then(host_of));
    finish(title, ex.author, ex.blocks)
}

pub(crate) fn extract(html: &str, url: Option<&str>) -> Result<Extracted, ImportError> {
    check_text_size(html.len())?;
    let config = Config {
        // Keep classes so footnote sections (`class="footnotes"`) can still be recognised and skipped.
        keep_classes: true,
        max_elements_to_parse: MAX_HTML_ELEMENTS,
        ..Config::default()
    };
    let mut readability = match Readability::new(html, url, Some(config.clone())) {
        Ok(r) => r,
        // A relative or odd URL only helps resolve links, which are never read; parse without it.
        Err(_) => Readability::new(html, None, Some(config)).map_err(|e| ImportError::Malformed(e.to_string()))?,
    };
    let article = match readability.parse() {
        Ok(article) => Some(article),
        Err(ReadabilityError::TooManyElements(..)) => {
            return Err(ImportError::TooLarge("the page has too many elements"));
        }
        Err(ReadabilityError::GrabFailed | ReadabilityError::BadDocumentURL) => None,
    };

    let mut ex = Extracted::default();
    if let Some(article) = article {
        ex.blocks = blocks_from_html(&article.content, false);
        ex.title = clean_title(&article.title);
        ex.author = article.byline.clone();
    }
    if !has_text(&ex.blocks) {
        // Readability found nothing worth reading (or gave up); fall back to every block of the original body.
        ex.blocks = blocks_from_html(html, true);
    }
    if ex.title.is_none() {
        ex.title = document_title(html);
    }
    if let Some(title) = ex.title.take() {
        ex.title = Some(without_site_name(title, html, &ex.blocks));
    }
    if let Some(author) = ex.author.as_deref() {
        drop_byline_block(&mut ex.blocks, author);
    }
    Ok(ex)
}

/// How many leading blocks may hold the byline that readability already reported.
const BYLINE_SEARCH_BLOCKS: usize = 3;

/// Removes a leading paragraph that only repeats the byline ("By Jane Doe"); the author is shown separately.
fn drop_byline_block(blocks: &mut Vec<Block>, author: &str) {
    let Some(author) = clean_author(author).map(|a| a.to_lowercase()) else { return };
    let is_byline = |b: &Block| clean_author(&b.text).is_some_and(|t| t.to_lowercase() == author);
    if let Some(i) = blocks.iter().take(BYLINE_SEARCH_BLOCKS).position(is_byline) {
        blocks.remove(i);
    }
}

/// Separators between an article title and the site name in `<title>`.
const TITLE_SEPARATORS: &[&str] = &[" | ", " - ", " – ", " — ", " :: ", " · ", " • ", " / ", " » "];

/// `Story | Site` → `Story` when a heading of the page reads exactly `Story`. Readability keeps short titles whole
/// (it cannot tell which part is the site name) and drops the article heading that repeats the title, so the
/// original page's `h1`/`h2` are consulted too; the page is only parsed again when the title has a separator.
fn without_site_name(title: String, html: &str, blocks: &[Block]) -> String {
    if !TITLE_SEPARATORS.iter().any(|sep| title.contains(sep)) {
        return title;
    }
    let doc = Document::from(html);
    let page_headings = doc.select("h1, h2").iter().map(|h| h.text().to_string()).collect::<Vec<_>>();
    let headings: Vec<String> = blocks
        .iter()
        .filter(|b| b.kind == BlockKind::Heading)
        .map(|b| b.text.clone())
        .chain(page_headings)
        .filter_map(|t| clean_title(&t))
        .map(|t| t.to_lowercase())
        .collect();
    TITLE_SEPARATORS
        .iter()
        .flat_map(|sep| title.split(sep))
        .filter_map(clean_title)
        .find(|part| part != &title && headings.contains(&part.to_lowercase()))
        .unwrap_or(title)
}

fn has_text(blocks: &[Block]) -> bool {
    blocks.iter().any(|b| !b.text.trim().is_empty())
}

/// Blocks of an HTML document; with `body_only` the `<body>` is walked, otherwise the whole document.
fn blocks_from_html(html: &str, body_only: bool) -> Vec<Block> {
    let doc = Document::from(html);
    let start = if body_only { doc.body().unwrap_or_else(|| doc.root()) } else { doc.root() };
    blocks_from_tree(&tree_of(&start))
}

/// The subtree below a dom_query node as a [`crate::dom`] tree rooted at a synthetic element.
pub(crate) fn tree_of(node: &NodeRef<'_>) -> Element {
    let mut root = Element::new("#root");
    convert_children(node, &mut root, 0);
    root
}

/// Copies the children of a dom_query node into `into`. Below [`MAX_TREE_DEPTH`] elements are flattened to their text.
fn convert_children(node: &NodeRef<'_>, into: &mut Element, depth: usize) {
    for child in node.children_it(false) {
        if child.is_text() {
            into.children.push(Node::Text(child.text().to_string()));
        } else if child.is_element() {
            if depth >= MAX_TREE_DEPTH {
                into.children.push(Node::Text(child.text().to_string()));
                continue;
            }
            let name = child.node_name().map(|n| n.to_ascii_lowercase()).unwrap_or_default();
            let mut el = Element::new(name);
            el.attrs = child
                .attrs()
                .iter()
                .map(|a| (a.name.local.to_ascii_lowercase().to_string(), a.value.to_string()))
                .collect();
            convert_children(&child, &mut el, depth + 1);
            into.children.push(Node::Element(el));
        }
    }
}

/// The page's `<title>`, used when readability gave no title.
fn document_title(html: &str) -> Option<String> {
    let doc = Document::from(html);
    let title = doc.select("title").text();
    clean_title(&title)
}

/// `example.com` from `https://www.example.com/a/b`; the last-resort title of a fetched page.
pub(crate) fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = host.split(':').next()?;
    let host = host.strip_prefix("www.").unwrap_or(host);
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARTICLE: &str = include_str!("../tests/fixtures/article.html");

    fn kinds_and_text(blocks: &[Block]) -> Vec<(BlockKind, String)> {
        blocks.iter().map(|b| (b.kind, b.text.split_whitespace().collect::<Vec<_>>().join(" "))).collect()
    }

    #[test]
    fn article_fixture_extracts_the_story_only() {
        let ex = extract(ARTICLE, Some("https://www.example.org/stories/lighthouse")).unwrap_or_default();
        assert_eq!(ex.title.as_deref(), Some("The Last Lighthouse Keeper"));
        assert_eq!(ex.author.as_deref().and_then(clean_author).as_deref(), Some("Mara Ellison"));
        let blocks = kinds_and_text(&ex.blocks);
        let all = blocks.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n");
        // Navigation, comments, scripts and the newsletter box are gone.
        for junk in ["Subscribe", "Home", "Comments", "trackPageView", "Related stories", "Cookie", "By Mara"] {
            assert!(!all.contains(junk), "{junk} leaked into: {all}");
        }
        assert!(blocks.contains(&(BlockKind::Heading, "The night shift".into())), "{blocks:?}");
        assert!(
            blocks.iter().any(|(k, t)| *k == BlockKind::Quote && t.starts_with("The sea does not care")),
            "{blocks:?}"
        );
        assert_eq!(blocks.iter().filter(|(k, _)| *k == BlockKind::ListItem).count(), 3, "{blocks:?}");
        assert!(blocks.iter().any(|(_, t)| t == crate::common::CODE_SKIPPED), "{blocks:?}");
        // The footnote marker is dropped, the sentence around it is kept.
        assert!(all.contains("kept a log every night."), "{all}");
        assert!(!all.contains("logbooks are now held"), "footnote text leaked: {all}");
        let first_para = blocks.iter().find(|(k, _)| *k == BlockKind::Paragraph).map(|(_, t)| t.as_str());
        assert!(first_para.is_some_and(|t| t.starts_with("For forty-one years")), "{blocks:?}");
    }

    #[test]
    fn short_page_falls_back_to_body() {
        let html = "<html><head><title>Note</title></head><body><p>Just one line.</p></body></html>";
        let ex = extract(html, None).unwrap_or_default();
        assert_eq!(ex.title.as_deref(), Some("Note"));
        assert_eq!(kinds_and_text(&ex.blocks), vec![(BlockKind::Paragraph, "Just one line.".into())]);
    }

    #[test]
    fn relative_url_is_ignored() {
        let html = "<p>Hello there.</p>";
        let ex = extract(html, Some("not a url")).unwrap_or_default();
        assert_eq!(kinds_and_text(&ex.blocks), vec![(BlockKind::Paragraph, "Hello there.".into())]);
        assert_eq!(ex.title, None);
    }

    #[test]
    fn empty_page_has_no_blocks() {
        let ex = extract("<html><body><script>x()</script></body></html>", None).unwrap_or_default();
        assert!(!has_text(&ex.blocks));
    }

    #[test]
    fn deep_nesting_is_flattened_not_lost() {
        let depth = MAX_TREE_DEPTH * 3;
        let html = format!("{}<p>Deep words.</p>{}", "<div>".repeat(depth), "</div>".repeat(depth));
        let ex = extract(&html, None).unwrap_or_default();
        let text: String = ex.blocks.iter().map(|b| b.text.as_str()).collect();
        assert!(text.contains("Deep words."), "{:?}", ex.blocks);
    }

    #[test]
    fn site_name_is_removed_only_when_a_heading_confirms_it() {
        let heading = |t: &str| Block { kind: BlockKind::Heading, text: t.into() };
        assert_eq!(without_site_name("Story | Site".into(), "", &[heading("Story")]), "Story");
        assert_eq!(without_site_name("Site — Story Name".into(), "", &[heading(" story  name ")]), "Story Name");
        assert_eq!(without_site_name("Story | Site".into(), "<h1>Other</h1>", &[heading("Other")]), "Story | Site");
        assert_eq!(without_site_name("Story | Site".into(), "<h1> Story </h1>", &[]), "Story");
        assert_eq!(without_site_name("Plain".into(), "", &[heading("Plain")]), "Plain");
    }

    #[test]
    fn byline_paragraph_is_dropped_once() {
        let p = |t: &str| Block { kind: BlockKind::Paragraph, text: t.into() };
        let mut blocks = vec![p("By  Jane Doe"), p("Body."), p("Jane Doe")];
        drop_byline_block(&mut blocks, "Jane Doe");
        assert_eq!(blocks, vec![p("Body."), p("Jane Doe")]);
    }

    #[test]
    fn host_of_urls() {
        assert_eq!(host_of("https://www.Example.com/a?b"), Some("example.com".into()));
        assert_eq!(host_of("http://user:pw@news.site:8080/x"), Some("news.site".into()));
        assert_eq!(host_of("https://"), None);
    }
}
