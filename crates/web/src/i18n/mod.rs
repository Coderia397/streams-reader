//! Interface text through Fluent. Every language is compiled in (a few kB each), so switching needs no network and
//! works offline. Lookups fall back to English, then to the key, so a missing translation never shows an empty
//! string. Use the [`t!`](crate::t) macro: `t!("library-title")`, `t!("time-left-min", minutes = 12)`.

pub mod lang;
mod number;
#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};

pub use lang::Lang;

use crate::util::dom;

/// localStorage key of the chosen language (absent: follow the browser).
pub const PREF_KEY: &str = "nolabs.lang";

type Bundle = FluentBundle<FluentResource>;

thread_local! {
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::En) };
    static BUNDLES: RefCell<Vec<(Lang, Bundle)>> = const { RefCell::new(Vec::new()) };
}

fn build(lang: Lang) -> Option<Bundle> {
    // A file with a syntax error still yields its valid messages; the key test catches the error in CI.
    let res = FluentResource::try_new(lang.source().to_owned()).unwrap_or_else(|(res, _)| res);
    let id = lang.tag().parse().ok()?;
    let mut bundle = FluentBundle::new(vec![id]);
    // Unicode isolation marks around arguments would show as stray glyphs in some fonts; the UI never mixes
    // directions inside one message except for user titles, which are wrapped in their own `<bdi>`.
    bundle.set_use_isolating(false);
    // NUMBER() for decimals and sizes, written with the language's own decimal separator (see `number`).
    bundle.add_builtins().ok()?;
    bundle.set_formatter(Some(number::formatter(lang.decimal_separator())));
    bundle.add_resource(res).ok()?;
    Some(bundle)
}

fn format_in(lang: Lang, key: &str, args: Option<&FluentArgs>) -> Option<String> {
    BUNDLES.with(|cell| {
        let mut bundles = cell.borrow_mut();
        if !bundles.iter().any(|(l, _)| *l == lang) {
            let bundle = build(lang)?;
            bundles.push((lang, bundle));
        }
        let (_, bundle) = bundles.iter().find(|(l, _)| *l == lang)?;
        let pattern = bundle.get_message(key)?.value()?;
        let mut errors = Vec::new();
        Some(bundle.format_pattern(pattern, args, &mut errors).into_owned())
    })
}

/// The language in use.
pub fn current() -> Lang {
    CURRENT.with(Cell::get)
}

/// Runs `f` with `lang` as the current language, then restores the previous one (tests of formatted text).
#[cfg(test)]
pub fn with_lang<T>(lang: Lang, f: impl FnOnce() -> T) -> T {
    let before = CURRENT.with(|c| c.replace(lang));
    let out = f();
    CURRENT.with(|c| c.set(before));
    out
}

/// Formats `key` with named arguments in the current language.
pub fn tr(key: &str, args: &[(&str, FluentValue<'static>)]) -> String {
    let fa = (!args.is_empty()).then(|| {
        let mut fa = FluentArgs::new();
        for (k, v) in args {
            fa.set(*k, v.clone());
        }
        fa
    });
    let lang = current();
    format_in(lang, key, fa.as_ref())
        .or_else(|| (lang != Lang::En).then(|| format_in(Lang::En, key, fa.as_ref())).flatten())
        .unwrap_or_else(|| key.to_owned())
}

/// Turns a value into a Fluent argument (numbers keep their plural category; strings are passed as they are).
pub fn arg(value: impl Into<FluentValue<'static>>) -> FluentValue<'static> {
    value.into()
}

/// Looks up interface text: `t!("key")` or `t!("key", name = value, count = 3)`.
#[macro_export]
macro_rules! t {
    ($key:literal) => {
        $crate::i18n::tr($key, &[])
    };
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::tr($key, &[$((stringify!($name), $crate::i18n::arg($value))),+])
    };
}

fn saved() -> Option<String> {
    dom::local_storage()?.get_item(PREF_KEY).ok().flatten()
}

fn browser_languages() -> Vec<String> {
    let Some(window) = dom::window() else { return Vec::new() };
    let nav = window.navigator();
    let mut tags: Vec<String> = nav.languages().iter().filter_map(|v| v.as_string()).collect();
    if tags.is_empty() {
        tags.extend(nav.language());
    }
    tags
}

/// Chooses the language and sets `lang`/`dir` on `<html>`. Call once before mounting.
pub fn init() {
    let search = dom::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
    let lang = lang::choose(lang::query_lang(&search), saved().as_deref(), &browser_languages());
    CURRENT.with(|c| c.set(lang));
    if let Some(root) = dom::html_element() {
        let _ = root.set_attribute("lang", lang.tag());
        let _ = root.set_attribute("dir", lang.dir());
    }
}

/// The language the browser asks for (what "Automatic" resolves to).
pub fn browser_lang() -> Lang {
    lang::choose(None, None, &browser_languages())
}

/// Whether the language follows the browser (nothing saved).
pub fn follows_browser() -> bool {
    saved().is_none()
}

/// Saves a choice (`None`: follow the browser) and reloads, so every screen and cached string is rebuilt in it.
pub fn choose(lang: Option<Lang>) {
    if let Some(storage) = dom::local_storage() {
        let _ = match lang {
            Some(l) => storage.set_item(PREF_KEY, l.code()),
            None => storage.remove_item(PREF_KEY),
        };
    }
    dom::reload();
}
