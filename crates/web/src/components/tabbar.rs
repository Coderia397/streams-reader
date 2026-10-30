//! Bottom navigation: Library, Voices, Settings.

use leptos::prelude::*;
use leptos_router::components::A;

use super::icons::{I, Icon};

#[component]
pub fn TabBar() -> impl IntoView {
    view! {
        <nav class="tabbar" aria-label="Main">
            <A href="/" exact=true attr:class="tabbar__tab">
                <Icon icon=I::Library />
                <span>"Library"</span>
            </A>
            <A href="/voices" attr:class="tabbar__tab">
                <Icon icon=I::Voices />
                <span>"Voices"</span>
            </A>
            <A href="/settings" attr:class="tabbar__tab">
                <Icon icon=I::Settings />
                <span>"Settings"</span>
            </A>
        </nav>
    }
}
