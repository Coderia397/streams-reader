//! Bottom navigation: Library, Voices, Settings. On wide screens it becomes a side rail (CSS). The active tab is
//! worked out from the route here (the router's own `aria-current` misses `/app/` with a trailing slash).

use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

use super::icon::{I, Icon};
use super::logo::Logo;
use crate::app::route_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Library,
    Voices,
    Settings,
}

/// The tab a route belongs to.
pub fn tab_of(route: &str) -> Option<Tab> {
    match route {
        "/" => Some(Tab::Library),
        "/voices" => Some(Tab::Voices),
        r if r.starts_with("/settings") => Some(Tab::Settings),
        _ => None,
    }
}

#[component]
pub fn TabBar() -> impl IntoView {
    let location = use_location();
    let active = Memo::new(move |_| tab_of(route_of(&location.pathname.get())));
    let tab = move |t: Tab, href: &'static str, icon: I, label: String| {
        view! {
            <A
                href=href
                attr:class=move || if active.get() == Some(t) { "tabbar__tab is-active" } else { "tabbar__tab" }
            >
                <Icon icon=icon />
                <span>{label}</span>
            </A>
        }
    };
    view! {
        <nav class="tabbar" aria-label=t!("nav-label")>
            <A href="/" attr:class="tabbar__brand" attr:aria-label=t!("app-name")><Logo /></A>
            {tab(Tab::Library, "/", I::Library, t!("nav-library"))}
            {tab(Tab::Voices, "/voices", I::Voices, t!("nav-voices"))}
            {tab(Tab::Settings, "/settings", I::Settings, t!("nav-settings"))}
        </nav>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_map_to_tabs() {
        assert_eq!(tab_of("/"), Some(Tab::Library));
        assert_eq!(tab_of("/voices"), Some(Tab::Voices));
        assert_eq!(tab_of("/voices/new"), None);
        assert_eq!(tab_of("/settings"), Some(Tab::Settings));
    }
}
