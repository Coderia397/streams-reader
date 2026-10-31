//! Static files copied into `dist/`: fonts (subsets of the web crate's), the demo clips, the site script, social
//! card images, and whatever the brand build has produced (`brand/dist`, minus the token sources).

use std::path::{Path, PathBuf};

use crate::Error;

/// Copy one file, creating parent folders.
pub fn copy(from: &Path, to: &Path) -> Result<u64, Error> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::copy(from, to).map_err(|e| Error::io(from, e))
}

/// Write a file, creating parent folders.
pub fn write(to: &Path, contents: impl AsRef<[u8]>) -> Result<(), Error> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::write(to, contents).map_err(|e| Error::io(to, e))
}

/// Every file under `dir`, as paths relative to it, sorted.
pub fn walk(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut out = Vec::new();
    walk_into(dir, Path::new(""), &mut out)?;
    out.sort();
    Ok(out)
}

fn walk_into(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> Result<(), Error> {
    let dir = root.join(rel);
    for entry in std::fs::read_dir(&dir).map_err(|e| Error::io(&dir, e))? {
        let entry = entry.map_err(|e| Error::io(&dir, e))?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let rel_path = rel.join(&name);
        if entry.path().is_dir() {
            walk_into(root, &rel_path, out)?;
        } else {
            out.push(rel_path);
        }
    }
    Ok(())
}

/// Copy a whole folder; returns the relative paths copied.
pub fn copy_dir(from: &Path, to: &Path, skip: impl Fn(&Path) -> bool) -> Result<Vec<PathBuf>, Error> {
    let files: Vec<PathBuf> = walk(from)?.into_iter().filter(|p| !skip(p)).collect();
    for rel in &files {
        copy(&from.join(rel), &to.join(rel))?;
    }
    Ok(files)
}

/// Fonts the site uses: subsets of the web crate's fonts made by `tools/subset_fonts.py` into `assets/fonts`
/// (upright: Latin-1 and punctuation, weights 400-700; italic: only the characters the site sets in italic).
pub const FONTS: [&str; 5] = [
    "inter-latin-wght-normal.woff2",
    "literata-latin-wght-normal.woff2",
    "literata-latin-wght-italic.woff2",
    "OFL-Inter.txt",
    "OFL-Literata.txt",
];

/// The brand build's output the site uses. Everything is optional: the site still builds (with a fallback mark and
/// favicon) before the brand build has run.
pub struct Brand {
    /// `brand/dist/web`: favicons, touch icon, social card.
    pub web_dir: Option<PathBuf>,
    /// Folder holding the spot illustrations as SVG (`brand/dist/spot`, else `brand/spot`).
    pub spot_dir: Option<PathBuf>,
    /// The mark drawn in `currentColor`, for inlining in the top bar and footer.
    pub mark_svg: Option<PathBuf>,
}

impl Brand {
    /// Look up the brand output under `brand_root` (the repository's `brand/` folder).
    pub fn find(brand_root: &Path) -> Self {
        let dist = brand_root.join("dist");
        let has_svg = |d: &Path| walk(d).is_ok_and(|f| f.iter().any(|p| p.extension().is_some_and(|e| e == "svg")));
        let first_dir = |cands: [PathBuf; 2]| cands.into_iter().find(|d| d.is_dir() && has_svg(d));
        let first_file = |cands: [PathBuf; 2]| cands.into_iter().find(|f| f.is_file());
        let mark = "streams-reader-mark-currentcolor.svg";
        Self {
            web_dir: Some(dist.join("web")).filter(|d| d.is_dir()),
            spot_dir: first_dir([dist.join("spot"), brand_root.join("spot")]),
            mark_svg: first_file([dist.join("mark").join(mark), brand_root.join("mark").join(mark)]),
        }
    }

    /// Names (file stems) of the spot illustrations available as SVG.
    pub fn spots(&self) -> Vec<String> {
        let Some(dir) = &self.spot_dir else { return Vec::new() };
        walk(dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.parent() == Some(Path::new("")) && p.extension().is_some_and(|e| e == "svg"))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect()
    }
}

/// The built-in mark (note 12, concept 1: a reading cursor whose stem is a sound wave, with the Ember dot), used
/// until the brand build ships its own. Colours come from CSS (`currentColor` and the `.mark-dot` class).
pub const FALLBACK_MARK: &str = "<svg class=\"mark\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\" \
     width=\"24\" height=\"24\"><path d=\"M8 4h8M8 20h8\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2.25\" \
     stroke-linecap=\"round\"/><path d=\"M12 6.5c2.6 1.8 2.6 3.7 0 5.5s-2.6 3.7 0 5.5\" fill=\"none\" stroke=\"currentColor\" \
     stroke-width=\"2.25\" stroke-linecap=\"round\"/><circle class=\"mark-dot\" cx=\"12\" cy=\"12\" r=\"1.9\"/></svg>";

/// A standalone favicon from the fallback mark, coloured with token values (brand tile, white mark, ember dot).
pub fn fallback_favicon(brand: &str, on_brand: &str, ember: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 32 32\"><rect width=\"32\" height=\"32\" rx=\"9\" fill=\"{brand}\"/>\
         <g transform=\"translate(4 4)\"><path d=\"M8 4h8M8 20h8\" fill=\"none\" stroke=\"{on_brand}\" stroke-width=\"2.5\" stroke-linecap=\"round\"/>\
         <path d=\"M12 6.5c2.6 1.8 2.6 3.7 0 5.5s-2.6 3.7 0 5.5\" fill=\"none\" stroke=\"{on_brand}\" stroke-width=\"2.5\" stroke-linecap=\"round\"/>\
         <circle cx=\"12\" cy=\"12\" r=\"2.1\" fill=\"{ember}\"/></g></svg>\n"
    )
}

