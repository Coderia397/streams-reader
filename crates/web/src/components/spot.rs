//! The quiet mark for empty, success and onboarding states: one line icon in a soft disc (D40; the illustrated spot
//! set was removed as below the brand's bar).

use leptos::prelude::*;

use super::icon::{I, Icon};

#[component]
pub fn Spot(icon: I, #[prop(optional)] small: bool) -> impl IntoView {
    view! {
        <span class="spot" class:spot--small=small aria-hidden="true">
            <Icon icon=icon />
        </span>
    }
}

/// An empty or error state: icon disc, headline, one line, at most two actions.
#[component]
pub fn Empty(
    icon: I,
    #[prop(into)] title: String,
    #[prop(into)] line: String,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="empty">
            <Spot icon=icon />
            <h2 class="empty__title">{title}</h2>
            <p class="empty__line">{line}</p>
            {children.map(|c| view! { <div class="empty__actions">{c()}</div> })}
        </div>
    }
}
