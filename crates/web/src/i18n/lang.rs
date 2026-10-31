//! The interface languages, how one is chosen, and the text direction. Pure apart from [`detect`], so the choice
//! logic is unit tested natively.

/// Every language the app ships. The `.ftl` file for each lives in `crates/web/locales/<code>/app.ftl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Lang {
    #[default]
    En,
    Ar,
    Es,
    Fr,
    De,
    Pt,
    Hi,
    Zh,
}

impl Lang {
    pub const ALL: [Lang; 8] = [Lang::En, Lang::Ar, Lang::Es, Lang::Fr, Lang::De, Lang::Pt, Lang::Hi, Lang::Zh];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ar => "ar",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::De => "de",
            Lang::Pt => "pt",
            Lang::Hi => "hi",
            Lang::Zh => "zh",
        }
    }

    /// BCP 47 tag for `lang=` attributes and `Intl` (Chinese is Simplified).
    pub fn tag(self) -> &'static str {
        match self {
            Lang::Zh => "zh-Hans",
            other => other.code(),
        }
    }

    /// The language's name in itself, for the picker.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ar => "العربية",
            Lang::Es => "Español",
            Lang::Fr => "Français",
            Lang::De => "Deutsch",
            Lang::Pt => "Português",
            Lang::Hi => "हिन्दी",
            Lang::Zh => "简体中文",
        }
    }

    /// The decimal separator in interface numbers ("1,5×" in German). Arabic, Hindi and Chinese keep the full stop,
    /// as the app writes Latin digits in every language.
    pub fn decimal_separator(self) -> char {
        match self {
            Lang::Es | Lang::Fr | Lang::De | Lang::Pt => ',',
            Lang::En | Lang::Ar | Lang::Hi | Lang::Zh => '.',
        }
    }

    pub fn is_rtl(self) -> bool {
        self == Lang::Ar
    }

    pub fn dir(self) -> &'static str {
        if self.is_rtl() { "rtl" } else { "ltr" }
    }

    /// Parses a code or a full tag (`pt-BR`, `zh-Hans-CN`, `AR`). Traditional Chinese is not offered, so `zh-TW` and
    /// `zh-Hant` do not match.
    pub fn parse(tag: &str) -> Option<Lang> {
        let lower = tag.trim().to_ascii_lowercase().replace('_', "-");
        if lower.starts_with("zh") && (lower.contains("hant") || lower.ends_with("-tw") || lower.ends_with("-hk")) {
            return None;
        }
        let primary = lower.split('-').next()?;
        Lang::ALL.into_iter().find(|l| l.code() == primary)
    }

    pub(super) fn source(self) -> &'static str {
        match self {
            Lang::En => include_str!("../../locales/en/app.ftl"),
            Lang::Ar => include_str!("../../locales/ar/app.ftl"),
            Lang::Es => include_str!("../../locales/es/app.ftl"),
            Lang::Fr => include_str!("../../locales/fr/app.ftl"),
            Lang::De => include_str!("../../locales/de/app.ftl"),
            Lang::Pt => include_str!("../../locales/pt/app.ftl"),
            Lang::Hi => include_str!("../../locales/hi/app.ftl"),
            Lang::Zh => include_str!("../../locales/zh/app.ftl"),
        }
    }
}

/// Picks the language: `?lang=` in the address (testing, shared links), then the saved choice, then the browser's
/// preferred languages in order, then English.
pub fn choose(query: Option<&str>, saved: Option<&str>, browser: &[String]) -> Lang {
    query
        .and_then(Lang::parse)
        .or_else(|| saved.and_then(Lang::parse))
        .or_else(|| browser.iter().find_map(|t| Lang::parse(t)))
        .unwrap_or_default()
}

/// The `lang` value of a query string such as `?a=1&lang=ar`.
pub fn query_lang(search: &str) -> Option<&str> {
    search.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix("lang="))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tags() {
        assert_eq!(Lang::parse("pt-BR"), Some(Lang::Pt));
        assert_eq!(Lang::parse("AR"), Some(Lang::Ar));
        assert_eq!(Lang::parse("zh-Hans-CN"), Some(Lang::Zh));
        assert_eq!(Lang::parse("zh_CN"), Some(Lang::Zh));
        assert_eq!(Lang::parse("zh-TW"), None);
        assert_eq!(Lang::parse("zh-Hant"), None);
        assert_eq!(Lang::parse("nl"), None);
        assert_eq!(Lang::parse(""), None);
    }

    #[test]
    fn choice_order() {
        let browser = vec!["nl-NL".to_owned(), "de-DE".to_owned(), "en".to_owned()];
        assert_eq!(choose(Some("ar"), Some("fr"), &browser), Lang::Ar);
        assert_eq!(choose(Some("xx"), Some("fr"), &browser), Lang::Fr);
        assert_eq!(choose(None, None, &browser), Lang::De);
        assert_eq!(choose(None, None, &[]), Lang::En);
    }

    #[test]
    fn query_values() {
        assert_eq!(query_lang("?x=1&lang=hi"), Some("hi"));
        assert_eq!(query_lang(""), None);
    }

    #[test]
    fn only_arabic_is_rtl() {
        assert!(Lang::ALL.iter().all(|l| l.is_rtl() == (*l == Lang::Ar)));
        assert_eq!(Lang::Ar.dir(), "rtl");
        assert_eq!(Lang::Zh.tag(), "zh-Hans");
    }
}
