//! What the site is made of: its origin, languages and pages, and where each one lives on disk and on the web.

pub const ORIGIN: &str = "https://reader.streams.watch";
pub const GITHUB_URL: &str = "https://github.com/Coderia397/streams-reader";
pub const CONTACT_EMAIL: &str = "help@streams.watch";
pub const PARENT_URL: &str = "https://streams.watch";
pub const CHANGELOG_URL: &str = "https://github.com/Coderia397/streams-reader/blob/main/CHANGELOG.md";
pub const RELEASES_URL: &str = "https://github.com/Coderia397/streams-reader/releases";
/// The comparison on the landing page quotes this page; the facts were checked on `COMPARE_CHECKED` (note 15, D16).
pub const COMPARE_SOURCE_URL: &str = "https://elevenreader.io/pricing";
pub const COMPARE_CHECKED: &str = "2026-10-30";
/// The web app, served by the Worker next to the site.
pub const APP_PATH: &str = "/app/";
/// IndexNow key: a public identifier proving ownership of the host to search engines (not a secret).
pub const INDEXNOW_KEY: &str = "5f1c2a9e7b4d4c3fa8e06d1b92c7e4a1";

/// Text direction of a language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Ltr,
    Rtl,
}

impl Dir {
    pub fn as_str(self) -> &'static str {
        match self {
            Dir::Ltr => "ltr",
            Dir::Rtl => "rtl",
        }
    }
}

/// One language of the site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lang {
    /// BCP 47 tag, also the folder under `locales/` and the URL prefix (except for the default).
    pub code: &'static str,
    pub dir: Dir,
    /// The language's own name, for the language menu.
    pub native_name: &'static str,
    /// Open Graph locale (`og:locale`).
    pub og_locale: &'static str,
}

impl Lang {
    pub fn is_default(&self) -> bool {
        self.code == DEFAULT_LANG.code
    }

    /// URL prefix: empty for the default language, `/ar` for Arabic.
    pub fn prefix(&self) -> String {
        if self.is_default() { String::new() } else { format!("/{}", self.code) }
    }
}

pub const DEFAULT_LANG: Lang = Lang { code: "en", dir: Dir::Ltr, native_name: "English", og_locale: "en_US" };

pub const LANGS: [Lang; 8] = [
    DEFAULT_LANG,
    Lang { code: "ar", dir: Dir::Rtl, native_name: "العربية", og_locale: "ar_AR" },
    Lang { code: "es", dir: Dir::Ltr, native_name: "Español", og_locale: "es_ES" },
    Lang { code: "fr", dir: Dir::Ltr, native_name: "Français", og_locale: "fr_FR" },
    Lang { code: "de", dir: Dir::Ltr, native_name: "Deutsch", og_locale: "de_DE" },
    Lang { code: "pt", dir: Dir::Ltr, native_name: "Português", og_locale: "pt_BR" },
    Lang { code: "hi", dir: Dir::Ltr, native_name: "हिन्दी", og_locale: "hi_IN" },
    Lang { code: "zh", dir: Dir::Ltr, native_name: "简体中文", og_locale: "zh_CN" },
];

/// Every page the generator writes, in every language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Download,
    Help,
    Privacy,
    Terms,
    Credits,
    NotFound,
}

impl Page {
    pub const ALL: [Page; 7] =
        [Page::Home, Page::Download, Page::Help, Page::Privacy, Page::Terms, Page::Credits, Page::NotFound];

    /// The path segment after the language prefix; empty for the home page.
    pub fn slug(self) -> &'static str {
        match self {
            Page::Home => "",
            Page::Download => "download",
            Page::Help => "help",
            Page::Privacy => "privacy",
            Page::Terms => "terms",
            Page::Credits => "credits",
            Page::NotFound => "404",
        }
    }

    /// Pages listed in the sitemap and given hreflang alternates (the 404 page is neither).
    pub fn indexable(self) -> bool {
        self != Page::NotFound
    }

    /// Sitemap priority.
    pub fn priority(self) -> &'static str {
        match self {
            Page::Home => "1.0",
            Page::Download | Page::Help => "0.7",
            _ => "0.4",
        }
    }

    /// The canonical path: `/`, `/ar/`, `/download`, `/ar/download`.
    pub fn path(self, lang: &Lang) -> String {
        match self {
            Page::Home => format!("{}/", lang.prefix()),
            _ => format!("{}/{}", lang.prefix(), self.slug()),
        }
    }

    pub fn url(self, lang: &Lang) -> String {
        format!("{ORIGIN}{}", self.path(lang))
    }

    /// Where the page is written under `dist/`: `index.html`, `ar/index.html`, `download/index.html`, `404.html`.
    pub fn out_file(self, lang: &Lang) -> String {
        let dir = if lang.is_default() { String::new() } else { format!("{}/", lang.code) };
        match self {
            Page::Home => format!("{dir}index.html"),
            Page::NotFound => format!("{dir}404.html"),
            _ => format!("{dir}{}/index.html", self.slug()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_files_follow_the_language_prefix() {
        let ar = LANGS[1];
        assert_eq!(Page::Home.path(&DEFAULT_LANG), "/");
        assert_eq!(Page::Home.path(&ar), "/ar/");
        assert_eq!(Page::Download.path(&ar), "/ar/download");
        assert_eq!(Page::Download.out_file(&DEFAULT_LANG), "download/index.html");
        assert_eq!(Page::NotFound.out_file(&ar), "ar/404.html");
        assert_eq!(Page::Home.url(&DEFAULT_LANG), "https://reader.streams.watch/");
    }

    #[test]
    fn language_codes_are_unique_and_only_arabic_is_rtl() {
        let mut codes: Vec<_> = LANGS.iter().map(|l| l.code).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), LANGS.len());
        assert!(LANGS.iter().all(|l| (l.dir == Dir::Rtl) == (l.code == "ar")));
    }
}
