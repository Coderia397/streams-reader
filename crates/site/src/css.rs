//! The stylesheet: `style/site.scss` compiled with grass on top of the brand tokens, after a lint that keeps raw
//! colours and literal radii out of the site's SCSS (they belong in `brand/tokens.toml`).

use std::path::Path;

use crate::Error;

/// The two stylesheets: the landing page's and the inner pages'. Each page loads exactly one.
pub const ENTRIES: [&str; 2] = ["landing", "inner"];

/// Compile `style/<entry>.scss`. `tokens_dir` is `brand/dist/tokens` (holds `_tokens.scss`).
pub fn compile(style_dir: &Path, tokens_dir: &Path, entry: &str) -> Result<String, Error> {
    let violations = lint_dir(style_dir)?;
    if !violations.is_empty() {
        return Err(Error::Scss(format!("raw design values outside the token layer:\n{}", violations.join("\n"))));
    }
    let entry = style_dir.join(format!("{entry}.scss"));
    let options = grass::Options::default()
        .style(grass::OutputStyle::Compressed)
        .load_path(style_dir)
        .load_path(tokens_dir)
        .quiet(true);
    let css = grass::from_path(&entry, &options).map_err(|e| Error::Scss(e.to_string()))?;
    let css = squeeze(&shorten_properties(&drop_unused_properties(&strip_theme_overrides(&css))));
    // Last: no space is needed between an at-rule name and its condition (`@media(min-width:600px)`).
    let css = pack_media(&fold_calc(&physical_block_sides(&dedupe_light_theme(&css))));
    Ok(css.replace("@media (", "@media(").replace("@supports (", "@supports("))
}

/// The pages only use horizontal writing (Arabic is right to left, still horizontal), where `margin-block-start` is
/// `margin-top` and so on; the physical names are shorter. Inline-direction properties are left alone (RTL).
pub fn physical_block_sides(css: &str) -> String {
    let mut out = css.to_string();
    for (logical, physical) in [
        ("margin-block-start:", "margin-top:"),
        ("margin-block-end:", "margin-bottom:"),
        ("padding-block-start:", "padding-top:"),
        ("padding-block-end:", "padding-bottom:"),
    ] {
        for lead in ['{', ';'] {
            out = out.replace(&format!("{lead}{logical}"), &format!("{lead}{physical}"));
        }
    }
    out
}

/// The light theme block (`@media (prefers-color-scheme:light){:root:not([data-theme]){…}}`) repeats some values of
/// the dark defaults in `:root{…}` (brand blue, white on brand). Drop the repeats, shorten the selector to `:root`
/// (the site has no theme switch, so no element carries `data-theme`) and `#FFFFFF` to `#fff`.
pub fn dedupe_light_theme(css: &str) -> String {
    const LIGHT: &str = "@media (prefers-color-scheme:light){:root:not([data-theme]){";
    let css = css.replace("#FFFFFF", "#fff").replace("#ffffff", "#fff");
    let Some(at) = css.find(LIGHT) else { return css };
    let body_start = at + LIGHT.len();
    let Some(len) = css[body_start..].find('}') else { return css };
    let dark: std::collections::HashSet<&str> = css
        .match_indices(":root{")
        .filter(|(i, _)| *i < at)
        .filter_map(|(i, _)| css[i + 6..].split('}').next())
        .flat_map(|b| b.split(';'))
        .collect();
    let body: Vec<&str> = css[body_start..body_start + len].split(';').filter(|d| !dark.contains(d)).collect();
    format!("{}@media (prefers-color-scheme:light){{:root{{{}{}", &css[..at], body.join(";"), &css[body_start + len..])
}

/// Byte savings grass's compressed output leaves: spaces after commas and colons outside strings, leading zeros
/// (`0.5` → `.5`), spaces around `*` and `/`, and the long form of transparent black.
pub fn squeeze(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut quote: Option<char> = None;
    let mut prev = '\0';
    let mut chars = css.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            out.push(c);
            if c == q {
                quote = None;
            }
            prev = c;
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            ' ' if matches!(prev, ',' | ':') => continue,
            '0' if chars.peek() == Some(&'.') && !(prev.is_ascii_digit() || prev == '.') => {
                prev = '0';
                continue;
            }
            _ => {}
        }
        out.push(c);
        prev = c;
    }
    // `*` and `/` need no spaces around them (unlike `+` and `-` in `calc()`); no quoted value here contains them.
    out.replace("rgba(0,0,0,0)", "#0000").replace("transparent", "#0000").replace(" * ", "*").replace(" / ", "/")
}

