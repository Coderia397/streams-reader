//! Files for crawlers and machines: robots.txt, sitemap.xml, llms.txt, security.txt and the IndexNow key file.

use crate::config::{CONTACT_EMAIL, DEFAULT_LANG, GITHUB_URL, INDEXNOW_KEY, LANGS, ORIGIN, PARENT_URL, Page};
use crate::i18n::Catalog;

/// A calendar date (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    pub year: i64,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// From days since 1970-01-01 (Howard Hinnant's civil-from-days).
    pub fn from_unix_days(days: i64) -> Self {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = yoe + era * 400 + i64::from(month <= 2);
        Self { year, month, day }
    }

    pub fn today() -> Self {
        let secs =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default();
        Self::from_unix_days(i64::try_from(secs / 86_400).unwrap_or_default())
    }

    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

const CRAWLERS: [&str; 17] = [
    "Googlebot",
    "Bingbot",
    "DuckDuckBot",
    "Applebot",
    "Applebot-Extended",
    "ClaudeBot",
    "Claude-SearchBot",
    "Claude-User",
    "GPTBot",
    "OAI-SearchBot",
    "ChatGPT-User",
    "PerplexityBot",
    "Perplexity-User",
    "Google-Extended",
    "CCBot",
    "Meta-ExternalAgent",
    "Amazonbot",
];

pub fn robots_txt() -> String {
    let rules = "Allow: /\nDisallow: /api/\nDisallow: /app/\n";
    let named: String = CRAWLERS.iter().map(|c| format!("User-agent: {c}\n")).collect();
    format!(
        "# Streams Reader welcomes search engines and AI search and assistant crawlers on its public pages\n\
         # (the landing page, download, help, policies and credits, in every language). The app and its API need an account.\n\
         User-agent: *\n{rules}\n{named}{rules}\nSitemap: {ORIGIN}/sitemap.xml\n"
    )
}

pub fn sitemap_xml() -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">\n",
    );
    for page in Page::ALL.into_iter().filter(|p| p.indexable()) {
        let alternates: String = LANGS
            .iter()
            .map(|l| format!("    <xhtml:link rel=\"alternate\" hreflang=\"{}\" href=\"{}\"/>\n", l.code, page.url(l)))
            .chain(std::iter::once(format!(
                "    <xhtml:link rel=\"alternate\" hreflang=\"x-default\" href=\"{}\"/>\n",
                page.url(&DEFAULT_LANG)
            )))
            .collect();
        for lang in LANGS {
            out.push_str(&format!(
                "  <url>\n    <loc>{}</loc>\n{alternates}    <priority>{}</priority>\n  </url>\n",
                page.url(&lang),
                page.priority()
            ));
        }
    }
    out.push_str("</urlset>\n");
    out
}

/// llms.txt (llmstxt.org): what the site is and where its pages are, in English.
pub fn llms_txt(en: &Catalog) -> String {
    let link = |page: Page, key: &str, note: &str| {
        let sep = if note.is_empty() { String::new() } else { format!(": {note}") };
        format!("- [{}]({}){sep}\n", en.text(key), page.url(&DEFAULT_LANG))
    };
    let langs: Vec<String> = LANGS.iter().filter(|l| !l.is_default()).map(|l| format!("/{}/", l.code)).collect();
    format!(
        "# {name}\n\n> {summary}\n\n## For people\n\n{home}{download}{help}- [{app}]({ORIGIN}/app/): the web app itself (needs an account)\n\n\
         ## Policies\n\n{privacy}{terms}{credits}\n## Optional\n\n- [Sitemap]({ORIGIN}/sitemap.xml)\n\
         - [Source code]({GITHUB_URL}): the app, the Worker and the voice engine, written in Rust and Python (MIT)\n\
         - [Streams]({PARENT_URL}): the parent app, for finding films and series\n\
         - Every page is also available in Arabic, Spanish, French, German, Portuguese, Hindi and Chinese under {langs}\n",
        name = en.text("brand-name"),
        summary = en.text("llms-summary"),
        home = link(Page::Home, "brand-name", &en.text("llms-home")),
        download = link(Page::Download, "download-title", &en.text("llms-download")),
        help = link(Page::Help, "help-title", &en.text("llms-help")),
        app = en.text("nav-open-app"),
        privacy = link(Page::Privacy, "privacy-title", ""),
        terms = link(Page::Terms, "terms-title", ""),
        credits = link(Page::Credits, "credits-title", ""),
        langs = langs.join(", "),
    )
}

/// RFC 9116 security.txt, valid for a year from the build date.
pub fn security_txt(today: Date) -> String {
    let expires = Date::from_unix_days(days_from_civil(today) + 365);
    format!(
        "Contact: mailto:{CONTACT_EMAIL}\nExpires: {}T00:00:00Z\nPreferred-Languages: en, ar\n\
         Canonical: {ORIGIN}/.well-known/security.txt\n",
        expires.iso()
    )
}

/// The IndexNow key file: `/<key>.txt` containing the key.
pub fn indexnow_file() -> (String, String) {
    (format!("{INDEXNOW_KEY}.txt"), INDEXNOW_KEY.to_string())
}

fn days_from_civil(d: Date) -> i64 {
    let y = d.year - i64::from(d.month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(d.month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d.day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        assert_eq!(Date::from_unix_days(0).iso(), "1970-01-01");
        let d = Date { year: 2026, month: 10, day: 30 };
        assert_eq!(Date::from_unix_days(days_from_civil(d)), d);
        assert_eq!(Date::from_unix_days(days_from_civil(Date { year: 2028, month: 2, day: 29 })).iso(), "2028-02-29");
    }

    #[test]
    fn security_txt_expires_in_a_year() {
        let s = security_txt(Date { year: 2026, month: 10, day: 30 });
        assert!(s.contains("Expires: 2027-10-30T00:00:00Z"));
        assert!(s.contains("Contact: mailto:help@streams.watch"));
    }

    #[test]
    fn robots_blocks_app_and_api_for_everyone() {
        let r = robots_txt();
        assert_eq!(r.matches("Disallow: /app/").count(), 2);
        assert_eq!(r.matches("Disallow: /api/").count(), 2);
        assert!(r.contains("User-agent: ClaudeBot"));
        assert!(r.ends_with("Sitemap: https://listen.streams.watch/sitemap.xml\n"));
    }

    #[test]
    fn sitemap_lists_every_indexable_page_in_every_language_with_reciprocal_alternates() {
        let xml = sitemap_xml();
        let indexable: Vec<Page> = Page::ALL.into_iter().filter(|p| p.indexable()).collect();
        assert_eq!(xml.matches("<url>").count(), indexable.len() * LANGS.len());
        assert!(!xml.contains("/404"));
        for page in &indexable {
            for lang in LANGS {
                let loc = format!("<loc>{}</loc>", page.url(&lang));
                assert_eq!(xml.matches(&loc).count(), 1, "{loc}");
                // Every URL appears as an alternate once per entry of its page (8 languages), so hreflang is mutual.
                let alt = format!("hreflang=\"{}\" href=\"{}\"", lang.code, page.url(&lang));
                assert_eq!(xml.matches(&alt).count(), LANGS.len(), "{alt}");
            }
        }
        assert!(xml.contains("xmlns:xhtml=\"http://www.w3.org/1999/xhtml\""));
    }

    #[test]
    fn indexnow_file_contains_its_own_name() {
        let (name, body) = indexnow_file();
        assert_eq!(name, format!("{body}.txt"));
        assert!(body.len() >= 8 && body.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
