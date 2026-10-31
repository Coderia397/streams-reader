//! UI icons from Remix Icon (Apache 2.0, `assets/icons/remix/`). Each SVG is compiled into the binary with
//! `include_str!` and rendered inline through [`Icon`], so there is no icon font and no runtime fetch. Icons whose
//! meaning has a direction (back, forward, chevrons) mirror in right-to-left languages.

mod registry;

use leptos::prelude::*;

pub use registry::I;

/// The `d` attribute of the single `<path>` in a Remix SVG.
pub(crate) fn path_data(svg: &'static str) -> &'static str {
    svg.split_once(" d=\"").and_then(|(_, rest)| rest.split_once('"')).map_or("", |(d, _)| d)
}

/// An inline icon, decorative by default (the button around it carries the label).
#[component]
pub fn Icon(icon: I, #[prop(optional)] class: &'static str) -> impl IntoView {
    let class = match (icon.mirrors(), class.is_empty()) {
        (true, true) => "icon icon--mirror".to_owned(),
        (true, false) => format!("icon icon--mirror {class}"),
        (false, true) => "icon".to_owned(),
        (false, false) => format!("icon {class}"),
    };
    view! {
        <svg class=class viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <path d=path_data(icon.svg()) />
        </svg>
    }
}

/// A spinning loader.
#[component]
pub fn Spinner(#[prop(optional)] class: &'static str) -> impl IntoView {
    view! { <Icon icon=I::Loader class=if class.is_empty() { "spin" } else { class } /> }
}
