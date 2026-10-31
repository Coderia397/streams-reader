//! The build: load strings and tokens, compile the stylesheet, render every page in every language, write the SEO
//! files and copy the static assets into `dist/`.

use std::path::{Path, PathBuf};

use crate::assets::{self, Brand};
use crate::config::{LANGS, Page};
use crate::data::{Demo, Library};
use crate::i18n::Catalog;
use crate::layout::{self, Ctx, Site};
use crate::seo::{self, Date};
use crate::tokens::ThemeColors;
use crate::{Error, content_hash, css, pages};

/// Marks a folder as site-gen output, so a rebuild may safely empty it.
const MARKER: &str = ".site-gen";

/// Where everything is read from and written to.
#[derive(Debug, Clone)]
pub struct Paths {
    /// `crates/site`.
    pub crate_dir: PathBuf,
    /// The repository root (holds `brand/` and `crates/web/`).
    pub repo_root: PathBuf,
    /// The output folder (`crates/site/dist`).
    pub out: PathBuf,
}

impl Paths {
    /// Paths relative to this crate's source folder.
    pub fn from_manifest() -> Self {
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = crate_dir.join("../..");
        let out = crate_dir.join("dist");
        Self { crate_dir, repo_root, out }
    }

    pub fn brand_root(&self) -> PathBuf {
        self.repo_root.join("brand")
    }

    pub fn tokens_dir(&self) -> PathBuf {
        self.brand_root().join("dist/tokens")
    }

    pub fn fonts(&self) -> PathBuf {
        self.crate_dir.join("assets/fonts")
    }
}

/// What the build produced, for the command-line summary.
#[derive(Debug, Default)]
pub struct BuildReport {
    pub pages: usize,
    pub files: usize,
    /// Size of each stylesheet (entry name, bytes).
    pub css_bytes: Vec<(&'static str, usize)>,
    pub js_bytes: usize,
    pub brand_files: usize,
    pub warnings: Vec<String>,
}

pub fn build(paths: &Paths, today: Date) -> Result<BuildReport, Error> {
    let mut report = BuildReport::default();
    let catalogs: Vec<Catalog> =
        LANGS.iter().map(|l| Catalog::load(&paths.crate_dir.join("locales"), *l)).collect::<Result<_, _>>()?;
    let theme = ThemeColors::load(&paths.tokens_dir().join("tokens.json"))?;

    reset_out(&paths.out)?;
    let out = &paths.out;

    // Stylesheet and script, named by content hash so they can be cached for a year.
    let mut css_hrefs = Vec::new();
    for entry in css::ENTRIES {
        let css = css::compile(&paths.crate_dir.join("style"), &paths.tokens_dir(), entry)?;
        let name = format!("assets/{entry}.{}.css", content_hash(css.as_bytes()));
        assets::write(&out.join(&name), &css)?;
        report.css_bytes.push((entry, css.len()));
        css_hrefs.push(format!("/{name}"));
    }
    let js_src = paths.crate_dir.join("assets/js/site.js");
    let js = std::fs::read(&js_src).map_err(|e| Error::io(&js_src, e))?;
    let js_name = format!("assets/site.{}.js", content_hash(&js));
    assets::write(&out.join(&js_name), &js)?;
    report.js_bytes = js.len();

    // Fonts (subsets of the web crate's, made by tools/subset_fonts.py), the demo clips, cover previews, covers and the hero film.
    for font in assets::FONTS {
        assets::copy(&paths.fonts().join(font), &out.join("assets/fonts").join(font))?;
    }
    let src = |d: &str| paths.crate_dir.join("assets").join(d);
    let is_data =
        |p: &Path| p.extension().is_some_and(|e| e == "md") || p.ends_with("demo.json") || p.ends_with("covers.json");
    let demo = Demo::load(&src("demo"))?;
    let library = Library::load(&src("covers").join("covers.json"))?;
    let mut media = Vec::new();
    for dir in ["demo", "previews", "covers", "media"] {
        if src(dir).is_dir() {
            let copied = assets::copy_dir(&src(dir), &out.join("assets").join(dir), is_data)?;
            if dir == "media" {
                media = copied.iter().map(|p| p.to_string_lossy().into_owned()).collect();
            }
        }
    }
    // Images of the auth emails (crates/mail), served at the stable URL `/email/<file>` that sent mail points to.
    if src("email").is_dir() {
        assets::copy_dir(&src("email"), &out.join("email"), |_| false)?;
    } else {
        report.warnings.push("no email images in assets/email".into());
    }
    if media.is_empty() {
        report.warnings.push("no hero film in assets/media; run tools/media.py".into());
    }

    // Brand output (favicons, social card, the mark), when the brand build has produced it.
    let brand = Brand::find(&paths.brand_root());
    if let Some(web) = &brand.web_dir {
        report.brand_files += assets::copy_dir(web, &out.join("assets/brand/web"), |p| p.starts_with("splash"))?.len();
    }
    let mark_svg = brand
        .mark_svg
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| assets::inline_svg(&s))
        .unwrap_or_else(|| {
            report.warnings.push("brand mark not found; using the built-in mark".into());
            assets::FALLBACK_MARK.to_string()
        });
    let (icon_links, og_default) = favicons(&brand, out, &theme, &mut report)?;