/// Merge repeated top-level `@media (min-width:…)`, `@media (max-width:…)` and `@media (prefers-reduced-motion:…)`
/// blocks into one block each at the end of the sheet: max-width (phone-only) blocks, min-width blocks in ascending
/// order, then the motion blocks. The SCSS writes
/// breakpoints inside each rule (mobile first), so grass repeats the same prelude dozens of times. Moving a
/// breakpoint rule later is safe because it only ever overrides its own base rule; the motion blocks only add or
/// remove animations. Other at-rules (`@supports`, `@font-face`, other media) stay where they are.
pub fn pack_media(css: &str) -> String {
    let mut keep = String::with_capacity(css.len());
    let mut groups: Vec<(String, String)> = Vec::new();
    let bytes = css.as_bytes();
    let (mut i, mut start) = (0usize, 0usize);
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
        } else if b == b'"' || b == b'\'' {
            quote = Some(b);
        } else if b == b'{' {
            depth += 1;
        } else if b == b'}' {
            depth -= 1;
            if depth == 0 {
                let block = &css[start..=i];
                let packable = block.starts_with("@media (min-width:")
                    || block.starts_with("@media (max-width:")
                    || block.starts_with("@media (prefers-reduced-motion:");
                match (packable, block.find('{')) {
                    (true, Some(open)) => {
                        let (prelude, body) = (&block[..open], &block[open + 1..block.len() - 1]);
                        match groups.iter_mut().find(|(p, _)| p == prelude) {
                            Some((_, all)) => all.push_str(body),
                            None => groups.push((prelude.to_string(), body.to_string())),
                        }
                    }
                    _ => keep.push_str(block),
                }
                start = i + 1;
            }
        }
        i += 1;
    }
    keep.push_str(&css[start..]);
    // Phone-only (max-width) blocks first, then min-width blocks in ascending order, then the motion blocks.
    let width = |p: &str| -> u32 {
        if p.starts_with("@media (max-width:") {
            return 0;
        }
        p.strip_prefix("@media (min-width:")
            .and_then(|r| r.split("px").next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(u32::MAX)
    };
    groups.sort_by_key(|(p, _)| width(p));
    for (prelude, body) in groups {
        keep.push_str(&prelude);
        keep.push('{');
        keep.push_str(&body);
        keep.push('}');
    }
    keep
}

