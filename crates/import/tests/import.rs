//! End-to-end tests of the public API: every importer's full output after `nolabs_core::text::normalise_blocks`.

use std::io::{Cursor, Write};

use nolabs_core::proto::api::{Block, BlockKind};
use nolabs_import::{ImportError, PdfLine, from_epub, from_html, from_markdown, from_pdf_pages, from_text};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn b(kind: BlockKind, text: &str) -> Block {
    Block { kind, text: text.into() }
}

use BlockKind::{Heading, ListItem, Paragraph, Quote};

#[test]
fn text_full_output() -> TestResult {
    let doc = from_text(
        "A Short Story\n\nIt was an exam-\nple of   wrapped text.[12]\n\nThe end, see https://www.example.com/x.",
        None,
    )?;
    assert_eq!(doc.title, "A Short Story");
    assert_eq!(doc.author, None);
    assert_eq!(
        doc.blocks,
        vec![
            b(Heading, "A Short Story"),
            b(Paragraph, "It was an example of wrapped text."),
            b(Paragraph, "The end, see example.com."),
        ]
    );
    Ok(())
}

#[test]
fn text_without_short_first_line_derives_a_title() -> TestResult {
    let long = "This first paragraph is a single long line that goes on well past the length of any sensible title.";
    let doc = from_text(long, None)?;
    assert!(doc.title.ends_with('…'), "{}", doc.title);
    assert!(long.starts_with(doc.title.trim_end_matches('…')));
    Ok(())
}

#[test]
fn empty_inputs_are_empty_errors() {
    assert_eq!(from_text(" \n\n\t", None), Err(ImportError::Empty));
    assert_eq!(from_markdown("```\ncode only\n```\n", None), Err(ImportError::Empty));
    assert_eq!(from_html("<html><body><script>x()</script></body></html>", None), Err(ImportError::Empty));
    assert_eq!(from_pdf_pages(&[vec![], vec![]], None), Err(ImportError::Empty));
}

#[test]
fn markdown_full_output() -> TestResult {
    let md = "---\ntitle: Notes on Tides\nauthor: R. Moon\n---\n\n# Tides\n\nThe *moon* pulls the sea[^1].\n\n> \"Time and tide...\"\n\n- High water\n- Low water\n\n```\nprint(1)\n```\n\n[^1]: Mostly.\n";
    let doc = from_markdown(md, Some("tides.md"))?;
    assert_eq!(doc.title, "Notes on Tides");
    assert_eq!(doc.author.as_deref(), Some("R. Moon"));
    assert_eq!(
        doc.blocks,
        vec![
            b(Heading, "Tides"),
            b(Paragraph, "The moon pulls the sea."),
            b(Quote, "“Time and tide…”"),
            b(ListItem, "High water"),
            b(ListItem, "Low water"),
            b(Paragraph, "Code block skipped."),
        ]
    );
    Ok(())
}

#[test]
fn html_article_full_output() -> TestResult {
    let html = include_str!("fixtures/article.html");
    let doc = from_html(html, Some("https://www.example.org/stories/lighthouse"))?;
    assert_eq!(doc.title, "The Last Lighthouse Keeper");
    assert_eq!(doc.author.as_deref(), Some("Mara Ellison"));
    let kinds: Vec<BlockKind> = doc.blocks.iter().map(|b| b.kind).collect();
    assert_eq!(
        kinds,
        vec![
            Paragraph, Paragraph, Heading, Paragraph, Quote, Paragraph, ListItem, ListItem, ListItem, Paragraph,
            Paragraph, Paragraph,
        ]
    );
    assert_eq!(
        doc.blocks[0].text,
        "For forty-one years, Tomas Reyes climbed the one hundred and twelve steps of the Pointe Grise lighthouse every \
         evening at dusk. He trimmed the wick, wound the clockwork that turned the great lens, and kept a log every \
         night. When the light was automated last spring, he was the last keeper on this stretch of coast to hand over \
         the keys."
    );
    assert_eq!(doc.blocks[2], b(Heading, "The night shift"));
    assert_eq!(doc.blocks[10], b(Paragraph, "Code block skipped."));
    // Normalised: no newlines or double spaces survive anywhere.
    assert!(doc.blocks.iter().all(|b| !b.text.contains('\n') && !b.text.contains("  ")));
    Ok(())
}

