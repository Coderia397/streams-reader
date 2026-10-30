//! Chapters: the document's headings; tap one to listen from there.

use leptos::prelude::*;

use crate::components::sheet::Sheet;
use crate::reading::doc::Chapter;
use crate::state::use_app;

#[component]
pub fn ChaptersSheet(open: RwSignal<bool>) -> impl IntoView {
    let player = use_app().player;
    let chapters = move || -> Vec<(usize, Chapter)> {
        player
            .doc
            .with(|d| d.as_ref().map(|d| d.model.chapters.iter().cloned().enumerate().collect()).unwrap_or_default())
    };
    let current = Memo::new(move |_| {
        let seg = player.segment.get();
        player.doc.with(|d| d.as_ref().and_then(|d| d.model.chapter_at(seg)))
    });
    view! {
        <Sheet open=open title="Chapters" tall=true>
            <ol class="chapters">
                <For each=chapters key=|(i, c)| (*i, c.segment) let:item>
                    {
                        let (i, chapter) = item;
                        let segment = chapter.segment;
                        view! {
                            <li>
                                <button
                                    class="chapter"
                                    class:is-on=move || current.get() == Some(i)
                                    type="button"
                                    on:click=move |_| {
                                        player.jump_to(segment);
                                        open.set(false);
                                    }
                                >
                                    <span class="chapter__num">{i + 1}</span>
                                    <span class="chapter__title">{chapter.title.clone()}</span>
                                </button>
                            </li>
                        }
                    }
                </For>
            </ol>
        </Sheet>
    }
}