    // Per-language social cards override the brand's (optional: assets/og/<lang>.png).
    let og_dir = paths.crate_dir.join("assets/og");
    let og_langs: Vec<String> = if og_dir.is_dir() {
        assets::copy_dir(&og_dir, &out.join("assets/og"), |_| false)?
            .iter()
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect()
    } else {
        Vec::new()
    };

    let site = Site {
        css_landing: css_hrefs[0].clone(),
        css_inner: css_hrefs[1].clone(),
        js_href: format!("/{js_name}"),
        theme,
        year: u16::try_from(today.year).unwrap_or(2026),
        mark_svg,
        icon_links,
        og_langs,
        og_default,
        demo,
        library,
        media,
    };

    for cat in &catalogs {
        for page in Page::ALL {
            let ctx = Ctx { cat, site: &site, page };
            let doc = pages::render(&ctx);
            assets::write(&out.join(page.out_file(&cat.lang)), layout::render(&ctx, &doc))?;
            report.pages += 1;
        }
    }

    // Files for crawlers.
    assets::write(&out.join("robots.txt"), seo::robots_txt())?;
    assets::write(&out.join("sitemap.xml"), seo::sitemap_xml())?;
    assets::write(&out.join("llms.txt"), seo::llms_txt(&catalogs[0]))?;
    assets::write(&out.join(".well-known/security.txt"), seo::security_txt(today))?;
    let (key_name, key_body) = seo::indexnow_file();
    assets::write(&out.join(key_name), key_body)?;

    let missing: Vec<String> =
        catalogs.iter().flat_map(|c| c.missing().into_iter().map(move |k| format!("{}: {k}", c.lang.code))).collect();
    if !missing.is_empty() {
        return Err(Error::Missing(missing.join(", ")));
    }
    report.files = assets::walk(out)?.len();
    Ok(report)
}

/// Favicon links and the default social card: the brand build's when present, otherwise a fallback SVG favicon
/// drawn in token colours.
fn favicons(
    brand: &Brand,
    out: &Path,
    theme: &ThemeColors,
    report: &mut BuildReport,
) -> Result<(String, String), Error> {
    let web = brand.web_dir.as_ref().filter(|w| w.join("favicon.svg").is_file());
    let Some(web) = web else {
        assets::write(&out.join("favicon.svg"), assets::fallback_favicon(&theme.brand, &theme.on_brand, &theme.ember))?;
        report.warnings.push("brand/dist/web not found; wrote a fallback favicon and no social card".into());
        return Ok(("<link rel=\"icon\" href=\"/favicon.svg\" type=\"image/svg+xml\">\n".into(), String::new()));
    };
    let mut links = String::from("<link rel=\"icon\" href=\"/assets/brand/web/favicon.svg\" type=\"image/svg+xml\">\n");
    if web.join("favicon.ico").is_file() {
        // Browsers ask for /favicon.ico on their own; serve it from the root too.
        assets::copy(&web.join("favicon.ico"), &out.join("favicon.ico"))?;
        links.push_str("<link rel=\"icon\" href=\"/favicon.ico\" sizes=\"48x48\">\n");
    }
    if web.join("apple-touch-icon.png").is_file() {
        links.push_str("<link rel=\"apple-touch-icon\" href=\"/assets/brand/web/apple-touch-icon.png\">\n");
    }
    let og =
        if web.join("og-image.png").is_file() { "/assets/brand/web/og-image.png".to_string() } else { String::new() };
    Ok((links, og))
}

/// Empty the output folder, but only if it is missing, empty, or a previous site-gen output.
fn reset_out(out: &Path) -> Result<(), Error> {
    if out.exists() {
        let is_ours = out.join(MARKER).exists();
        let is_empty = std::fs::read_dir(out).map_err(|e| Error::io(out, e))?.next().is_none();
        if !is_ours && !is_empty {
            return Err(Error::io(
                out,
                std::io::Error::other("output folder exists and was not made by site-gen; refusing to empty it"),
            ));
        }
        std::fs::remove_dir_all(out).map_err(|e| Error::io(out, e))?;
    }
    assets::write(&out.join(MARKER), "Generated by site-gen; the whole folder is rebuilt on every run.\n")
}
