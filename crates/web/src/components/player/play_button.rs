//! The round accent play/pause button, with a ring spinner while audio is loading.

use leptos::prelude::*;

use crate::components::icons::{I, Icon};
use crate::state::use_app;

#[component]
pub fn PlayButton(#[prop(optional)] small: bool) -> impl IntoView {
    let player = use_app().player;
    let label = move || if player.shows_pause() { "Pause" } else { "Play" };
    view! {
        <button
            class="play-btn"
            class:play-btn--small=small
            class:is-waiting=move || player.is_waiting()
            type="button"
            aria-label=label
            on:click=move |e| {
                e.stop_propagation();
                player.toggle();
            }
        >
            <span class="play-btn__ring" aria-hidden="true"></span>
            {move || if player.shows_pause() {
                view! { <Icon icon=I::Pause /> }
            } else {
                view! { <Icon icon=I::Play class="play-btn__play" /> }
            }}
        </button>
    }
}