/// Make an SVG file safe and styleable for inlining: keep the root `<svg>`, drop the prolog, comments, `<title>`
/// (the link around it carries the name) and `style` attributes (blocked by the CSP), and turn ids into
/// `mark-<id>` classes (the mark appears twice per page, so ids would clash).
pub fn inline_svg(src: &str) -> Option<String> {
    let start = src.find("<svg")?;
    let end = src.rfind("</svg>")? + "</svg>".len();
    let mut svg = src[start..end].to_string();
    if svg.contains("<script") || svg.contains("javascript:") || svg.contains("<foreignObject") {
        return None;
    }
    while let (Some(a), Some(b)) = (svg.find("<title>"), svg.find("</title>")) {
        if b < a {
            break;
        }
        svg.replace_range(a..b + "</title>".len(), "");
    }
    svg = remove_attr(&svg, "style");
    svg = svg.replace(" id=\"", " class=\"mark-");
    Some(svg.replacen("<svg", "<svg class=\"mark\" aria-hidden=\"true\" focusable=\"false\"", 1))
}

/// Remove every `name="…"` attribute.
fn remove_attr(svg: &str, name: &str) -> String {
    let needle = format!(" {name}=\"");
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(i) = rest.find(&needle) {
        out.push_str(&rest[..i]);
        let after = &rest[i + needle.len()..];
        rest = after.find('"').map_or("", |j| &after[j + 1..]);
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_svg_strips_prolog_and_refuses_scripts() {
        let s = inline_svg("<?xml version=\"1.0\"?>\n<!-- x --><svg viewBox=\"0 0 1 1\"><path/></svg>\n").unwrap();
        assert!(s.starts_with("<svg class=\"mark\""));
        assert!(inline_svg("<svg><script>alert(1)</script></svg>").is_none());
        let m = inline_svg("<svg><title>X</title><path id=\"word\" d=\"M1 1\" style=\"stroke:red;\" /></svg>").unwrap();
        assert_eq!(
            m,
            "<svg class=\"mark\" aria-hidden=\"true\" focusable=\"false\"><path class=\"mark-word\" d=\"M1 1\" /></svg>"
        );
    }

    #[test]
    fn fallback_favicon_uses_given_colours() {
        let f = fallback_favicon("BRAND", "ON", "EMBER");
        assert!(f.contains("fill=\"BRAND\"") && f.contains("stroke=\"ON\"") && f.contains("fill=\"EMBER\""));
    }
}
