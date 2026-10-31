//! The shell every page shares: `<head>` (SEO, social cards, hreflang, fonts), the top bar and the footer.

use serde_json::Value;

use crate::config::{APP_PATH, CHANGELOG_URL, GITHUB_URL, LANGS, Lang, ORIGIN, PARENT_URL, Page};
use crate::data::{Demo, Library};
use crate::html::{esc, json_for_script};
use crate::i18n::Catalog;
use crate::icons::{icon, icon_with_class};
use crate::tokens::ThemeColors;

/// Build-wide facts every page needs.
pub struct Site {
    /// The landing page's stylesheet, and the one every other page uses.
    pub css_landing: String,
    pub css_inner: String,
    pub js_href: String,
    pub theme: ThemeColors,
    pub year: u16,
    /// Inline SVG of the brand mark (from `brand/dist` when present, otherwise the built-in fallback).
    pub mark_svg: String,
    /// Favicon links, already HTML.
    pub icon_links: String,
    /// Languages that have their own social card image in `assets/og/<code>.png`; others use `og_default`.
    pub og_langs: Vec<String>,
    /// The brand build's social card (empty when there is none).
    pub og_default: String,
    /// The hero demo: texts, voices and clip timings.
    pub demo: Demo,
    /// The classics row.
    pub library: Library,
    /// Hero film files present under `/assets/media/` (file names).
    pub media: Vec<String>,
}

impl Site {
    /// Path of the social card image for a language.
    pub fn og_image(&self, lang: &Lang) -> String {
        if self.og_langs.iter().any(|c| c == lang.code) {
            format!("/assets/og/{}.png", lang.code)
        } else {
            self.og_default.clone()
        }
    }

    pub fn has_media(&self, file: &str) -> bool {
        self.media.iter().any(|m| m == file)
    }
}

/// One page being rendered.
pub struct Ctx<'a> {
    pub cat: &'a Catalog,
    pub site: &'a Site,
    pub page: Page,
}

impl Ctx<'_> {
    pub fn lang(&self) -> Lang {
        self.cat.lang
    }

    /// Path of another page in the same language.
    pub fn href(&self, page: Page) -> String {
        page.path(&self.lang())
    }

    pub fn t(&self, key: &str) -> String {
        self.cat.t(key)
    }
}

/// What a page contributes to the shell.
pub struct Doc {
    /// Plain text; escaped here.
    pub title: String,
    pub description: String,
    pub body: String,
    pub json_ld: Vec<Value>,
    /// Load the site script (hero demo, OS detection).
    pub script: bool,
    /// Extra `<head>` HTML (preloads for the page's first image).
    pub preload: String,
}

pub fn render(ctx: &Ctx, doc: &Doc) -> String {
    let lang = ctx.lang();
    format!(
        "<!doctype html>\n<html lang=\"{code}\" dir=\"{dir}\">\n<head>\n{head}</head>\n<body>\n\
         <a class=\"btn skip\" href=\"#main\">{skip}</a>\n<span class=\"sentinel\" aria-hidden=\"true\" data-sentinel></span>\n{header}\n<main id=\"main\" tabindex=\"-1\">\n{body}</main>\n\
         {footer}\n</body>\n</html>\n",
        code = lang.code,
        dir = lang.dir.as_str(),
        head = head(ctx, doc),
        skip = ctx.t("nav-skip"),
        header = header(ctx),
        body = doc.body,
        footer = footer(ctx),
    )
}