/// Fold `calc(<number><unit>*<number>)` (what `calc(t.space(12) * 2)` compiles to once the scales are inlined) and
/// `calc(<a><unit> + <b><unit>)` with one unit into one length: `calc(48px*-1.5)` → `-72px`, `calc(48px + 8px)` →
/// `56px`. Anything else inside `calc()` is left alone.
pub fn fold_calc(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(i) = rest.find("calc(") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 5..];
        match after
            .find(')')
            .and_then(|end| fold_product(&after[..end]).or_else(|| fold_sum(&after[..end])).map(|v| (end, v)))
        {
            Some((end, v)) => {
                out.push_str(&v);
                rest = &after[end + 1..];
            }
            None => {
                out.push_str("calc(");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `48px` → `(48.0, "px")`; `None` unless it is a plain number with a px, rem or em unit.
fn length(text: &str) -> Option<(f64, &str)> {
    let unit_at = text.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = text.split_at(unit_at);
    matches!(unit, "px" | "rem" | "em").then_some(())?;
    Some((num.parse::<f64>().ok()?, unit))
}

/// `48px*-1.5` → `-72px`; `None` unless the expression is exactly a length times a plain number.
fn fold_product(expr: &str) -> Option<String> {
    let (a, b) = expr.split_once('*')?;
    let (num, unit) = length(a)?;
    Some(format_length(num * b.parse::<f64>().ok()?, unit))
}

/// `48px + 8px` → `56px`, `32px - 8px` → `24px`; `None` unless both sides are lengths in the same unit.
fn fold_sum(expr: &str) -> Option<String> {
    let (a, op, b) = expr
        .split_once(" + ")
        .map(|(a, b)| (a, 1.0, b))
        .or_else(|| expr.split_once(" - ").map(|(a, b)| (a, -1.0, b)))?;
    let ((x, unit), (y, unit_b)) = (length(a)?, length(b)?);
    (unit == unit_b).then(|| format_length(x + op * y, unit))
}

fn format_length(value: f64, unit: &str) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    let text = format!("{rounded}");
    let text = text.strip_prefix("0.").map_or_else(|| text.replace("-0.", "-."), |t| format!(".{t}"));
    format!("{text}{unit}")
}

/// Give every custom property a short name (`--a`, `--b`, … by frequency). Each stylesheet is self-contained and
/// nothing outside it (HTML, script) reads these properties, so the rename is safe; it saves about a tenth of the
/// file. Only `--name` tokens that start a declaration or sit inside `var(…)` are renamed, never BEM class names
/// such as `.btn--primary`.
pub fn shorten_properties(css: &str) -> String {
    let spans = property_spans(css);
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (a, b) in &spans {
        *counts.entry(&css[*a..*b]).or_default() += 1;
    }
    let mut names: Vec<(&str, usize)> = counts.into_iter().collect();
    names.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(y.0)));
    let short: std::collections::HashMap<&str, String> =
        names.iter().enumerate().map(|(i, (n, _))| (*n, format!("--{}", short_id(i)))).collect();
    let mut out = String::with_capacity(css.len());
    let mut at = 0;
    for (a, b) in spans {
        out.push_str(&css[at..a]);
        out.push_str(&short[&css[a..b]]);
        at = b;
    }
    out.push_str(&css[at..]);
    out
}

fn short_id(mut i: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut s = String::new();
    loop {
        s.push(ALPHABET[i % ALPHABET.len()] as char);
        i /= ALPHABET.len();
        if i == 0 {
            return s;
        }
        i -= 1;
    }
}

/// Byte ranges of custom property names: after `(`, `{`, `;`, `,`, `:` or whitespace, starting with `--`.
fn property_spans(css: &str) -> Vec<(usize, usize)> {
    let bytes = css.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = css[i..].find("--") {
        let at = i + off;
        let ok_before = at == 0 || matches!(bytes[at - 1], b'(' | b'{' | b';' | b',' | b':' | b' ' | b'\n');
        let end = css[at + 2..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .map_or(css.len(), |e| at + 2 + e);
        if ok_before && end > at + 2 {
            out.push((at, end));
        }
        i = end.max(at + 2);
    }
    out
}

/// Remove custom property declarations (`--name: …`) that nothing reads with `var(--name)`. The token layer
/// declares every token; the site inlines the theme-independent scales, so most of those are unused. Repeats until
/// stable, because an unused declaration may be the only reader of another.
pub fn drop_unused_properties(css: &str) -> String {
    let mut css = css.to_string();
    loop {
        let decls = declarations(&css);
        let unused: Vec<(usize, usize)> = decls
            .iter()
            .filter(|(name, _, _)| !css.contains(&format!("var({name})")) && !css.contains(&format!("var({name},")))
            .map(|(_, a, b)| (*a, *b))
            .collect();
        if unused.is_empty() {
            return css;
        }
        for (a, b) in unused.into_iter().rev() {
            css.replace_range(a..b, "");
        }
        css = css.replace(";}", "}").replace("{;", "{");
    }
}

/// `(name, start, end)` of each custom property declaration, `end` past its `;` (or at its closing `}`).
fn declarations(css: &str) -> Vec<(String, usize, usize)> {
    let bytes = css.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = css[i..].find("--") {
        let at = i + off;
        let prev = css[..at].trim_end().chars().last();
        let name_end = css[at..].find(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).map_or(css.len(), |e| at + e);
        let is_decl = matches!(prev, Some('{' | ';')) && css[name_end..].trim_start().starts_with(':');
        if !is_decl {
            i = name_end.max(at + 2);
            continue;
        }
        let mut end = name_end;
        let mut depth = 0i32;
        while end < bytes.len() {
            match bytes[end] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                b';' if depth == 0 => {
                    end += 1;
                    break;
                }
                b'}' if depth == 0 => break,
                _ => {}
            }
            end += 1;
        }
        out.push((css[at..name_end].to_string(), at, end));
        i = end;
    }
    out
}

