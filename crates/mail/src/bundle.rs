//! The wording of every language, compiled into the binary from `ftl/*.ftl`. A message missing in a language is read
//! from English; a message missing everywhere comes back as its id, so a gap is visible in the email, not blank.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::ftl;

/// Languages with wording (decision D11). English first: it is the fallback.
pub const LANGS: [&str; 8] = ["en", "ar", "es", "fr", "de", "pt", "hi", "zh"];

const SOURCES: [&str; 8] = [
    include_str!("../ftl/en.ftl"),
    include_str!("../ftl/ar.ftl"),
    include_str!("../ftl/es.ftl"),
    include_str!("../ftl/fr.ftl"),
    include_str!("../ftl/de.ftl"),
    include_str!("../ftl/pt.ftl"),
    include_str!("../ftl/hi.ftl"),
    include_str!("../ftl/zh.ftl"),
];

type Messages = HashMap<String, String>;

/// Parsed on first use. A file that fails to parse (caught by the tests) reads as empty and falls back to English.
fn messages(index: usize) -> &'static Messages {
    static CELLS: [OnceLock<Messages>; 8] = [const { OnceLock::new() }; 8];
    CELLS[index].get_or_init(|| SOURCES.get(index).and_then(|s| ftl::parse(s).ok()).unwrap_or_default())
}

/// The bundle language for a requested tag: `pt-BR` → `pt`, `zh-Hans` → `zh`, anything unknown → `en`.
pub fn pick_language(requested: &str) -> &'static str {
    let base = requested.trim().split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    LANGS.iter().copied().find(|l| *l == base).unwrap_or("en")
}

pub fn is_rtl(lang: &str) -> bool {
    matches!(lang, "ar" | "he" | "fa" | "ur")
}

/// Message `id` in `lang` with its variables filled in.
pub fn text(lang: &str, id: &str, vars: &[(&str, &str)]) -> String {
    let index = LANGS.iter().position(|l| *l == pick_language(lang)).unwrap_or(0);
    let raw = messages(index).get(id).or_else(|| messages(0).get(id)).map_or(id, String::as_str);
    ftl::fill(raw, vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_file_parses() {
        for (lang, src) in LANGS.iter().zip(SOURCES) {
            ftl::parse(src).unwrap_or_else(|e| panic!("{lang}.ftl: {e}"));
        }
    }

    /// Each language has every English message, nothing extra, and uses exactly the same variables.
    #[test]
    fn every_language_is_complete_and_uses_the_same_variables() {
        let en = ftl::parse(SOURCES[0]).unwrap();
        for (lang, src) in LANGS.iter().zip(SOURCES).skip(1) {
            let other = ftl::parse(src).unwrap();
            for (id, english) in &en {
                let translated = other.get(id).unwrap_or_else(|| panic!("{lang} lacks {id}"));
                let mut want = ftl::variables(english);
                let mut got = ftl::variables(translated);
                want.sort();
                got.sort();
                assert_eq!(want, got, "{lang} {id}");
            }
            for id in other.keys() {
                assert!(en.contains_key(id), "{lang} has {id}, which English lacks");
            }
        }
    }

    #[test]
    fn language_tags_map_to_bundles() {
        assert_eq!(pick_language("pt-BR"), "pt");
        assert_eq!(pick_language("zh_Hans"), "zh");
        assert_eq!(pick_language("AR"), "ar");
        assert_eq!(pick_language("xx"), "en");
        assert_eq!(pick_language(""), "en");
        assert!(is_rtl("ar") && !is_rtl("de"));
    }

    #[test]
    fn missing_messages_show_their_id() {
        assert_eq!(text("de", "no-such-message", &[]), "no-such-message");
        assert_eq!(text("de", "greeting", &[]), "Hallo,");
    }
}
