//! EPUB packaging: `META-INF/container.xml` names the package document (OPF); the OPF gives metadata, the manifest
//! (id → file) and the spine (reading order). `META-INF/encryption.xml` reveals DRM.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;

use super::xml::{attributes, local_name, resolve_ref};

/// Algorithms used only to obfuscate embedded fonts (IDPF and Adobe); books using them are not DRM-protected.
const FONT_OBFUSCATION: &[&str] = &["http://www.idpf.org/2008/embedding", "http://ns.adobe.com/pdf/enc#RC"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestItem {
    pub href: String,
    pub media_type: String,
    pub properties: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpineItem {
    pub idref: String,
    /// `linear="no"` marks auxiliary content (pop-up notes, answer keys) that is not part of the reading order.
    pub linear: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Package {
    pub title: Option<String>,
    /// Creators whose role is author (or unspecified), in document order.
    pub authors: Vec<String>,
    pub manifest: HashMap<String, ManifestItem>,
    pub spine: Vec<SpineItem>,
}

impl Package {
    /// Manifest items of the spine that hold readable XHTML, in reading order. Navigation documents and non-linear
    /// items are left out.
    pub(crate) fn reading_order(&self) -> Vec<&ManifestItem> {
        self.spine
            .iter()
            .filter(|s| s.linear)
            .filter_map(|s| self.manifest.get(&s.idref))
            .filter(|item| {
                let mt = item.media_type.to_ascii_lowercase();
                (mt == "application/xhtml+xml" || mt == "text/html" || mt.is_empty())
                    && !item.properties.split_whitespace().any(|p| p == "nav")
            })
            .collect()
    }
}

/// Path of the first package document named in `container.xml`.
pub(crate) fn rootfile(container_xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(container_xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) if local_name(&e) == "rootfile" => {
                let attrs = attributes(&e);
                let media_type = attr(&attrs, "media-type").unwrap_or("application/oebps-package+xml");
                if media_type == "application/oebps-package+xml"
                    && let Some(path) = attr(&attrs, "full-path").filter(|p| !p.is_empty())
                {
                    return Some(path.to_owned());
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}

/// A creator before its role is known (EPUB 3 gives roles in separate `<meta refines="#id">` elements).
struct Creator {
    id: Option<String>,
    role: Option<String>,
    name: String,
}

/// Which metadata text is being collected.
enum Capture {
    None,
    Title,
    Creator,
    /// `<meta refines="#id" property="role">`: the role of creator `id`.
    Role(String),
}

/// Parses the package document. Malformed XML after the useful parts is tolerated: whatever was read is returned.
pub(crate) fn parse_opf(opf: &str) -> Package {
    let mut reader = Reader::from_str(opf);
    reader.config_mut().check_end_names = false;
    let mut pkg = Package::default();
    let mut creators: Vec<Creator> = Vec::new();
    let mut refined_roles: HashMap<String, String> = HashMap::new();
    let mut capture = Capture::None;
    let mut text = String::new();
    let mut in_metadata = false;

    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(ev) => ev,
        };
        match event {
            Event::Start(e) => {
                let name = local_name(&e);
                let attrs = attributes(&e);
                let started = match name.as_str() {
                    "metadata" => {
                        in_metadata = true;
                        None
                    }
                    "title" if in_metadata && pkg.title.is_none() => Some(Capture::Title),
                    "creator" if in_metadata => {
                        let role = attr(&attrs, "opf:role").or_else(|| attr(&attrs, "role")).map(str::to_owned);
                        creators.push(Creator { id: attr(&attrs, "id").map(str::to_owned), role, name: String::new() });
                        Some(Capture::Creator)
                    }
                    "meta" if in_metadata && attr(&attrs, "property") == Some("role") => {
                        attr(&attrs, "refines").map(|r| Capture::Role(r.trim_start_matches('#').to_owned()))
                    }
                    _ => {
                        manifest_or_spine(&name, &attrs, &mut pkg);
                        None
                    }
                };
                if let Some(c) = started {
                    capture = c;
                    text.clear();
                }
            }
            Event::Empty(e) => manifest_or_spine(&local_name(&e), &attributes(&e), &mut pkg),
            Event::Text(t) if capturing(&capture) => text.push_str(&t.xml10_content()),
            Event::CData(t) if capturing(&capture) => text.push_str(&t.xml10_content()),
            Event::GeneralRef(r) if capturing(&capture) => text.extend(resolve_ref(&r)),
            Event::End(e) => {
                let name = e.local_name().as_ref().to_ascii_lowercase();
                let closes = matches!(
                    (&capture, name.as_str()),
                    (Capture::Title, "title")
                        | (Capture::Creator, "creator")
                        | (Capture::Role(_), "meta")
                        | (_, "metadata")
                );
                if name == "metadata" {
                    in_metadata = false;
                }
                // Other end tags inside a captured element (`<dc:title><span>…</span></dc:title>`) keep capturing.
                if closes {
                    let value = text.trim().to_owned();
                    match std::mem::replace(&mut capture, Capture::None) {
                        Capture::Title => pkg.title = Some(value).filter(|t| !t.is_empty()),
                        Capture::Creator => {
                            if let Some(c) = creators.last_mut() {
                                c.name = value;
                            }
                        }
                        Capture::Role(id) => {
                            refined_roles.insert(id, value);
                        }
                        Capture::None => {}
                    }
                }
            }
            _ => {}
        }
    }

    pkg.authors = creators
        .into_iter()
        .filter(|c| !c.name.is_empty())
        .filter(|c| {
            let role = c.role.clone().or_else(|| c.id.as_ref().and_then(|id| refined_roles.get(id).cloned()));
            role.is_none_or(|r| r.trim().eq_ignore_ascii_case("aut"))
        })
        .map(|c| c.name)
        .collect();
    pkg
}

fn capturing(capture: &Capture) -> bool {
    !matches!(capture, Capture::None)
}

fn manifest_or_spine(name: &str, attrs: &[(String, String)], pkg: &mut Package) {
    match name {
        "item" => {
            if let (Some(id), Some(href)) = (attr(attrs, "id"), attr(attrs, "href")) {
                pkg.manifest.insert(
                    id.to_owned(),
                    ManifestItem {
                        href: href.to_owned(),
                        media_type: attr(attrs, "media-type").unwrap_or_default().to_owned(),
                        properties: attr(attrs, "properties").unwrap_or_default().to_owned(),
                    },
                );
            }
        }
        "itemref" => {
            if let Some(idref) = attr(attrs, "idref") {
                let linear = !attr(attrs, "linear").is_some_and(|l| l.trim().eq_ignore_ascii_case("no"));
                pkg.spine.push(SpineItem { idref: idref.to_owned(), linear });
            }
        }
        _ => {}
    }
}

/// True when `encryption.xml` encrypts anything other than fonts, i.e. the book is DRM-protected.
pub(crate) fn has_drm(encryption_xml: &str) -> bool {
    let mut reader = Reader::from_str(encryption_xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) if local_name(&e) == "encryptionmethod" => {
                let attrs = attributes(&e);
                if !attr(&attrs, "algorithm").is_some_and(|a| FONT_OBFUSCATION.contains(&a.trim())) {
                    return true;
                }
            }
            Ok(Event::Eof) | Err(_) => return false,
            Ok(_) => {}
        }
    }
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPF3: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="id">urn:uuid:1</dc:identifier>
    <dc:title>A Tale &amp; More</dc:title>
    <dc:title>Second Title Ignored</dc:title>
    <dc:creator id="c1">Jane Author</dc:creator>
    <meta refines="#c1" property="role" scheme="marc:relators">aut</meta>
    <dc:creator id="c2">Ed Itor</dc:creator>
    <meta refines="#c2" property="role" scheme="marc:relators">edt</meta>
    <dc:creator>Co Author</dc:creator>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="ch1" href="text/ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch2" href="text/ch2.xhtml" media-type="application/xhtml+xml"></item>
    <item id="notes" href="text/notes.xhtml" media-type="application/xhtml+xml"/>
    <item id="css" href="style.css" media-type="text/css"/>
  </manifest>
  <spine>
    <itemref idref="nav"/>
    <itemref idref="ch1"/>
    <itemref idref="css"/>
    <itemref idref="ch2"/>
    <itemref idref="notes" linear="no"/>
    <itemref idref="missing"/>
  </spine>
</package>"##;

    #[test]
    fn parses_epub3_package() {
        let pkg = parse_opf(OPF3);
        assert_eq!(pkg.title.as_deref(), Some("A Tale & More"));
        assert_eq!(pkg.authors, vec!["Jane Author".to_owned(), "Co Author".to_owned()]);
        assert_eq!(pkg.spine.len(), 6);
        let order: Vec<&str> = pkg.reading_order().iter().map(|i| i.href.as_str()).collect();
        assert_eq!(order, vec!["text/ch1.xhtml", "text/ch2.xhtml"]);
    }

    #[test]
    fn parses_epub2_roles() {
        let opf = r#"<package><metadata><dc:title>T</dc:title>
            <dc:creator opf:role="ill">Illus Trator</dc:creator><dc:creator opf:role="aut">Real Author</dc:creator>
            </metadata><manifest/><spine/></package>"#;
        let pkg = parse_opf(opf);
        assert_eq!(pkg.authors, vec!["Real Author".to_owned()]);
    }

    #[test]
    fn truncated_opf_keeps_what_was_read() {
        let pkg = parse_opf(&OPF3[..OPF3.find("<spine>").unwrap_or(OPF3.len())]);
        assert_eq!(pkg.title.as_deref(), Some("A Tale & More"));
        assert!(pkg.spine.is_empty());
        assert_eq!(pkg.manifest.len(), 5);
    }

    #[test]
    fn finds_rootfile() {
        let xml = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
            <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
        assert_eq!(rootfile(xml).as_deref(), Some("OEBPS/content.opf"));
        assert_eq!(rootfile("<container/>"), None);
        assert_eq!(rootfile("not xml <<<"), None);
    }

    #[test]
    fn detects_drm_but_not_font_obfuscation() {
        let fonts = r#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#">
            <enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/>
            <enc:CipherData><enc:CipherReference URI="fonts/a.otf"/></enc:CipherData></enc:EncryptedData></encryption>"#;
        assert!(!has_drm(fonts));
        let drm = fonts.replace("http://www.idpf.org/2008/embedding", "http://www.w3.org/2001/04/xmlenc#aes128-cbc");
        assert!(has_drm(&drm));
    }
}