fn head(ctx: &Ctx, doc: &Doc) -> String {
    let lang = ctx.lang();
    let site = ctx.site;
    let indexable = ctx.page.indexable();
    let canonical = ctx.page.url(&lang);
    let title = esc(&doc.title);
    let desc = esc(&doc.description);
    let og_image = format!("{ORIGIN}{}", site.og_image(&lang));
    let mut h = String::with_capacity(4096);
    h.push_str("<meta charset=\"utf-8\">\n");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\">\n");
    h.push_str(&format!("<title>{title}</title>\n<meta name=\"description\" content=\"{desc}\">\n"));
    if indexable {
        h.push_str(&format!("<link rel=\"canonical\" href=\"{canonical}\">\n"));
        for alt in LANGS {
            h.push_str(&format!(
                "<link rel=\"alternate\" hreflang=\"{}\" href=\"{}\">\n",
                alt.code,
                ctx.page.url(&alt)
            ));
        }
        h.push_str(&format!(
            "<link rel=\"alternate\" hreflang=\"x-default\" href=\"{}\">\n",
            ctx.page.url(&crate::config::DEFAULT_LANG)
        ));
    } else {
        h.push_str("<meta name=\"robots\" content=\"noindex\">\n");
    }
    h.push_str("<meta name=\"color-scheme\" content=\"dark light\">\n");
    h.push_str(&format!(
        "<meta name=\"theme-color\" content=\"{}\" media=\"(prefers-color-scheme: dark)\">\n\
         <meta name=\"theme-color\" content=\"{}\" media=\"(prefers-color-scheme: light)\">\n",
        site.theme.dark_bg, site.theme.light_bg
    ));
    // All three: the italic face sets the top bar's "Reader" and the hero accent, so it is in the first view too.
    for font in ["inter-latin-wght-normal", "literata-latin-wght-normal", "literata-latin-wght-italic"] {
        h.push_str(&format!(
            "<link rel=\"preload\" href=\"/assets/fonts/{font}.woff2\" as=\"font\" type=\"font/woff2\" crossorigin>\n"
        ));
    }
    h.push_str(&doc.preload);
    let css = if ctx.page == Page::Home { &site.css_landing } else { &site.css_inner };
    h.push_str(&format!("<link rel=\"stylesheet\" href=\"{css}\">\n"));
    h.push_str(&site.icon_links);
    // Social cards.
    let site_name = ctx.cat.t("brand-name");
    h.push_str(&format!(
        "<meta property=\"og:type\" content=\"website\">\n<meta property=\"og:site_name\" content=\"{site_name}\">\n\
         <meta property=\"og:title\" content=\"{title}\">\n<meta property=\"og:description\" content=\"{desc}\">\n\
         <meta property=\"og:url\" content=\"{canonical}\">\n<meta property=\"og:image\" content=\"{og_image}\">\n\
         <meta property=\"og:image:width\" content=\"1200\">\n<meta property=\"og:image:height\" content=\"630\">\n\
         <meta property=\"og:image:alt\" content=\"{alt}\">\n<meta property=\"og:locale\" content=\"{locale}\">\n",
        alt = ctx.t("og-image-alt"),
        locale = lang.og_locale,
    ));
    for alt in LANGS.iter().filter(|l| **l != lang) {
        h.push_str(&format!("<meta property=\"og:locale:alternate\" content=\"{}\">\n", alt.og_locale));
    }
    h.push_str(&format!(
        "<meta name=\"twitter:card\" content=\"summary_large_image\">\n<meta name=\"twitter:title\" content=\"{title}\">\n\
         <meta name=\"twitter:description\" content=\"{desc}\">\n<meta name=\"twitter:image\" content=\"{og_image}\">\n"
    ));
    for ld in &doc.json_ld {
        h.push_str(&format!("<script type=\"application/ld+json\">{}</script>\n", json_for_script(ld)));
    }
    if doc.script {
        h.push_str(&format!("<script src=\"{}\" defer></script>\n", site.js_href));
    }
    h
}

/// The brand lockup: mark tile and "Streams Reader" wordmark.
pub fn lockup(ctx: &Ctx) -> String {
    format!(
        "<a class=\"lockup\" href=\"{home}\" aria-label=\"{label}\"><span class=\"lockup__mark\">{mark}</span>\
         <span class=\"lockup__word\" aria-hidden=\"true\"><b>{parent}</b> <i>{product}</i></span></a>",
        home = ctx.href(Page::Home),
        label = ctx.t("nav-home-label"),
        mark = ctx.site.mark_svg,
        parent = ctx.t("brand-parent"),
        product = ctx.t("brand-product"),
    )
}

fn header(ctx: &Ctx) -> String {
    let home = ctx.href(Page::Home);
    let current = |p: Page| if ctx.page == p { " aria-current=\"page\"" } else { "" };
    let links = format!(
        "<li><a href=\"{home}#library\">{library}</a></li><li><a href=\"{home}#how\">{how}</a></li>\
         <li><a href=\"{home}#voices\">{voices}</a></li><li><a href=\"{download}\"{dl_cur}>{dl}</a></li>",
        library = ctx.t("nav-library"),
        how = ctx.t("nav-how"),
        voices = ctx.t("nav-voices"),
        download = ctx.href(Page::Download),
        dl_cur = current(Page::Download),
        dl = ctx.t("nav-download"),
    );
    format!(
        "<header class=\"topbar\" data-topbar>\n<nav class=\"topbar__inner wrap\" aria-label=\"{nav_label}\">\n{lockup}\n\
         <ul class=\"topbar__links\">{links}</ul>\n<div class=\"topbar__actions\">\n\
         <a class=\"btn icon-btn topbar__gh\" href=\"{GITHUB_URL}\" aria-label=\"{gh}\">{gh_icon}</a>\n\
         <a class=\"btn btn--primary btn--sm\" href=\"{APP_PATH}\">{open}</a>\n\
         <details class=\"menu\" data-menu><summary class=\"btn icon-btn\" aria-label=\"{menu}\">{menu_icon}{close_icon}</summary>\
         <div class=\"menu__sheet\"><ul class=\"menu__links\">{links}<li><a href=\"{help}\"{help_cur}>{help_label}</a></li>\
         <li><a href=\"{GITHUB_URL}\">{gh_icon}GitHub</a></li></ul></div></details>\n</div>\n</nav>\n</header>",
        nav_label = ctx.t("nav-label"),
        lockup = lockup(ctx),
        gh = ctx.t("nav-github"),
        gh_icon = icon("github-fill"),
        open = ctx.t("nav-open-app"),
        menu = ctx.t("nav-menu"),
        menu_icon = icon_with_class("menu-line", "ri menu__open"),
        close_icon = icon_with_class("close-line", "ri menu__close"),
        help = ctx.href(Page::Help),
        help_cur = current(Page::Help),
        help_label = ctx.t("nav-help"),
    )
}

