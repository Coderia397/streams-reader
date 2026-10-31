//! Floating mini-player above the tab bar on every screen except the reader: cover, title, chapter or time left,
//! skip back, play, and a thin progress line along the bottom edge.

use leptos::prelude::*;

use super::play_button::PlayButton;
use super::skip::SkipButton;
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
            <div class="mini" role="region" aria-label=t!("player-now-playing")>
                <button class="mini__body" type="button" on:click=open>
                    {move || view! { <Cover title=player.title() size=CoverSize::Tiny /> }}
                    <span class="mini__text">
                        <span class="mini__title" dir="auto">{move || player.title()}</span>
                        <span class="mini__meta">
                            {move || player.chapter_title().unwrap_or_else(|| format::time_left(player.seconds_left()))}
                        </span>
                    </span>
                </button>
                <SkipButton forward=false class="mini__skip" />
                <PlayButton small=true />
                <div class="mini__progress" aria-hidden="true">
                    <span class="mini__fill" style:transform=move || format!("scaleX({:.4})", player.fraction())></span>
                </div>
            </div>
        </Show>
    }
}
