//! The windowed reading view. Only a few display chunks around the position are in the DOM; sentinels at both ends
//! grow the window as the user scrolls, and playback moving elsewhere re-centres it.

use std::sync::Arc;
use std::time::Duration;

use leptos::ev;
use leptos::html;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::{IntersectionObserver, IntersectionObserverEntry, IntersectionObserverInit};

use super::block::BlockView;
use super::follow::Follow;
use crate::reading::window::Window;
use crate::state::player::OpenDoc;
use crate::state::use_app;

#[component]
fn ChunkView(doc: Arc<OpenDoc>, chunk: usize, current: Selector<u32>, follow: Follow) -> impl IntoView {
    let Some(c) = doc.model.chunks.get(chunk).copied() else { return ().into_any() };
    view! {
        <div class="chunk" data-chunk=chunk>
            {(c.first_block..c.end_block)
                .map(|b| view! { <BlockView doc=Arc::clone(&doc) block=b current=current.clone() follow=follow /> })
                .collect_view()}
        </div>
    }
    .into_any()
}

#[component]
pub fn TextView(doc: Arc<OpenDoc>, follow: Follow) -> impl IntoView {
    let player = use_app().player;
    let total = doc.model.chunks.len();
    let start_chunk = doc.model.chunk_of_segment(player.segment.get_untracked());
    let window = RwSignal::new(Window::around(start_chunk, total));
    let current = Selector::new(move || player.segment.get());
    let top_ref = NodeRef::<html::Div>::new();
    let bottom_ref = NodeRef::<html::Div>::new();

    // Re-centre on the playing chunk when it leaves the window while following.
    let model_doc = Arc::clone(&doc);
    Effect::new(move |_| {
        let seg = player.segment.get();
        let on = follow.on.get();
        let chunk = model_doc.model.chunk_of_segment(seg);
        if on && !window.get_untracked().contains(chunk) {
            window.set(Window::around(chunk, total));
        }
    });

    // Keep the current sentence in view after the DOM has updated.
    Effect::new(move |prev: Option<u32>| {
        let seg = player.segment.get();
        if follow.on.get() {
            let first = prev.is_none();
            set_timeout(move || follow.scroll_to(seg, first), Duration::from_millis(if first { 60 } else { 30 }));
        }
        seg
    });

    // Manual scrolling hands control to the user.
    let touch = window_event_listener(ev::touchmove, move |_| follow.user_moved());
    let wheel = window_event_listener(ev::wheel, move |_| follow.user_moved());
    on_cleanup(move || {
        touch.remove();
        wheel.remove();
    });

    // Sentinels: grow the window when one comes near the viewport.
    let observer = StoredValue::new_local(None::<(IntersectionObserver, Closure<dyn FnMut(js_sys::Array)>)>);
    Effect::new(move |_| {
        let (Some(top), Some(bottom)) = (top_ref.get(), bottom_ref.get()) else { return };
        if observer.with_value(Option::is_some) {
            return;
        }
        let top_el: web_sys::Element = top.clone().into();
        let cb = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
            for entry in entries.iter() {
                let Ok(entry) = entry.dyn_into::<IntersectionObserverEntry>() else { continue };
                if !entry.is_intersecting() {
                    continue;
                }
                let is_top = entry.target() == top_el;
                window.update(|w| *w = if is_top { w.grow_up() } else { w.grow_down(total) });
            }
        });
        let init = IntersectionObserverInit::new();
        init.set_root_margin("800px 0px");
        if let Ok(obs) = IntersectionObserver::new_with_options(cb.as_ref().unchecked_ref(), &init) {
            obs.observe(&top);
            obs.observe(&bottom);
            observer.set_value(Some((obs, cb)));
        }
    });
    // A sentinel still in range after a grow does not fire again; re-observing makes the browser report it anew.
    Effect::new(move |_| {
        let _ = window.get();
        set_timeout(
            move || {
                observer.with_value(|o| {
                    if let (Some((obs, _)), Some(top), Some(bottom)) =
                        (o, top_ref.get_untracked(), bottom_ref.get_untracked())
                    {
                        obs.unobserve(&top);
                        obs.unobserve(&bottom);
                        obs.observe(&top);
                        obs.observe(&bottom);
                    }
                });
            },
            Duration::from_millis(120),
        );
    });
    on_cleanup(move || {
        observer.with_value(|o| {
            if let Some((obs, _)) = o {
                obs.disconnect();
            }
        });
    });

    let title = doc.summary.meta.title.clone();
    let author = doc.summary.meta.author.clone();
    let chunks_doc = Arc::clone(&doc);
    view! {
        <article class="reading" lang="en">
            <Show when=move || window.get().at_top()>
                <header class="reading__head">
                    <h1 class="reading__title">{title.clone()}</h1>
                    {author.clone().map(|a| view! { <p class="reading__author">{a}</p> })}
                </header>
            </Show>
            <div class="sentinel" node_ref=top_ref class:is-off=move || window.get().at_top()></div>
            <For
                each=move || { let w = window.get(); w.start..w.end }
                key=|c| *c
                let:chunk
            >
                <ChunkView doc=Arc::clone(&chunks_doc) chunk=chunk current=current.clone() follow=follow />
            </For>
            <div class="sentinel" node_ref=bottom_ref class:is-off=move || window.get().at_bottom(total)></div>
            <Show when=move || window.get().at_bottom(total)>
                <p class="reading__end" aria-hidden="true">"·  ·  ·"</p>
            </Show>
        </article>
    }
}
