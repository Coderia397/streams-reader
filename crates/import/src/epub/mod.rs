//! EPUB 2 and 3: `container.xml` → package document (OPF) → metadata and spine → XHTML documents → blocks.

mod archive;
mod package;
mod xhtml;
mod xml;

use nolabs_core::proto::api::Block;

use self::archive::Archive;
use crate::common::finish;
use crate::limits::MAX_EPUB_BYTES;
use crate::tree_blocks::blocks_from_tree;
use crate::{ImportError, Imported};

const CONTAINER: &str = "META-INF/container.xml";
const ENCRYPTION: &str = "META-INF/encryption.xml";

#[derive(Debug, Default)]
pub(crate) struct Extracted {
    pub title: Option<String>,
    pub author: Option<String>,
    pub blocks: Vec<Block>,
}

pub(crate) fn import(bytes: &[u8]) -> Result<Imported, ImportError> {
    let ex = extract(bytes)?;
    finish(ex.title, ex.author, ex.blocks)
}

pub(crate) fn extract(bytes: &[u8]) -> Result<Extracted, ImportError> {
    if bytes.len() > MAX_EPUB_BYTES {
        return Err(ImportError::TooLarge("the book is larger than 200 MB"));
    }
    let mut archive = Archive::open(bytes)?;

    if let Some(enc) = archive.read(ENCRYPTION)?
        && package::has_drm(&xml::decode(&enc))
    {
        return Err(ImportError::Unsupported("this book is DRM-protected".into()));
    }

    let opf_path = package_path(&mut archive)?;
    let opf = archive
        .read(&opf_path)?
        .ok_or_else(|| ImportError::Malformed(format!("the package document {opf_path} is missing")))?;
    let pkg = package::parse_opf(&xml::decode(&opf));
    if pkg.spine.is_empty() {
        return Err(ImportError::Malformed("the book has no reading order (empty spine)".into()));
    }

    let base = opf_path.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut blocks = Vec::new();
    for item in pkg.reading_order() {
        let path = resolve_href(base, &item.href);
        // A spine entry pointing at a missing file is skipped; the rest of the book is still worth reading.
        if let Some(doc) = archive.read(&path)? {
            blocks.extend(blocks_from_tree(&xhtml::parse(&xml::decode(&doc))));
        }
    }

    let author = (!pkg.authors.is_empty()).then(|| pkg.authors.join(", "));
    Ok(Extracted { title: pkg.title, author, blocks })
}

/// The package document path from `container.xml`, else the first `.opf` file in the archive.
fn package_path(archive: &mut Archive<'_>) -> Result<String, ImportError> {
    if let Some(container) = archive.read(CONTAINER)?
        && let Some(path) = package::rootfile(&xml::decode(&container))
        && archive.contains(&path)
    {
        return Ok(path);
    }
    archive
        .names()
        .into_iter()
        .find(|n| n.to_ascii_lowercase().ends_with(".opf"))
        .ok_or_else(|| ImportError::Malformed("not an EPUB: no package document".into()))
}

/// Zip path of a manifest `href` relative to the package directory: percent-decoded, without fragment or query,
/// with `.` and `..` segments resolved (never above the archive root).
fn resolve_href(base: &str, href: &str) -> String {
    let href = href.split(['#', '?']).next().unwrap_or_default();
    let href = percent_decode(href);
    let mut parts: Vec<&str> = if href.starts_with('/') { Vec::new() } else { base.split('/').collect() };
    for seg in href.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.retain(|p| !p.is_empty());
    parts.join("/")
}

/// Decodes `%XX` escapes; invalid escapes are kept as written.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = s.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::BlockKind;

    use super::archive::tests::zip_of;
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    const CONTAINER_XML: &[u8] = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;

    const OPF: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">urn:uuid:test</dc:identifier>
    <dc:title>The Small Book</dc:title>
    <dc:creator id="a1">Ada Writer</dc:creator>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="Text/chapter%201.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="Text/../Text/chapter2.xhtml#start" media-type="application/xhtml+xml"/>
    <item id="notes" href="Text/notes.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="nav"/>
    <itemref idref="c2"/>
    <itemref idref="c1"/>
    <itemref idref="notes" linear="no"/>
  </spine>
</package>"#;

    const NAV: &[u8] = br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body>
<nav epub:type="toc"><ol><li><a href="Text/chapter2.xhtml">Chapter Two</a></li></ol></nav></body></html>"#;

    const CH1: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>One</title><link rel="stylesheet" href="../style.css"/></head>
