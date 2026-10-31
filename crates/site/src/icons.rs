//! Remix Icon SVGs (vendored in `assets/icons/`, Remix Icon License v1.0), inlined into the HTML so they cost no
//! request and inherit `currentColor`. Icons are decorative: the surrounding text carries the meaning.

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        /// The raw SVG source of a vendored icon.
        fn source(name: &str) -> Option<&'static str> {
            match name {
                $($name => Some(include_str!(concat!("../assets/icons/", $name, ".svg"))),)*
                _ => None,
            }
        }

        #[cfg(test)]
        const NAMES: &[&str] = &[$($name),*];
    };
}

icons!(
    "add-line",
    "android-fill",
    "apple-fill",
    "arrow-down-s-line",
    "arrow-left-line",
    "arrow-left-s-line",
    "arrow-right-line",
    "arrow-right-s-line",
    "arrow-right-up-line",
    "article-line",
    "book-open-line",
    "book-shelf-line",
    "bookmark-line",
    "check-line",
    "chrome-line",
    "close-line",
    "code-s-slash-line",
    "download-2-line",
    "external-link-line",
    "eye-off-line",
    "file-pdf-2-line",
    "file-text-line",
    "gift-line",
    "github-fill",
    "global-line",
    "headphone-line",
    "keyboard-line",
    "links-line",
    "lock-line",
    "macbook-line",
    "mail-line",
    "menu-line",
    "mic-line",
    "moon-line",
    "pause-fill",
    "play-fill",
    "puzzle-2-line",
    "question-line",
    "quill-pen-line",
    "record-circle-fill",
    "shield-check-line",
    "skip-back-fill",
    "skip-forward-fill",
    "smartphone-line",
    "sparkling-line",
    "speed-up-line",
    "star-line",
    "terminal-box-line",
    "time-line",
    "timer-line",
    "translate-2",
    "ubuntu-fill",
    "user-voice-line",
    "voiceprint-line",
    "volume-up-line",
    "wifi-off-line",
    "windows-fill",
);

/// An inline, decorative icon. Unknown names render nothing (and fail the unit test that lists every use).
pub fn icon(name: &str) -> String {
    icon_with_class(name, "ri")
}

/// An inline icon with extra classes (for icons that flip in right-to-left layouts: `ri ri-flip`).
pub fn icon_with_class(name: &str, class: &str) -> String {
    let Some(svg) = source(name) else { return String::new() };
    svg.trim().replacen(
        "<svg ",
        &format!("<svg class=\"{class}\" aria-hidden=\"true\" focusable=\"false\" width=\"24\" height=\"24\" "),
        1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_a_single_svg_with_current_color() {
        for name in NAMES {
            let svg = icon(name);
            assert!(svg.starts_with("<svg class=\"ri\" aria-hidden=\"true\""), "{name}");
            assert!(svg.contains("currentColor"), "{name}");
            assert!(!svg.contains("<script"), "{name}");
        }
    }

    /// Every icon name written in the page sources (`"…-line"`, `"…-fill"`) is vendored.
    #[test]
    fn every_icon_used_in_the_pages_exists() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut files = vec![format!("{dir}/layout.rs")];
        for e in std::fs::read_dir(format!("{dir}/pages")).unwrap().flatten() {
            files.push(e.path().display().to_string());
        }
        for f in files {
            let src = std::fs::read_to_string(&f).unwrap();
            for part in src.split('"').skip(1).step_by(2) {
                let is_icon = (part.ends_with("-line") || part.ends_with("-fill"))
                    && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    && !part.starts_with("footer-");
                if is_icon {
                    assert!(NAMES.contains(&part), "{f}: unknown icon {part}");
                }
            }
        }
    }

    #[test]
    fn unknown_icon_is_empty() {
        assert_eq!(icon("nope"), "");
    }
}