/// The token layer also emits `:root[data-theme="…"]` blocks for a manual theme switch. The website has no switch
/// (it follows `prefers-color-scheme`), so those blocks are dead weight on every page; drop them.
pub fn strip_theme_overrides(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(i) = rest.find(":root[data-theme=") {
        // Only whole top-level rules: the selector starts the rule (after `}` or at the start).
        let starts_rule = rest[..i].trim_end().ends_with('}') || rest[..i].trim().is_empty();
        let Some(close) = rest[i..].find('}') else { break };
        let block = &rest[i..i + close];
        if !starts_rule || block.contains('{') && block.matches('{').count() > 1 {
            out.push_str(&rest[..i + 1]);
            rest = &rest[i + 1..];
            continue;
        }
        out.push_str(&rest[..i]);
        rest = &rest[i + close + 1..];
    }
    out.push_str(rest);
    out
}

/// Lint every `.scss` file under `dir`; returns "file:line: problem" strings.
pub fn lint_dir(dir: &Path) -> Result<Vec<String>, Error> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))?;
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            out.extend(lint_dir(&path)?);
        } else if path.extension().is_some_and(|e| e == "scss") {
            let src = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            out.extend(lint(&src).into_iter().map(|(line, msg)| format!("{name}:{line}: {msg}")));
        }
    }
    Ok(out)
}

/// Problems in one SCSS source: hex colours, rgb()/hsl() literals and literal border radii.
pub fn lint(src: &str) -> Vec<(usize, String)> {
    let mut problems = Vec::new();
    for (i, raw) in strip_comments(src).lines().enumerate() {
        let line = raw.trim();
        if let Some(hex) = find_hex_colour(line) {
            problems.push((i + 1, format!("hex colour {hex}; use a colour token")));
        }
        for f in ["rgb(", "rgba(", "hsl(", "hsla(", "oklch(", "lab("] {
            if line.contains(f) {
                problems.push((i + 1, format!("{f}…) literal; use a colour token")));
            }
        }
        if let Some(value) = line.strip_prefix("border-radius:") {
            let v = value.trim().trim_end_matches(';');
            let ok = v.split_whitespace().all(|part| {
                part.starts_with("var(") || part.starts_with("t.") || part == "0" || part == "50%" || part == "inherit"
            });
            if !ok {
                problems.push((i + 1, format!("literal radius `{v}`; use t.radius() or a component token")));
            }
        }
    }
    problems
}

/// `#` followed by 3, 4, 6 or 8 hex digits and not by another word character.
fn find_hex_colour(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' && !(i + 1 < bytes.len() && bytes[i + 1] == b'{') {
            let digits = bytes[i + 1..].iter().take_while(|b| b.is_ascii_hexdigit()).count();
            let next = bytes.get(i + 1 + digits);
            let word_follows = next.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-');
            if matches!(digits, 3 | 4 | 6 | 8) && !word_follows {
                return Some(line[i..i + 1 + digits].to_string());
            }
            i += 1 + digits;
        } else {
            i += 1;
        }
    }
    None
}

