//! The brand mark v2 (D26): the pixel book on the 16×16 grid, read from `brand/mark/streams-reader-mark-currentcolor.svg`
//! at compile time, so a rebuild of the brand updates it here. Drawn at whole-pixel cell sizes only (16, 32, 48 px)
//! with crisp edges; the colour comes from the tokens through CSS (`currentColor`). `tile` puts the white mark on
//! the blue app tile, as on the home screen.

use leptos::prelude::*;

use super::icon::path_data;

const MARK_SVG: &str = include_str!("../../../../brand/mark/streams-reader-mark-currentcolor.svg");

/// The mark's pixel outline (`d` of its single path).
pub fn mark_path() -> &'static str {
    path_data(MARK_SVG)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogoSize {
    /// 32 px, 2 px cells (headers, the desktop rail).
    #[default]
    Medium,
    /// 48 px, 3 px cells (sign-in, splash), on a 64 px tile when `tile`.
    Large,
}

impl LogoSize {
    fn class(self) -> &'static str {
        match self {
            LogoSize::Medium => "mark",
            LogoSize::Large => "mark mark--lg",
        }
    }
}

#[component]
pub fn Logo(#[prop(optional)] size: LogoSize, #[prop(optional)] tile: bool) -> impl IntoView {
    view! {
        <span class=size.class() class:mark--tile=tile aria-hidden="true">
            <svg viewBox="0 0 16 16" focusable="false" shape-rendering="crispEdges">
                <path fill="currentColor" d=mark_path() />
            </svg>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_comes_from_the_brand_file() {
        assert!(MARK_SVG.contains("viewBox=\"0 0 16 16\""));
        assert!(mark_path().starts_with('M'));
        assert!(mark_path().ends_with('z'));
    }
}
