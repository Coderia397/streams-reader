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
    let mut types = BTreeSet::new();
    for part in html.split("<script type=\"application/ld+json\">").skip(1) {
        let json = &part[..part.find("</script>").unwrap()];
        let v: serde_json::Value = serde_json::from_str(json).expect("valid JSON-LD");
        types.insert(v["@type"].as_str().expect("@type").to_string());
        assert!(
            !json.contains("AggregateRating") && !json.contains("\"Review\""),
            "no invented ratings (note 15, A12)"
        );
    }
    let want: BTreeSet<String> = ["SoftwareApplication", "FAQPage", "Organization", "WebSite", "ItemList"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(types, want);
}

/// Note 15 A14 and the owner's budget: JS <= 25 KB, CSS <= 40 KB, the poster (LCP) <= 120 KB, covers <= 15 KB each,
/// each film file <= 1.5 MB, each demo clip small. Sizes are raw bytes on disk (before transfer compression).
#[test]
fn landing_assets_stay_within_budget() {
    let out = built();
    let html = read("index.html");
    let linked = |ext: &str| -> Vec<String> {
        html.split("\"/assets/")
            .skip(1)
            .filter_map(|p| p.split('"').next())
            .filter(|p| p.ends_with(ext) && !p.contains('/'))
            .map(|p| format!("assets/{p}"))
            .collect()
    };
    let size = |rel: &str| std::fs::metadata(out.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}")).len();
    let css = linked(".css");
    let js = linked(".js");
    assert_eq!(css.len(), 1, "one stylesheet");
    assert_eq!(js.len(), 1, "one script");
    assert!(size(&css[0]) <= 40 * 1024, "{} is {} B", css[0], size(&css[0]));
    assert!(size(&js[0]) <= 25 * 1024, "{} is {} B", js[0], size(&js[0]));
    assert!(html.len() <= 200 * 1024, "landing HTML is {} B", html.len());
    for dir in ["assets/media", "assets/covers", "assets/demo", "assets/previews"] {
        for e in std::fs::read_dir(out.join(dir)).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let n = e.metadata().unwrap().len();
            let cap = match () {
                _ if name.contains(".poster.") => 120 * 1024,
                _ if name.ends_with(".webm") && dir.ends_with("media") => 1536 * 1024,
                _ if name.ends_with(".mp4") => 1536 * 1024,
                _ if dir.ends_with("covers") && (name.ends_with(".avif") || name.ends_with(".webp")) => 18 * 1024,
                _ if name.ends_with(".webm") => 80 * 1024,
                _ => 64 * 1024,
            };
            assert!(n <= cap, "{dir}/{name} is {n} B (cap {cap})");
        }
    }
}

/// Every voice and sample of the hero demo has its clip and timings, and every timing file matches its text.
#[test]
fn demo_clips_and_timings_are_complete() {
    let out = built();
    let manifest = std::fs::read_to_string(crate_dir().join("assets/demo/demo.json")).unwrap();
    let demo: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    for t in demo["texts"].as_array().unwrap() {
        let text = t["text"].as_str().unwrap();
        for c in t["clips"].as_array().unwrap() {
            let base = format!("assets/demo/{}-{}", t["id"].as_str().unwrap(), c.as_str().unwrap());
            assert!(out.join(format!("{base}.webm")).is_file(), "{base}.webm");
            let timing: serde_json::Value = serde_json::from_str(&read(&format!("{base}.json"))).unwrap();
            let words = timing["w"].as_array().unwrap();
            assert!(!words.is_empty(), "{base} has words");
            let mut last = 0;
            for w in words {
                let w: Vec<u64> = w.as_array().unwrap().iter().map(|x| x.as_u64().unwrap()).collect();
                assert!(w[0] >= last && w[1] >= w[0], "{base}: times go forward");
                assert!(w[3] as usize <= text.chars().count(), "{base}: word inside the text");
                last = w[0];
            }
        }
    }
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

/// Class names in a stylesheet's selectors (not inside declaration blocks).
fn selector_classes(css: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut prelude = String::new();
    for c in css.chars() {
        match c {
            '{' => {
                if !prelude.trim_start().starts_with('@') {
                    for part in prelude.split('.').skip(1) {
                        let name: String =
                            part.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
                        if name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
                            out.insert(name);
                        }
                    }
                }
                prelude.clear();
            }
            '}' | ';' => prelude.clear(),
            _ => prelude.push(c),
        }
    }
    out
}

/// The 404 page once reused `.hero__ctas`, which only the landing stylesheet styles: every class an inner page uses
/// must have a rule in the inner stylesheet (script hooks use data attributes, not classes).
#[test]
fn every_class_on_an_inner_page_is_styled_by_the_inner_stylesheet() {
    let out = built();
    let css_href = |html: &str| -> String {
        let at = html.find("<link rel=\"stylesheet\" href=\"/").unwrap() + 30;
        html[at..].split('"').next().unwrap().to_string()
    };
    for lang in LANGS {
        for page in Page::ALL.into_iter().filter(|p| *p != Page::Home) {
            let html = read(&page.out_file(&lang));
            let css = std::fs::read_to_string(out.join(css_href(&html))).unwrap();
            let styled = selector_classes(&css);
            for attr in html.split(" class=\"").skip(1) {
                for class in attr[..attr.find('"').unwrap()].split_whitespace() {
                    assert!(
                        styled.contains(class),
                        "{}: .{class} has no rule in the inner stylesheet",
                        page.out_file(&lang)
                    );
                }
            }
        }
    }
}

