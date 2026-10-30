//! Generated covers: two hues from a hash of the title, a soft diagonal gradient, and the title's initials in the
//! reading face. Deterministic, so a document looks the same everywhere without storing an image.

use leptos::prelude::*;

use crate::util::format::initials;
use crate::util::hash::stable_hash;

/// Base hues that stay rich at low lightness (muddy olive and plain yellow are left out): reds, ambers, teals, blues,
/// violets, magentas.
const BASE_HUES: [u32; 12] = [4, 22, 38, 168, 186, 204, 222, 240, 258, 278, 304, 336];

/// `(hue_a, hue_b)` in degrees: a curated base hue with ±8° jitter, and a second hue 35–95° away so gradients never
/// look flat or clash.
pub fn hues(title: &str) -> (u32, u32) {
    let h = stable_hash(title);
    let base = BASE_HUES[(h % BASE_HUES.len() as u32) as usize];
    let a = (base + 352 + (h >> 8) % 17) % 360;
    let gap = 35 + (h >> 16) % 61;
    (a, (a + gap) % 360)
}

pub fn gradient(title: &str) -> String {
    let (a, b) = hues(title);
    format!(
        "background: radial-gradient(120% 90% at 0% 0%, hsl({a} 62% 58% / .55), transparent 60%), \
         linear-gradient(140deg, hsl({a} 46% 34%), hsl({b} 52% 20%));"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverSize {
    Small,
    Large,
}

impl CoverSize {
    fn class(self) -> &'static str {
        match self {
            CoverSize::Small => "cover cover--s",
            CoverSize::Large => "cover cover--l",
        }
    }
}

#[component]
pub fn Cover(#[prop(into)] title: String, size: CoverSize) -> impl IntoView {
    let style = gradient(&title);
    let letters = initials(&title);
    view! {
        <div class=size.class() style=style aria-hidden="true">
            <span class="cover__initials">{letters}</span>
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
        assert!(gradient("x").contains("hsl("));
    }
}
