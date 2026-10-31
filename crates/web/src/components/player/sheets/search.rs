//! Search inside the open document; tap a match to listen from there.

use leptos::prelude::*;

use crate::components::icon::{I, Icon};
use crate::components::sheet::Sheet;
use crate::reading::search::{Hit, find};
use crate::state::use_app;

const MAX_HITS: usize = 60;

#[component]
pub fn SearchSheet(open: RwSignal<bool>, on_jump: Callback<u32>) -> impl IntoView {
    let player = use_app().player;
    let query = RwSignal::new(String::new());
    let hits = Memo::new(move |_| {
        let q = query.get();
        player.doc.with(|d| d.as_ref().map(|d| find(&d.model, &q, MAX_HITS)).unwrap_or_default())
    });
    view! {
        <Sheet open=open title=t!("search-doc-title") tall=true>
            <label class="search-field">
                <Icon icon=I::Search />
                <input class="search-field__input" type="search" autofocus=true placeholder=t!("search-doc-placeholder")
                    prop:value=move || query.get()
                    on:input=move |e| query.set(event_target_value(&e)) />
            </label>
            {move || {
                let list: Vec<Hit> = hits.get();
                if query.with(|q| q.trim().chars().count() < 2) {
                    return view! { <p class="sheet-note">{t!("search-doc-hint")}</p> }.into_any();
                }
                if list.is_empty() {
                    return view! { <p class="sheet-note">{t!("search-no-results")}</p> }.into_any();
                }
                view! {
                    <p class="sheet-note">{t!("search-doc-count", count = list.len())}</p>
                    <ul class="hits">
                        {list.into_iter().map(|h| {
                            let seg = h.segment;
                            view! {
                                <li>
                                    <button class="hits__item" type="button" dir="auto" on:click=move |_| {
                                        open.set(false);
                                        on_jump.run(seg);
                                    }>
                                        {h.before}<mark>{h.matched}</mark>{h.after}
                                    </button>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any()
            }}
        </Sheet>
    }
}
