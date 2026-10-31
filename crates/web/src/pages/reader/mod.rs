//! `/read/:id`: the document's text with live highlighting, and the player dock. Tap a sentence for "Play from
//! here" (double tap plays at once); scrolling by hand pauses follow mode and shows "Back to voice". While playing,
//! the dock folds to a slim strip after a few seconds without a touch.

mod block;
mod bookmarks;
mod ctx;
mod follow;
mod keys;
mod more;
mod text_view;

use std::time::Duration;

use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::components::cover::{Cover, CoverSize};
use crate::components::icon::{I, Icon};
use crate::components::player::dock::Dock;
use crate::components::player::sheets::{ContentsSheet, SearchSheet};
use crate::config::DOCK_FOLD_MS;
use crate::state::player::OpenRequest;
use crate::state::use_app;
use ctx::{ReaderCtx, pill_top};
use more::MoreSheet;
use text_view::TextView;

/// Height of the reader's top bar, for placing the "Play from here" pill.
const BAR_PX: f64 = 56.0;

#[component]
pub fn ReaderPage() -> impl IntoView {
    let app = use_app();
    let player = app.player;
    let params = use_params_map();
    let id = Memo::new(move |_| params.with(|p| p.get("id").unwrap_or_default()));
    let ctx = ReaderCtx::new(player);
    provide_context(ctx);
    let follow = ctx.follow;
    let more_open = RwSignal::new(false);
    let search_open = RwSignal::new(false);
    let contents_open = RwSignal::new(false);
    let hide_text = RwSignal::new(false);

    Effect::new(move |_| {
        let id = id.get();
        if !id.is_empty() && player.doc_id().as_deref() != Some(id.as_str()) {
            spawn_local(player.open(OpenRequest::doc(id, false), app));
        }
    });

    // Fold the dock while playing after a pause in touches; any touch, key or scroll up unfolds it.
    let folded = RwSignal::new(false);
    let touched = RwSignal::new(0u64);
    let wake = move || {
        folded.set(false);
        touched.update(|n| *n += 1);
    };
    Effect::new(move |_| {
        let generation = touched.get();
        if !player.shows_pause() {
            folded.set(false);
            return;
        }
        set_timeout(
            move || {
                // The reader may be gone by now (the signals are then disposed).
                if touched.try_get_untracked() == Some(generation) && player.shows_pause() {
                    let _ = folded.try_set(true);
                }
            },
            Duration::from_millis(DOCK_FOLD_MS),
        );
    });
    let last_y = StoredValue::new(0.0f64);
    let on_scroll = window_event_listener(ev::scroll, move |_| {
        let y = crate::util::dom::window().and_then(|w| w.scroll_y().ok()).unwrap_or(0.0);
        if y < last_y.get_value() - 4.0 {
            wake();
        }
        last_y.set_value(y);
    });
    let on_pointer = window_event_listener(ev::pointerdown, move |_| wake());
    on_cleanup(move || {
        on_scroll.remove();
        on_pointer.remove();
    });

    let back_to_voice = move || {
        follow.resume();
        follow.keep_in_band(player.segment.get_untracked(), true);
    };
    keys::install(app, back_to_voice, move || search_open.set(true));

    let doc = move || player.doc.get().filter(|d| d.summary.id == id.get());
    let pill = move || ctx.picked.get().map(|p| (p.segment, pill_top(&p, BAR_PX)));
    view! {
        <div
            class="reader"
            style:--read-size=move || format!("{}px", app.settings.text_size.get())
        >
            <header class="reader__bar">
                <button class="icon-btn" type="button" aria-label=t!("reader-back") on:click=move |_| app.nav.go("/")>
                    <Icon icon=I::Back />
                </button>
                <bdi class="reader__bar-title">{move || player.chapter_title().unwrap_or_else(|| player.title())}</bdi>
                <button class="icon-btn" type="button" aria-label=t!("search-doc-title") on:click=move |_| search_open.set(true)>
                    <Icon icon=I::Search />
                </button>
            </header>
            {move || match doc() {
                Some(_) if hide_text.get() => view! {
                    <div class="listen-only">
                        <Cover title=player.title() size=CoverSize::Huge />
                        <bdi class="listen-only__title">{player.title()}</bdi>
                        <p class="listen-only__meta">{move || player.chapter_title().unwrap_or_default()}</p>
                        <button class="btn btn--ghost" type="button" on:click=move |_| hide_text.set(false)>
                            <Icon icon=I::Eye /><span>{t!("reader-show-text")}</span>
                        </button>
                    </div>
                }.into_any(),
                Some(d) => view! { <TextView doc=d /> }.into_any(),
                None if player.opening.get().is_some() || player.doc.with(Option::is_none) => view! {
                    <div class="reading reading--skeleton" aria-hidden="true">
                        <div class="skel skel--title"></div>
                        {(0..9).map(|i| view! { <div class="skel skel--text" class:skel--short=i % 3 == 2></div> }).collect_view()}
                    </div>
                }.into_any(),
                None => view! { <div class="reading"></div> }.into_any(),
            }}
            {move || pill().map(|(seg, top)| view! {
                <button class="pill pill--brand play-here" type="button" style:top=format!("{top:.0}px")
                    on:click=move |_| ctx.play_from(seg)>
                    <Icon icon=I::Play />
                    <span>{t!("reader-play-here")}</span>
                </button>
            })}
            <Show when=move || !follow.on.get() && doc().is_some() && !hide_text.get()>
                <button class="pill back-to-voice" type="button" on:click=move |_| back_to_voice()>
                    <Icon icon=I::Follow />
                    <span>{t!("reader-back-to-voice")}</span>
                </button>
            </Show>
            <Dock
                folded=folded.into()
                on_more=Callback::new(move |()| more_open.set(true))
                on_contents=Callback::new(move |()| contents_open.set(true))
            />
            <MoreSheet
                open=more_open
                hide_text=hide_text
                on_contents=Callback::new(move |()| contents_open.set(true))
                on_search=Callback::new(move |()| search_open.set(true))
                on_jump=Callback::new(move |seg: u32| ctx.play_from(seg))
            />
            <ContentsSheet open=contents_open />
            <SearchSheet open=search_open on_jump=Callback::new(move |seg: u32| ctx.play_from(seg)) />
        </div>
    }
}