fn footer(ctx: &Ctx) -> String {
    let lang = ctx.lang();
    let home = ctx.href(Page::Home);
    let download = ctx.href(Page::Download);
    let link = |href: &str, key: &str| format!("<li><a href=\"{href}\">{}</a></li>", ctx.t(key));
    let languages: String = LANGS
        .iter()
        .map(|l| {
            let page = if ctx.page.indexable() { ctx.page } else { Page::Home };
            let cur = if *l == lang { " aria-current=\"true\"" } else { "" };
            format!(
                "<li><a href=\"{}\" hreflang=\"{code}\" lang=\"{code}\"{cur}>{}</a></li>",
                page.path(l),
                esc(l.native_name),
                code = l.code
            )
        })
        .collect();
    let col = |title: &str, items: String| {
        format!("<div class=\"footer__col\"><h2 class=\"footer__title\">{}</h2><ul>{items}</ul></div>\n", ctx.t(title))
    };
    let kokoro = "<a href=\"https://huggingface.co/hexgrad/Kokoro-82M\">Kokoro-82M</a>";
    let ming = "<a href=\"https://huggingface.co/inclusionAI/Ming-3.0-tts-0.5B\">Ming-3.0-tts</a>";
    let parent = format!("<a href=\"{PARENT_URL}\">Streams</a>");
    let cols = [
        col(
            "footer-get-started",
            [
                link(APP_PATH, "footer-listen-online"),
                link(&format!("{download}#android"), "footer-android"),
                link(&format!("{download}#desktop"), "footer-desktop"),
                link(&format!("{home}#everywhere"), "footer-extension"),
                link(&download, "footer-all-downloads"),
            ]
            .concat(),
        ),
        col(
            "footer-library",
            [
                link(&format!("{home}#library"), "footer-classics"),
                link(&format!("{}#library", ctx.href(Page::Credits)), "footer-book-sources"),
            ]
            .concat(),
        ),
        col(
            "footer-help-title",
            [
                link(&ctx.href(Page::Help), "nav-help"),
                link(&format!("{home}#faq"), "footer-faq"),
                link(CHANGELOG_URL, "footer-changelog"),
            ]
            .concat(),
        ),
        col(
            "footer-project",
            [
                link(GITHUB_URL, "footer-source"),
                link("/.well-known/security.txt", "footer-security"),
                link(&format!("{GITHUB_URL}/blob/main/LICENSE"), "footer-licence"),
            ]
            .concat(),
        ),
        col(
            "footer-legal",
            [
                link(&ctx.href(Page::Privacy), "footer-privacy"),
                link(&ctx.href(Page::Terms), "footer-terms"),
                link(&ctx.href(Page::Credits), "footer-credits"),
            ]
            .concat(),
        ),
    ]
    .concat();
    format!(
        "<footer class=\"footer\">\n<div class=\"wrap footer__grid\">\n<div class=\"footer__brand\">\n{lockup}\n\
         <p class=\"footer__tagline\">{tagline}</p>\n<p class=\"footer__parent\">{parent_line}</p>\n</div>\n{cols}</div>\n\
         <div class=\"wrap footer__base\">\n<div class=\"footer__langs\"><h2 class=\"footer__title\" id=\"lang-title\">{icon_tr}{lang_title}</h2>\
         <ul aria-labelledby=\"lang-title\">{languages}</ul></div>\n<p>{credits}</p>\n<p>{copyright}</p>\n</div>\n</footer>",
        lockup = lockup(ctx),
        tagline = ctx.t("footer-tagline"),
        parent_line = ctx.cat.t_html("footer-parent", &[("streams", &parent)]),
        icon_tr = icon("translate-2"),
        lang_title = ctx.t("footer-language"),
        credits = ctx.cat.t_html("footer-credits-line", &[("kokoro", kokoro), ("ming", ming)]),
        copyright = ctx.cat.t_args("footer-copyright", &[("year", &ctx.site.year.to_string())]),
    )
}

/// A page heading block used by the inner pages.
pub fn page_intro(eyebrow: &str, title: &str, lede: &str) -> String {
    format!(
        "<header class=\"intro wrap\">\n<p class=\"eyebrow\">{eyebrow}</p>\n<h1 class=\"intro__title\">{title}</h1>\n\
         <p class=\"intro__lede\">{lede}</p>\n</header>\n"
    )
}
