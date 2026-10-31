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
    "arrow-right-line",
    "arrow-right-up-line",
    "book-open-line",
    "bookmark-line",
    "check-line",
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
    "links-line",
    "lock-line",
    "macbook-line",
    "mail-line",
    "mic-line",
    "moon-line",
    "pause-fill",
    "play-fill",
    "question-line",
    "quill-pen-line",
    "shield-check-line",
    "smartphone-line",
    "sparkling-line",
    "speed-up-line",
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

    #[test]
    fn unknown_icon_is_empty() {
        assert_eq!(icon("nope"), "");
    }
}
