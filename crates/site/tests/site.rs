//! End-to-end checks on a real build: translations are complete, every page links its language alternates both
//! ways, nothing needs inline script or style (strict CSP), and every internal link resolves to a built file.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)] // test helpers fail loudly by design

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use nolabs_site::config::{DEFAULT_LANG, LANGS, ORIGIN, Page};
use nolabs_site::i18n::Catalog;
use nolabs_site::seo::Date;
use nolabs_site::{Paths, build};

/// Messages only the English file needs (llms.txt is English only).
const ENGLISH_ONLY_PREFIX: &str = "llms-";

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Build once into a fresh temporary folder shared by the tests.
fn built() -> &'static Path {
    static OUT: OnceLock<PathBuf> = OnceLock::new();
    OUT.get_or_init(|| {
        let out = std::env::temp_dir().join(format!("site-gen-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let paths = Paths { crate_dir: crate_dir(), repo_root: crate_dir().join("../.."), out: out.clone() };
        build(&paths, Date { year: 2026, month: 10, day: 30 }).expect("site builds");
        out
    })
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(built().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn placeables(src: &str, key: &str) -> BTreeSet<String> {
    let line = src.lines().find(|l| l.starts_with(&format!("{key} ="))).unwrap_or_default();
    line.split("{ $").skip(1).filter_map(|p| p.split(' ').next()).map(str::to_string).collect()
}

#[test]
fn every_language_has_every_message_with_the_same_placeables() {
    let locales = crate_dir().join("locales");
    let en_src = std::fs::read_to_string(locales.join("en/site.ftl")).unwrap();
    let en = Catalog::load(&locales, DEFAULT_LANG).unwrap();
    let wanted: BTreeSet<&str> =
        en.keys().iter().map(String::as_str).filter(|k| !k.starts_with(ENGLISH_ONLY_PREFIX)).collect();
    for lang in LANGS.iter().filter(|l| !l.is_default()) {
        let src = std::fs::read_to_string(locales.join(lang.code).join("site.ftl")).unwrap();
        let cat = Catalog::load(&locales, *lang).unwrap();
        let have: BTreeSet<&str> = cat.keys().iter().map(String::as_str).collect();
        let missing: Vec<_> = wanted.difference(&have).collect();
        let extra: Vec<_> = have.difference(&wanted).collect();
        assert!(missing.is_empty(), "{}: missing {missing:?}", lang.code);
        assert!(extra.is_empty(), "{}: unknown {extra:?}", lang.code);
        for key in &wanted {
            assert_eq!(placeables(&src, key), placeables(&en_src, key), "{}: placeables of {key}", lang.code);
        }
    }
}

#[test]
fn every_page_exists_in_every_language_with_lang_and_direction() {
    for lang in LANGS {
        for page in Page::ALL {
            let html = read(&page.out_file(&lang));
            let dir = if lang.code == "ar" { "rtl" } else { "ltr" };
            assert!(html.contains(&format!("<html lang=\"{}\" dir=\"{dir}\">", lang.code)), "{}", page.out_file(&lang));
            assert!(html.contains("<title>") && html.contains("<meta name=\"description\""));
            assert!(html.contains("<h1"), "{} has a heading", page.out_file(&lang));
        }
    }
}

#[test]
fn hreflang_alternates_are_complete_and_mutual() {
    for page in Page::ALL.into_iter().filter(|p| p.indexable()) {
        for lang in LANGS {
            let html = read(&page.out_file(&lang));
            assert!(html.contains(&format!("<link rel=\"canonical\" href=\"{}\">", page.url(&lang))));
            for alt in LANGS {
                let link = format!("<link rel=\"alternate\" hreflang=\"{}\" href=\"{}\">", alt.code, page.url(&alt));
                assert!(html.contains(&link), "{} lacks {link}", page.out_file(&lang));
            }
            assert!(html.contains(&format!("hreflang=\"x-default\" href=\"{}\"", page.url(&DEFAULT_LANG))));
        }
    }
    let not_found = read("404.html");
    assert!(not_found.contains("<meta name=\"robots\" content=\"noindex\">"));
    assert!(!not_found.contains("rel=\"canonical\""));
}

#[test]
fn pages_need_no_inline_script_handlers_or_styles() {
    for lang in LANGS {
        for page in Page::ALL {
            let html = read(&page.out_file(&lang));
            for tag in html.split("<script").skip(1) {
                let head = &tag[..tag.find('>').unwrap()];
                assert!(
                    head.contains(" src=\"/assets/") || head.contains("type=\"application/ld+json\""),
                    "inline script"
                );
            }
            assert!(!html.contains(" style=\""), "style attribute in {}", page.out_file(&lang));
            for attr in [" onclick=", " onload=", " onerror=", " onchange=", "javascript:"] {
                assert!(!html.contains(attr), "{attr} in {}", page.out_file(&lang));
            }
        }
    }
}

#[test]
fn structured_data_is_valid_json() {
    let html = read("index.html");
    let mut found = 0;
    for part in html.split("<script type=\"application/ld+json\">").skip(1) {
        let json = &part[..part.find("</script>").unwrap()];
        let v: serde_json::Value = serde_json::from_str(json).expect("valid JSON-LD");
        assert!(v["@type"].is_string());
        found += 1;
    }
    assert_eq!(found, 2, "SoftwareApplication and FAQPage");
}

#[test]
fn internal_links_resolve_to_built_files() {
    let out = built();
    let resolve = |href: &str| -> bool {
        let path = href.split(['#', '?']).next().unwrap_or_default();
        if path == "/app/" || path.is_empty() {
            return true; // the web app, served by the Worker next to the site
        }
        let rel = path.trim_start_matches('/');
        let candidates = [out.join(rel), out.join(rel).join("index.html")];
        candidates.iter().any(|p| p.is_file())
    };
    for lang in LANGS {
        for page in Page::ALL {
            let html = read(&page.out_file(&lang));
            for attr in ["href=\"", "src=\""] {
                for part in html.split(attr).skip(1) {
                    let href = &part[..part.find('"').unwrap()];
                    if href.starts_with('/') && !href.starts_with("//") {
                        assert!(resolve(href), "{} links to missing {href}", page.out_file(&lang));
                    }
                }
            }
        }
    }
}

#[test]
fn seo_files_are_written() {
    let sitemap = read("sitemap.xml");
    let indexable = Page::ALL.iter().filter(|p| p.indexable()).count();
    assert_eq!(sitemap.matches("<loc>").count(), indexable * LANGS.len());
    assert!(read("robots.txt").contains(&format!("Sitemap: {ORIGIN}/sitemap.xml")));
    assert!(read(".well-known/security.txt").contains("Expires: 2027-10-30T00:00:00Z"));
    let llms = read("llms.txt");
    assert!(llms.starts_with("# Streams Reader\n\n> "));
    assert!(llms.contains(&format!("({ORIGIN}/privacy)")));
    let key = nolabs_site::config::INDEXNOW_KEY;
    assert_eq!(read(&format!("{key}.txt")), key);
}
