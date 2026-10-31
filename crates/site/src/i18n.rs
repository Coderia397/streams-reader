//! Translated strings from `locales/<lang>/site.ftl` (Fluent). Lookups never fail at the call site: a missing
//! message is recorded and reported once the build finishes, and the build then fails.

use std::cell::RefCell;
use std::path::Path;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

use crate::Error;
use crate::config::Lang;
use crate::html::esc;

pub struct Catalog {
    pub lang: Lang,
    bundle: FluentBundle<FluentResource>,
    keys: Vec<String>,
    missing: RefCell<Vec<String>>,
}

impl Catalog {
    /// Load `<locales>/<code>/site.ftl`.
    pub fn load(locales: &Path, lang: Lang) -> Result<Self, Error> {
        let path = locales.join(lang.code).join("site.ftl");
        let src = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        Self::from_source(lang, src)
    }

    pub fn from_source(lang: Lang, src: String) -> Result<Self, Error> {
        let resource = FluentResource::try_new(src).map_err(|(_, errs)| Error::Fluent {
            lang: lang.code,
            detail: errs.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>().join("; "),
        })?;
        let keys = resource
            .entries()
            .filter_map(|e| match e {
                fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_string()),
                _ => None,
            })
            .collect();
        let langid: LanguageIdentifier =
            lang.code.parse().map_err(|_| Error::Fluent { lang: lang.code, detail: "bad language tag".into() })?;
        let mut bundle = FluentBundle::new(vec![langid]);
        bundle.set_use_isolating(false);
        bundle.add_resource(resource).map_err(|errs| Error::Fluent {
            lang: lang.code,
            detail: errs.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>().join("; "),
        })?;
        Ok(Self { lang, bundle, keys, missing: RefCell::new(Vec::new()) })
    }

    /// Message ids defined in this language, in file order.
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// Messages asked for but not defined.
    pub fn missing(&self) -> Vec<String> {
        self.missing.borrow().clone()
    }

    fn format(&self, key: &str, args: Option<&FluentArgs>) -> String {
        let Some(pattern) = self.bundle.get_message(key).and_then(|m| m.value()) else {
            self.missing.borrow_mut().push(key.to_string());
            return key.to_string();
        };
        let mut errors = Vec::new();
        let out = self.bundle.format_pattern(pattern, args, &mut errors).into_owned();
        if !errors.is_empty() {
            self.missing.borrow_mut().push(format!("{key} ({errors:?})"));
        }
        out
    }

    /// The plain, unescaped text (for JSON, XML and plain-text files that do their own escaping).
    pub fn text(&self, key: &str) -> String {
        self.format(key, None)
    }

    /// The plain, unescaped text with plain-text arguments.
    pub fn text_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut fa = FluentArgs::new();
        for (k, v) in args {
            fa.set(*k, FluentValue::from(*v));
        }
        self.format(key, Some(&fa))
    }

    /// Escaped for HTML.
    pub fn t(&self, key: &str) -> String {
        esc(&self.format(key, None))
    }

    /// Escaped for HTML, with plain-text arguments.
    pub fn t_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        esc(&self.text_args(key, args))
    }

    /// Escaped for HTML, with arguments that are already HTML (links, `<strong>`): each argument is passed to Fluent
    /// as a private-use marker, the result is escaped, then the markers are swapped for the HTML.
    pub fn t_html(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut fa = FluentArgs::new();
        let markers: Vec<String> = (0..args.len()).map(|i| format!("\u{E000}{i}\u{E001}")).collect();
        for ((k, _), m) in args.iter().zip(&markers) {
            fa.set(*k, FluentValue::from(m.as_str()));
        }
        let mut out = esc(&self.format(key, Some(&fa)));
        for ((_, html), m) in args.iter().zip(&markers) {
            out = out.replace(m.as_str(), html);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_LANG;

    fn cat(src: &str) -> Catalog {
        Catalog::from_source(DEFAULT_LANG, src.to_string()).unwrap()
    }

    #[test]
    fn escapes_text_but_keeps_html_arguments() {
        let c = cat("hi = Read <this> { $link }\n");
        assert_eq!(c.t_html("hi", &[("link", "<a href=\"/x\">x</a>")]), "Read &lt;this&gt; <a href=\"/x\">x</a>");
    }

    #[test]
    fn plain_arguments_are_escaped_and_not_isolated() {
        let c = cat("v = Version { $v }\n");
        assert_eq!(c.t_args("v", &[("v", "<1>")]), "Version &lt;1&gt;");
    }

    #[test]
    fn missing_keys_are_recorded() {
        let c = cat("a = A\n");
        assert_eq!(c.t("nope"), "nope");
        assert_eq!(c.missing(), vec!["nope".to_string()]);
    }

    #[test]
    fn lists_keys_in_order() {
        let c = cat("b = B\na = A\n-term = T\n");
        assert_eq!(c.keys(), ["b", "a"]);
    }
}