<body><h1>Chapter One</h1>
<p>The first chapter&#8217;s opening line.<a epub:type="noteref" href="notes.xhtml#n1">1</a></p>
<blockquote><p>A quotation.</p></blockquote>
<ul><li>Item one</li><li>Item two</li></ul>
</body></html>"#;

    const CH2: &[u8] = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body>
<h2>Chapter Two</h2><p>Second chapter text.</p></body></html>"#;

    const NOTES: &[u8] = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p id="n1">A note.</p></body></html>"#;

    fn book(extra: &[(&str, &[u8])]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut entries: Vec<(&str, &[u8])> = vec![
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", CONTAINER_XML),
            ("OEBPS/content.opf", OPF),
            ("OEBPS/nav.xhtml", NAV),
            ("OEBPS/Text/chapter 1.xhtml", CH1),
            ("OEBPS/Text/chapter2.xhtml", CH2),
            ("OEBPS/Text/notes.xhtml", NOTES),
        ];
        entries.extend_from_slice(extra);
        zip_of(&entries)
    }

    fn summary(blocks: &[Block]) -> Vec<(BlockKind, String)> {
        blocks.iter().map(|b| (b.kind, b.text.split_whitespace().collect::<Vec<_>>().join(" "))).collect()
    }

    #[test]
    fn reads_spine_in_order_with_metadata() -> TestResult {
        let ex = extract(&book(&[])?)?;
        assert_eq!(ex.title.as_deref(), Some("The Small Book"));
        assert_eq!(ex.author.as_deref(), Some("Ada Writer"));
        assert_eq!(
            summary(&ex.blocks),
            vec![
                (BlockKind::Heading, "Chapter Two".into()),
                (BlockKind::Paragraph, "Second chapter text.".into()),
                (BlockKind::Heading, "Chapter One".into()),
                (BlockKind::Paragraph, "The first chapter’s opening line.".into()),
                (BlockKind::Quote, "A quotation.".into()),
                (BlockKind::ListItem, "Item one".into()),
                (BlockKind::ListItem, "Item two".into()),
            ]
        );
        Ok(())
    }

    #[test]
    fn falls_back_to_any_opf_without_container() -> TestResult {
        let bytes = zip_of(&[("OEBPS/content.opf", OPF), ("OEBPS/Text/chapter2.xhtml", CH2)])?;
        let ex = extract(&bytes)?;
        assert_eq!(summary(&ex.blocks).len(), 2);
        Ok(())
    }

    #[test]
    fn drm_is_unsupported() -> TestResult {
        let enc: &[u8] = br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container"
            xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData>
            <enc:EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#aes128-cbc"/>
            <enc:CipherData><enc:CipherReference URI="OEBPS/Text/chapter2.xhtml"/></enc:CipherData>
            </enc:EncryptedData></encryption>"#;
        assert!(matches!(extract(&book(&[("META-INF/encryption.xml", enc)])?), Err(ImportError::Unsupported(_))));
        Ok(())
    }

    #[test]
    fn not_a_zip_is_malformed() {
        assert!(matches!(extract(b"PK\x03\x04 garbage"), Err(ImportError::Malformed(_))));
    }

    #[test]
    fn zip_without_package_is_malformed() -> TestResult {
        let bytes = zip_of(&[("hello.txt", b"hi")])?;
        assert!(matches!(extract(&bytes), Err(ImportError::Malformed(_))));
        Ok(())
    }

    #[test]
    fn zip_bomb_chapter_is_refused() -> TestResult {
        let opf: &[u8] = br#"<package><metadata/><manifest><item id="b" href="b.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="b"/></spine></package>"#;
        let zeros = vec![b' '; 16 * 1024 * 1024];
        let bytes = zip_of(&[("content.opf", opf), ("b.xhtml", &zeros)])?;
        assert!(matches!(extract(&bytes), Err(ImportError::TooLarge(_))));
        Ok(())
    }

    #[test]
    fn resolves_hrefs() {
        assert_eq!(resolve_href("OEBPS", "Text/a%20b.xhtml#frag"), "OEBPS/Text/a b.xhtml");
        assert_eq!(resolve_href("OEBPS/sub", "../x.xhtml"), "OEBPS/x.xhtml");
        assert_eq!(resolve_href("", "./x.xhtml"), "x.xhtml");
        assert_eq!(resolve_href("a", "../../../x.xhtml"), "x.xhtml");
        assert_eq!(resolve_href("a", "/root.xhtml"), "root.xhtml");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%E2%80%94%zz"), "—%zz");
    }
}
