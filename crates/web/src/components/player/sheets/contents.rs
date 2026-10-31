//! Contents: the document's headings with estimated durations; tap one to listen from there.

use leptos::prelude::*;

use crate::components::icon::{I, Icon};
use crate::components::sheet::Sheet;
use crate::state::use_app;
use crate::util::format;

/// More chapters than this open the tall sheet.
const TALL_AFTER: usize = 8;

#[component]
pub fn ContentsSheet(open: RwSignal<bool>) -> impl IntoView {
    let player = use_app().player;
    let rows = move || -> Vec<(usize, String, u32, f64)> {
        let rate = player.rate.get();
        player.doc.with(|d| {
            d.as_ref()
                .map(|d| {
                    d.model
                        .chapters
                        .iter()
                        .enumerate()
                        .map(|(i, c)| (i, c.title.clone(), c.segment, d.model.chapter_seconds(i, rate)))
                        .collect()
                })
                .unwrap_or_default()
        })
    };
    // A short table of contents gets a sheet sized to its rows; a long one gets the tall sheet (the default sheet's
    // max-height still scrolls, the tall one just opens at full height).
    let tall =
        Signal::derive(move || player.doc.with(|d| d.as_ref().is_some_and(|d| d.model.chapters.len() > TALL_AFTER)));
    let current = Memo::new(move |_| {
        let seg = player.segment.get();
        player.doc.with(|d| d.as_ref().and_then(|d| d.model.chapter_at(seg)))
    });
    view! {
        <Sheet open=open title=t!("contents-title") tall=tall>
            <ol class="contents">
                <For each=rows key=|r| (r.0, r.2) let:row>
                    {
                        let (i, title, segment, secs) = row;
                        view! {
                            <li>
                                <button
                                    class="contents__item"
                                    class:is-current=move || current.get() == Some(i)
                                    type="button"
                                    on:click=move |_| {
                                        player.jump_to(segment);
                                        open.set(false);
                                    }
                                >
                                    <span class="contents__num">
                                        <Show when=move || current.get() == Some(i) fallback=move || (i + 1).to_string()>
                                            <Icon icon=I::Equalizer />
                                        </Show>
                                    </span>
                                    <span class="contents__title"><bdi>{title}</bdi></span>
                                    <span class="contents__time">{format::duration_short(secs)}</span>
                                </button>
                            </li>
                        }
                    }
                </For>
            </ol>
        </Sheet>
    }
}
