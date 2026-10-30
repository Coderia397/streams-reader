//! Bottom sheet: the app's one modal surface (add, speed, voice, sleep, chapters, confirmations).

use leptos::ev;
use leptos::prelude::*;

use super::icons::{I, Icon};

#[component]
pub fn Sheet(
    open: RwSignal<bool>,
    #[prop(into)] title: String,
    #[prop(optional)] tall: bool,
    children: ChildrenFn,
) -> impl IntoView {
    let title = StoredValue::new(title);
    let children = StoredValue::new(children);
    let close = move |_| open.set(false);
    let on_key = window_event_listener(ev::keydown, move |e| {
        if e.key() == "Escape" && open.get_untracked() {
            open.set(false);
        }
    });
    on_cleanup(move || on_key.remove());
    view! {
        <Show when=move || open.get()>
            <div class="sheet-backdrop" on:click=close></div>
            <section
                class="sheet"
                class:sheet--tall=tall
                role="dialog"
                aria-modal="true"
                aria-label=move || title.get_value()
            >
                <div class="sheet__grip" aria-hidden="true"></div>
                <header class="sheet__head">
                    <h2 class="sheet__title">{move || title.get_value()}</h2>
                    <button class="icon-btn" type="button" aria-label="Close" on:click=close>
                        <Icon icon=I::X />
                    </button>
                </header>
                <div class="sheet__body">{move || children.with_value(|c| c())}</div>
            </section>
        </Show>
    }
}
