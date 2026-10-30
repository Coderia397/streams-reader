//! `/read/:id`: the document's text with live highlighting, and the player dock.

mod block;
mod follow;
mod text_view;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::components::icons::{I, Icon};
use crate::components::player::dock::Dock;
use crate::state::player::OpenRequest;
use crate::state::use_app;
use follow::Follow;
use text_view::TextView;

#[component]
pub fn ReaderPage() -> impl IntoView {
    let app = use_app();
    let player = app.player;
    let params = use_params_map();
    let id = Memo::new(move |_| params.with(|p| p.get("id").unwrap_or_default()));
    let follow = Follow::new();

    Effect::new(move |_| {
        let id = id.get();
        if !id.is_empty() && player.doc_id().as_deref() != Some(id.as_str()) {
            spawn_local(player.open(OpenRequest::doc(id, false), app));
        }
    });

    let doc = move || player.doc.get().filter(|d| d.summary.id == id.get());
    let back_to_current = move |_| {
        follow.resume();
        follow.scroll_to(player.segment.get_untracked(), true);
    };
    view! {
        <div class="reader">
            <header class="reader__bar">
                <button class="icon-btn" type="button" aria-label="Back to library" on:click=move |_| app.nav.go("/")>
                    <Icon icon=I::ChevronLeft />
                </button>
                <p class="reader__bar-title">{move || player.chapter_title().unwrap_or_else(|| player.title())}</p>
                <span class="icon-btn-spacer"></span>
            </header>
            {move || match doc() {
                Some(d) => view! { <TextView doc=d follow=follow /> }.into_any(),
                None if player.opening.get().is_some() || player.doc.with(Option::is_none) => view! {
                    <div class="reading reading--skeleton" aria-hidden="true">
                        <div class="skel skel--title"></div>
                        {(0..9).map(|i| view! { <div class="skel skel--text" class:skel--short=i % 3 == 2></div> }).collect_view()}
                    </div>
                }.into_any(),
                None => view! { <div class="reading"></div> }.into_any(),
            }}
            <Show when=move || !follow.on.get() && doc().is_some()>
                <button class="pill" type="button" on:click=back_to_current>
                    <Icon icon=I::ArrowDown />
                    <span>"Back to current"</span>
                </button>
            </Show>
            <Dock />
        </div>
    }
}
