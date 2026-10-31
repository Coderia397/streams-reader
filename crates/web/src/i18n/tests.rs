//! Translation checks: every file parses, every language has exactly English's keys, the placeholders of each message
//! match English's, and every key the source code asks for exists.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use fluent_syntax::parser;

use super::{Lang, tr};

type Vars = BTreeSet<String>;

fn inline_vars(e: &InlineExpression<&str>, out: &mut Vars) {
    match e {
        InlineExpression::VariableReference { id } => {
            out.insert(id.name.to_owned());
        }
        InlineExpression::Placeable { expression } => expr_vars(expression, out),
        InlineExpression::FunctionReference { arguments, .. } => {
            for a in &arguments.positional {
                inline_vars(a, out);
            }
        }
        _ => {}
    }
}

fn expr_vars(e: &Expression<&str>, out: &mut Vars) {
    match e {
        Expression::Inline(i) => inline_vars(i, out),
        Expression::Select { selector, variants } => {
            inline_vars(selector, out);
            for v in variants {
                pattern_vars(&v.value, out);
            }
        }
    }
}

fn pattern_vars(p: &Pattern<&str>, out: &mut Vars) {
    for el in &p.elements {
        if let PatternElement::Placeable { expression } = el {
            expr_vars(expression, out);
        }
    }
}

/// Message id → variables it uses.
fn messages(lang: Lang) -> BTreeMap<String, Vars> {
    let source = lang.source();
    let res = match parser::parse(source) {
        Ok(r) => r,
        Err((_, errors)) => panic!("{}: {} syntax errors, first: {:?}", lang.code(), errors.len(), errors.first()),
    };
    let mut out = BTreeMap::new();
    for entry in &res.body {
        if let Entry::Message(m) = entry {
            let mut vars = Vars::new();
            if let Some(v) = &m.value {
                pattern_vars(v, &mut vars);
            }
            assert!(out.insert(m.id.name.to_owned(), vars).is_none(), "{}: duplicate key {}", lang.code(), m.id.name);
        }
    }
    out
}

#[test]
fn every_language_has_every_key_with_the_same_placeholders() {
    let en = messages(Lang::En);
    assert!(en.len() > 100, "English has only {} messages", en.len());
    for lang in Lang::ALL.into_iter().filter(|l| *l != Lang::En) {
        let other = messages(lang);
        let missing: Vec<&String> = en.keys().filter(|k| !other.contains_key(*k)).collect();
        let extra: Vec<&String> = other.keys().filter(|k| !en.contains_key(*k)).collect();
        assert!(missing.is_empty(), "{} is missing {missing:?}", lang.code());
        assert!(extra.is_empty(), "{} has keys English lacks: {extra:?}", lang.code());
        for (key, vars) in &en {
            assert_eq!(Some(vars), other.get(key), "{}: placeholders differ in {key}", lang.code());
        }
    }
}

/// Every `t!("…")` in the source names an English message.
#[test]
fn source_keys_exist() {
    let en = messages(Lang::En);
    let mut missing = Vec::new();
    let mut stack = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            for (i, _) in text.match_indices("t!(\"") {
                // `format!(`, `assert!(` and friends end in `t!(` too.
                if text[..i].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let rest = &text[i + 4..];
                let Some(end) = rest.find('"') else { continue };
                let key = &rest[..end];
                // `t!("…")` inside this file's own docs and tests is not a real lookup.
                if !en.contains_key(key) && !key.contains('…') && key != "key" {
                    missing.push(format!("{}: {key}", path.display()));
                }
            }
        }
    }
    assert!(missing.is_empty(), "keys used in code but missing from en: {missing:#?}");
}

#[test]
fn lookups_fall_back_to_the_key_and_format_arguments() {
    assert_eq!(tr("no-such-key", &[]), "no-such-key");
    assert_eq!(crate::t!("app-name"), "Streams Reader");
    assert_eq!(crate::t!("time-left-min", minutes = 12), "12 min left");
}

#[test]
fn counts_of_runs_and_seconds_take_plural_forms() {
    use super::with_lang;
    let en = |runs: u32, left: u32| {
        with_lang(Lang::En, || {
            [
                crate::t!("voices-budget-numbers", left = 42, cap = 60, runs = runs),
                crate::t!("voice-budget-left", minutes = 1, runs = runs),
                crate::t!("voice-ask-left", left = left, runs = runs),
                crate::t!("engine-gpu-left", left = 42, cap = 60, runs = runs),
            ]
        })
    };
    assert_eq!(en(1, 1)[0], "42 of 60 s · 1 run");
    assert_eq!(en(1, 1)[1], "About 1 min of expressive audio and 1 run left today.");
    assert_eq!(en(1, 1)[2], "1 second and 1 run left today.");
    assert_eq!(en(2, 2)[2], "2 seconds and 2 runs left today.");
    assert_eq!(en(2, 2)[3], "42 of 60 s · 2 runs");

    let ar = |runs: u32| {
        with_lang(Lang::Ar, || {
            [
                crate::t!("voices-budget-numbers", left = 42, cap = 60, runs = runs),
                crate::t!("voice-ask-left", left = runs, runs = runs),
            ]
        })
    };
    assert!(ar(1)[0].ends_with("مرة واحدة"), "{}", ar(1)[0]);
    assert!(ar(2)[0].ends_with("مرتان"), "{}", ar(2)[0]);
    assert!(ar(3)[0].ends_with("مرات"), "{}", ar(3)[0]);
    assert!(ar(2)[1].contains("ثانيتان") && ar(2)[1].contains("مرتان"), "{}", ar(2)[1]);
    assert!(ar(1)[1].contains("ثانية واحدة"), "{}", ar(1)[1]);
}