/// Remove `//` line comments and `/* */` block comments, keeping line breaks so line numbers stay right.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_block = false;
    let mut in_string: Option<char> = None;
    while let Some(c) = chars.next() {
        if in_block {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
            } else if c == '\n' {
                out.push('\n');
            }
            continue;
        }
        if let Some(q) = in_string {
            out.push(c);
            if c == q {
                in_string = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                in_string = Some(c);
                out.push(c);
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                in_block = true;
            }
            '/' if chars.peek() == Some(&'/') => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_hex_rgb_and_literal_radius() {
        let src = "a { color: #2F6BFF; }\nb { background: rgba(0,0,0,.5); }\nc {\n  border-radius: 12px;\n}\n";
        let lines: Vec<usize> = lint(src).into_iter().map(|(l, _)| l).collect();
        assert_eq!(lines, vec![1, 2, 4]);
    }

    #[test]
    fn allows_tokens_ids_interpolation_and_comments() {
        let src = "// #fff in a comment\n/* #000 */\n.a { color: t.color(text); border-radius: var(--radius-md); }\n\
                   .b-#{$x} { border-radius: t.radius(lg) t.radius(lg) 0 0; }\n.c { content: \"#\"; }\n#main { x: 1 }\n";
        assert!(lint(src).is_empty(), "{:?}", lint(src));
    }

    #[test]
    fn squeeze_keeps_strings_and_numbers() {
        assert_eq!(
            squeeze("a{margin:0 10.5px;font: 0.5rem \"A, B\";color:rgba(0,0,0,0)}@media (min-width: 9px){}"),
            "a{margin:0 10.5px;font:.5rem \"A, B\";color:#0000}@media (min-width:9px){}"
        );
    }

    #[test]
    fn media_blocks_merge_in_breakpoint_order() {
        let css = "a{x:1}@media (min-width:900px){a{y:2}}b{x:1}@media (min-width:600px){b{y:1}}\
                   @media (min-width:900px){b{y:3}}@media print{c{d:0}}@font-face{src:url(\"a{b}.woff2\")}";
        assert_eq!(
            pack_media(css),
            "a{x:1}b{x:1}@media print{c{d:0}}@font-face{src:url(\"a{b}.woff2\")}\
             @media (min-width:600px){b{y:1}}@media (min-width:900px){a{y:2}b{y:3}}"
        );
    }

    #[test]
    fn light_theme_drops_values_equal_to_the_dark_defaults() {
        let css = ":root{color-scheme:dark;--a:#FFFFFF;--b:#111}@media (prefers-color-scheme:light){:root:not([data-theme]){color-scheme:light;--a:#FFFFFF;--b:#eee}}a{b:c}";
        assert_eq!(
            dedupe_light_theme(css),
            ":root{color-scheme:dark;--a:#fff;--b:#111}@media (prefers-color-scheme:light){:root{color-scheme:light;--b:#eee}}a{b:c}"
        );
        assert_eq!(
            physical_block_sides("a{margin-block-start:1px;scroll-margin-block-start:2px}"),
            "a{margin-top:1px;scroll-margin-block-start:2px}"
        );
    }

    #[test]
    fn calc_products_fold_into_lengths() {
        assert_eq!(
            fold_calc("a{margin:calc(48px*-1.5) calc(20px*1.4);top:calc(100% - 4px);w:calc(4px*.5)}"),
            "a{margin:-72px 28px;top:calc(100% - 4px);w:2px}"
        );
        assert_eq!(fold_calc("a{b:calc(var(--x)*2);c:calc(1rem*0.25)}"), "a{b:calc(var(--x)*2);c:.25rem}");
        assert_eq!(
            fold_calc("a{w:calc(48px + 8px);h:calc(32px - 8px);i:calc(1rem + 4px);j:calc(100% - 4px)}"),
            "a{w:56px;h:24px;i:calc(1rem + 4px);j:calc(100% - 4px)}"
        );
    }

    #[test]
    fn property_names_shrink_but_class_names_do_not() {
        let css = ":root{--color-text: red;--space-4: 1px}.btn--primary{color:var(--color-text);margin:var(--space-4)}.x{color:var(--color-text)}";
        assert_eq!(
            shorten_properties(css),
            ":root{--a: red;--b: 1px}.btn--primary{color:var(--a);margin:var(--b)}.x{color:var(--a)}"
        );
        assert_eq!(short_id(0), "a");
        assert_eq!(short_id(52), "aa");
    }

    #[test]
    fn unused_custom_properties_are_dropped() {
        let css = ":root{--a: 1px;--b: var(--c);--c: 2px;--d: calc(var(--a) + 1px)}.x{width:var(--d)}";
        assert_eq!(drop_unused_properties(css), ":root{--a: 1px;--d: calc(var(--a) + 1px)}.x{width:var(--d)}");
    }

    #[test]
    fn theme_override_blocks_are_removed() {
        let css = ":root{--a:1}:root[data-theme=ink]{--a:2}:root[data-theme=paper]{--a:3}.x{color:red}";
        assert_eq!(strip_theme_overrides(css), ":root{--a:1}.x{color:red}");
    }

    #[test]
    fn the_site_stylesheet_is_clean() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("style");
        assert_eq!(lint_dir(&dir).unwrap(), Vec::<String>::new());
    }
}
