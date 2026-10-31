//! The stylesheet: `style/site.scss` compiled with grass on top of the brand tokens, after a lint that keeps raw
//! colours and literal radii out of the site's SCSS (they belong in `brand/tokens.toml`).

use std::path::Path;

use crate::Error;

/// Compile `style/site.scss`. `tokens_dir` is `brand/dist/tokens` (holds `_tokens.scss`).
pub fn compile(style_dir: &Path, tokens_dir: &Path) -> Result<String, Error> {
    let violations = lint_dir(style_dir)?;
    if !violations.is_empty() {
        return Err(Error::Scss(format!("raw design values outside the token layer:\n{}", violations.join("\n"))));
    }
    let entry = style_dir.join("site.scss");
    let options = grass::Options::default()
        .style(grass::OutputStyle::Compressed)
        .load_path(style_dir)
        .load_path(tokens_dir)
        .quiet(true);
    grass::from_path(&entry, &options).map_err(|e| Error::Scss(e.to_string()))
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
    fn the_site_stylesheet_is_clean() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("style");
        assert_eq!(lint_dir(&dir).unwrap(), Vec::<String>::new());
    }
}