#[test]
fn html_fragment_uses_url_host_as_title() -> TestResult {
    let doc = from_html("<p>Only a fragment of text.</p>", Some("https://news.example.net/a/b"))?;
    assert_eq!(doc.title, "news.example.net");
    assert_eq!(doc.blocks, vec![b(Paragraph, "Only a fragment of text.")]);
    Ok(())
}

/// A minimal valid EPUB 3 built in memory.
fn tiny_epub() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let files: [(&str, &str); 5] = [
        ("mimetype", "application/epub+zip"),
        (
            "META-INF/container.xml",
            r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="EPUB/package.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
        ),
        (
            "EPUB/package.opf",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">urn:uuid:0</dc:identifier><dc:title>Harbour Tales</dc:title>
    <dc:creator>Iris Quay</dc:creator><dc:creator>Tom Pier</dc:creator><dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="one" href="one.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="nav"/><itemref idref="one"/></spine>
</package>"#,
        ),
        (
            "EPUB/nav.xhtml",
            r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li>One</li></ol></nav></body></html>"#,
        ),
        (
            "EPUB/one.xhtml",
            r##"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>One</title></head>
<body>
  <h1>The Ferry</h1>
  <p>The ferry left at dawn,<a epub:type="noteref" href="#fn1">1</a> as it had for
     fifty years.</p>
  <blockquote><p>“Mind the gap,” said the mate.</p></blockquote>
  <aside epub:type="footnote" id="fn1"><p>Weather permitting.</p></aside>
</body></html>"##,
        ),
    ];
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in files {
        let method = if name == "mimetype" { CompressionMethod::Stored } else { CompressionMethod::Deflated };
        w.start_file(name, SimpleFileOptions::default().compression_method(method))?;
        w.write_all(data.as_bytes())?;
    }
    Ok(w.finish()?.into_inner())
}

#[test]
fn epub_full_output() -> TestResult {
    let doc = from_epub(&tiny_epub()?)?;
    assert_eq!(doc.title, "Harbour Tales");
    assert_eq!(doc.author.as_deref(), Some("Iris Quay, Tom Pier"));
    assert_eq!(
        doc.blocks,
        vec![
            b(Heading, "The Ferry"),
            b(Paragraph, "The ferry left at dawn, as it had for fifty years."),
            b(Quote, "“Mind the gap,” said the mate."),
        ]
    );
    Ok(())
}

#[test]
fn epub_garbage_is_malformed() {
    assert!(matches!(from_epub(b"definitely not a zip"), Err(ImportError::Malformed(_))));
}

#[test]
fn pdf_full_output() -> TestResult {
    let l = |text: &str, y: f32, height: f32| PdfLine { text: text.into(), y, height };
    let header = |n: usize| l(&format!("Coastal Notes   {n}"), 800.0, 8.0);
    let pages = vec![
        vec![
            header(1),
            l("Fog Signals", 750.0, 18.0),
            l("Before radio, fog signals warned ships away from the", 700.0, 10.0),
            l("rocks. Horns, bells and guns were all tried, each with", 686.0, 10.0),
            l("its own draw-", 672.0, 10.0),
        ],
        vec![
            header(2),
            l("backs in wind and rain.", 760.0, 10.0),
            l("The diaphone, a compressed-air horn, became the", 732.0, 10.0),
            l("standard sound of the coast for most of a century.", 718.0, 10.0),
        ],
        vec![header(3), l("Later chapters cover lights and buoys in detail.", 760.0, 10.0), l("3", 40.0, 8.0)],
    ];
    let doc = from_pdf_pages(&pages, None)?;
    assert_eq!(doc.title, "Fog Signals");
    assert_eq!(
        doc.blocks,
        vec![
            b(Heading, "Fog Signals"),
            b(
                Paragraph,
                "Before radio, fog signals warned ships away from the rocks. Horns, bells and guns were all tried, each \
                 with its own drawbacks in wind and rain."
            ),
            b(
                Paragraph,
                "The diaphone, a compressed-air horn, became the standard sound of the coast for most of a century. \
                 Later chapters cover lights and buoys in detail."
            ),
        ]
    );
    Ok(())
}
