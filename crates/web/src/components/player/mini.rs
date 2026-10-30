//! Persistent mini-player above the tab bar on every screen except the reader.

use leptos::prelude::*;

use super::play_button::PlayButton;
use crate::components::cover::{Cover, CoverSize};
use crate::state::use_app;
use crate::util::format;

#[component]
pub fn MiniPlayer() -> impl IntoView {
    let app = use_app();
    let player = app.player;
    let open = move |_| {
        if let Some(id) = player.doc_id() {
            app.nav.go(&format!("/read/{id}"));
        }
    };
    view! {
        <Show when=move || player.has_doc()>
            <div class="mini" role="region" aria-label="Now playing">
                <div class="mini__progress" style:width=move || format!("{:.2}%", player.fraction() * 100.0)></div>
                <button class="mini__body" type="button" on:click=open>
                    {move || view! { <Cover title=player.title() size=CoverSize::Small /> }}
                    <span class="mini__text">
                        <span class="mini__title">{move || player.title()}</span>
                        <span class="mini__meta">
                            {move || player.chapter_title().unwrap_or_else(|| format::time_left(player.seconds_left()))}
                        </span>
                    </span>
                </button>
                <PlayButton small=true />
            </div>
        </Show>
    }
}