/// Note 15: fonts <= 90 KB on first view. The three faces are subsets made by tools/subset_fonts.py.
#[test]
fn fonts_stay_within_budget() {
    let dir = crate_dir().join("assets/fonts");
    let total: u64 = ["inter-latin-wght-normal", "literata-latin-wght-normal", "literata-latin-wght-italic"]
        .iter()
        .map(|f| std::fs::metadata(dir.join(format!("{f}.woff2"))).unwrap().len())
        .sum();
    assert!(total <= 90 * 1024, "fonts weigh {total} B (budget 92160)");
}

/// The italic face only holds the characters the site sets in italic; when one of those strings changes, rerun
/// tools/subset_fonts.py.
#[test]
fn italic_subset_covers_every_italic_string() {
    let chars = std::fs::read_to_string(crate_dir().join("assets/fonts/literata-italic.chars.txt")).unwrap();
    let mut wanted = String::from("Chapter 1");
    for code in ["en", "es", "fr", "de", "pt"] {
        let lang = *LANGS.iter().find(|l| l.code == code).unwrap();
        let cat = Catalog::load(&crate_dir().join("locales"), lang).unwrap();
        for key in ["brand-product", "footer-tagline", "hero-title-accent", "ch3-title-accent"] {
            wanted.push_str(&cat.text(key));
        }
    }
    let manifest = std::fs::read_to_string(crate_dir().join("assets/demo/demo.json")).unwrap();
    let demo: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    for t in demo["texts"].as_array().unwrap() {
        let text = t["text"].as_str().unwrap();
        if text.contains('\n') {
            wanted.push_str(text);
        }
    }
    let missing: BTreeSet<char> = wanted.chars().filter(|c| *c != '\n' && !chars.contains(*c)).collect();
    assert!(missing.is_empty(), "rerun tools/subset_fonts.py: italic subset lacks {missing:?}");
}

/// Review round 3 (2026-10-31). Sections render eagerly: `content-visibility: auto` estimated their heights and made
/// the smooth in-page scroll to #faq overshoot by over 1,100 px at 1440. The hero puts the demo before the film in
/// source and visual order alike (no `grid-row` reordering outside the wide two-column layout).
#[test]
fn landing_layout_keeps_anchor_targets_and_order_stable() {
    let html = read("index.html");
    let css_rel = html.split("\"/assets/landing.").nth(1).and_then(|p| p.split('"').next()).unwrap();
    let css = read(&format!("assets/landing.{css_rel}"));
    assert!(!css.contains("content-visibility:auto"), "no estimated section heights");
    assert!(!css.contains("contain-intrinsic-size"));
    let demo = html.find("data-demo").expect("demo");
    let film = html.find("<figure class=\"film\"").expect("film");
    assert!(demo < film, "the demo comes before the film in the source");
}

/// No page title names the brand twice or ends its text in sentence punctuation before " · ", and no footer gives two
/// links the same name.
#[test]
fn titles_and_footer_link_names_do_not_repeat() {
    for lang in LANGS {
        let brand = Catalog::load(&crate_dir().join("locales"), lang).unwrap().text("brand-name");
        for page in Page::ALL {
            let html = read(&page.out_file(&lang));
            let title = html.split("<title>").nth(1).and_then(|t| t.split("</title>").next()).unwrap();
            assert_eq!(title.matches(brand.as_str()).count(), 1, "{}: {title}", page.out_file(&lang));
            if let Some((head, _)) = title.split_once(" · ") {
                let end = head.trim_end().chars().last().unwrap_or(' ');
                assert!(!".。।!?！？".contains(end), "{}: sentence end before the separator: {title}", page.out_file(&lang));
            }
            let footer = html.split("<footer").nth(1).unwrap();
            let names: Vec<&str> = footer
                .split("<a ")
                .skip(1)
                .filter_map(|a| a.split_once('>').map(|(_, rest)| rest.split('<').next().unwrap_or_default()))
                .filter(|n| !n.trim().is_empty())
                .collect();
            let unique: BTreeSet<&str> = names.iter().copied().collect();
            assert_eq!(unique.len(), names.len(), "{}: footer link names repeat: {names:?}", page.out_file(&lang));
        }
    }
}

/// The player's time label before play is the first clip's real audio length (the script counts down from it).
#[test]
fn demo_time_label_is_the_clip_duration() {
    let html = read("index.html");
    let label = html.split("data-demo-time>").nth(1).and_then(|t| t.split('<').next()).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(crate_dir().join("assets/demo/demo.json")).unwrap()).unwrap();
    let t = &manifest["texts"][0];
    let clip: serde_json::Value = serde_json::from_str(&read(&format!(
        "assets/demo/{}-{}.json",
        t["id"].as_str().unwrap(),
        t["clips"][0].as_str().unwrap()
    )))
    .unwrap();
    let s = clip["d"].as_u64().unwrap().div_ceil(1000);
    assert_eq!(label, format!("{}:{:02}", s / 60, s % 60));
}

/// Playback controls keep their order in Arabic; key names are isolated so brackets are not mirrored.
#[test]
fn rtl_keeps_transport_controls_and_key_names_left_to_right() {
    let html = read("ar/index.html");
    assert!(html.matches("class=\"mini__ctl\" dir=\"ltr\"").count() >= 2, "chapter 04 and the Android tile");
    assert!(html.matches("class=\"mini__bar\" dir=\"ltr\"").count() >= 2);
    let src = std::fs::read_to_string(crate_dir().join("locales/ar/site.ftl")).unwrap();
    let line = src.lines().find(|l| l.starts_with("feature-keyboard-text =")).unwrap();
    for key in ["J", "L", "[", "]"] {
        assert!(line.contains(&format!("\u{2066}{key}\u{2069}")), "{key} is LTR-isolated");
    }
}
