//! Generated covers: a deep base hue and an accent glow picked from a hash of the title (drawn in CSS), and the
//! title's first letter as a monogram in the reading face. Deterministic, so a document looks the same everywhere
//! without storing an image.

use leptos::prelude::*;

use crate::util::format::initials;
use crate::util::hash::stable_hash;

/// Base hues (OKLCH degrees, as `_cover.scss` draws them) that stay rich at low lightness: reds, greens, teals,
/// blues, violets, magentas. Amber, olive and yellow (OKLCH 60–120°) turn muddy and are left out.
const BASE_HUES: [u32; 12] = [27, 40, 150, 165, 190, 225, 250, 264, 282, 300, 322, 355];

/// `(hue_a, hue_b)` in degrees: a curated base hue with ±8° jitter, and a second hue 35–95° away so gradients never
/// look flat or clash.
pub fn hues(title: &str) -> (u32, u32) {
    let h = stable_hash(title);
    let base = BASE_HUES[(h % BASE_HUES.len() as u32) as usize];
    let a = (base + 352 + (h >> 8) % 17) % 360;
    let gap = 35 + (h >> 16) % 61;
    (a, (a + gap) % 360)
}

/// The two hues as custom properties; `_cover.scss` turns them into the gradient, so its look stays in the style
/// layer.
pub fn hue_vars(title: &str) -> String {
    let (a, b) = hues(title);
    format!("--hue-a: {a}; --hue-b: {b};")
}

/// The first letter of the title's initials (`"Alice's Adventures"` → `"A"`, Arabic keeps its first letter).
pub fn monogram(title: &str) -> String {
    initials(title).chars().next().map(String::from).unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverSize {
    /// 40 px (mini-player).
    Tiny,
    /// 48x72 (library rows).
    Small,
    /// 72x108 (continue card).
    Large,
    /// Hide-text mode in the reader.
    Huge,
}

impl CoverSize {
    fn class(self) -> &'static str {
        match self {
            CoverSize::Tiny => "cover cover--xs",
            CoverSize::Small => "cover cover--s",
            CoverSize::Large => "cover cover--l",
            CoverSize::Huge => "cover cover--xl",
        }
    }
}

#[component]
pub fn Cover(#[prop(into)] title: String, size: CoverSize) -> impl IntoView {
    let style = hue_vars(&title);
    let letters = monogram(&title);
    view! {
        <div class=size.class() style=style aria-hidden="true">
            <span class="cover__monogram">{letters}</span>
            <span class="cover__spine"></span>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hues_are_stable_and_apart() {
        let (a, b) = hues("Dune");
        assert_eq!(hues("Dune"), (a, b));
        let gap = (b + 360 - a) % 360;
        assert!((35..=95).contains(&gap), "gap {gap}");
        assert_ne!(hues("Dune"), hues("Emma"));
        assert!(hue_vars("x").starts_with("--hue-a: "));
        assert_eq!(monogram("Alice's Adventures in Wonderland"), "A");
        assert_eq!(monogram("مقدمة ابن خلدون"), "م");
    }
}
