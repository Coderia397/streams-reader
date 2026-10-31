//! Library (home): your documents first. "Continue listening", the ways to add something, search, every document,
//! and the share-target entry point. No store, no upsell.

mod hero;
pub mod meta;
mod row;

use crate::model::DocumentSummary;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_query_map;

use crate::components::add_sheet::{AddMode, AddSheet, start_import};
use crate::components::icon::{I, Icon, Spinner};
use crate::components::sheet::Sheet;
use crate::components::skeleton::RowSkeleton;
use crate::components::spot::Empty;
use crate::import_flow::Stage;
use crate::import_flow::source::from_share;
use crate::reading::search::matches_words;
use crate::state::library::in_progress;
use crate::state::player::OpenRequest;
use crate::state::use_app;
use hero::Hero;
use meta::haystack;
use row::DocRow;

/// The three ways in, as three equal tiles: compact above the list, full size on the empty library. Equal columns
/// with the label under the icon fit every language at phone width (a chip row did not: "Abrir um arquivo").
#[component]
fn AddTiles(add: Callback<AddMode>, #[prop(optional)] compact: bool) -> impl IntoView {
    view! {
        <div class="tiles" class:tiles--compact=compact>
            {AddMode::ALL.into_iter().map(|m| view! {
                <button class="tile" type="button" on:click=move |_| add.run(m)>
                    <Icon icon=m.icon() />
                    <span class="tile__label">{m.label()}</span>
                </button>
            }).collect_view()}
        </div>
    }
}

#[component]
pub fn LibraryPage() -> impl IntoView {
    let app = use_app();
    let add_open = RwSignal::new(false);
    let add_mode = RwSignal::new(None::<AddMode>);
    let share_busy = RwSignal::new(None::<Stage>);
    let actions_for = RwSignal::new(None::<DocumentSummary>);
    let actions_open = RwSignal::new(false);
    let searching = RwSignal::new(false);
    let query = RwSignal::new(String::new());

    spawn_local(app.library.refresh(app.toasts));

    // Web Share Target: `/app/?title=…&text=…&url=…`.
    let q = use_query_map();
    Effect::new(move |_| {
        let q = q.get();
        let (title, text, url) = (q.get("title"), q.get("text"), q.get("url"));
        if title.is_none() && text.is_none() && url.is_none() {
            return;
        }
        app.nav.replace("/");
        if let Some(src) = from_share(title.as_deref(), text.as_deref(), url.as_deref()) {
            start_import(app, src, share_busy, || {});
        }
    });

    let add = Callback::new(move |m: AddMode| {
        add_mode.set(Some(m));
        add_open.set(true);
    });
    let on_more = Callback::new(move |doc: DocumentSummary| {
        actions_for.set(Some(doc));
        actions_open.set(true);
    });
    let delete = move |_| {
        let Some(doc) = actions_for.get_untracked() else { return };
        actions_open.set(false);
        if app.player.doc_id().as_deref() == Some(doc.id.as_str()) {
            app.player.close();
        }
        app.library.delete_with_undo(doc, app.toasts);
    };
    let play_doc = move |_| {
        let Some(doc) = actions_for.get_untracked() else { return };
        actions_open.set(false);
        spawn_local(app.player.open(OpenRequest::doc(doc.id, true), app));
    };
    let toggle_search = move |_| {
        searching.update(|s| *s = !*s);
        if !searching.get_untracked() {
            query.set(String::new());
        }
    };

    let content = move || {
        let loading = app.library.loading.get();
        let failed = app.library.failed.get();
        let q = query.get();
        app.library.docs.with(|docs| match docs {
            None if failed => view! {
                <Empty icon=I::Alert title=t!("library-unreachable") line=t!("library-unreachable-line")>
                    <button class="btn btn--secondary" type="button" on:click=move |_| spawn_local(app.library.refresh(app.toasts))>
                        <Icon icon=I::Refresh /><span>{t!("action-retry")}</span>
                    </button>
                </Empty>
            }.into_any(),
            None => view! {
                <div class="hero hero--skeleton" aria-hidden="true"><div class="skel skel--hero"></div></div>
                <RowSkeleton />
            }.into_any(),
            Some(list) if list.is_empty() => view! {
                <Empty icon=I::BookOpen title=t!("library-empty-title") line=t!("library-empty-line")>
                    <AddTiles add=add />
                </Empty>
                <p class="tip">{t!("library-share-tip")}</p>
            }.into_any(),
            Some(list) if !q.trim().is_empty() => {
                let found: Vec<DocumentSummary> = list.iter().filter(|d| matches_words(&haystack(d), &q)).cloned().collect();
                if found.is_empty() {
                    view! { <Empty icon=I::Search title=t!("search-no-results") line=t!("library-search-empty-line") /> }.into_any()
                } else {
                    view! {
                        <ul class="doc-list">
                            {found.into_iter().map(|doc| view! { <DocRow doc=doc on_more=on_more /> }).collect_view()}
                        </ul>
                    }.into_any()
                }
            }
            Some(list) => {
                let hero = in_progress(list).cloned();
                let hero_id = hero.as_ref().map(|h| h.id.clone());
                let rest: Vec<DocumentSummary> =
                    list.iter().filter(|d| Some(&d.id) != hero_id.as_ref()).cloned().collect();
                view! {
                    {hero.map(|doc| view! { <Hero doc=doc /> })}
                    <AddTiles add=add compact=true />
                    <div class="list-head">
                        <h2 class="overline">{t!("library-all")}</h2>
                        <Show when=move || loading><Spinner /></Show>
                    </div>
                    <ul class="doc-list">
                        {rest.into_iter().map(|doc| view! { <DocRow doc=doc on_more=on_more /> }).collect_view()}
                    </ul>
                }.into_any()
            }
        })
    };

    view! {
        <div class="page">
            <header class="page-head">
                <h1 class="page-title">{t!("library-title")}</h1>
                <div class="page-head__actions">
                    <button class="icon-btn" type="button" aria-label=t!("library-search")
                        aria-pressed=move || searching.get().to_string() on:click=toggle_search>
                        <Icon icon=I::Search />
                    </button>
                    <button class="icon-btn icon-btn--primary" type="button" aria-label=t!("add-title")
                        on:click=move |_| { add_mode.set(None); add_open.set(true); }>
                        <Icon icon=I::Plus />
                    </button>
                </div>
            </header>
            <Show when=move || searching.get()>
                <label class="search-field">
                    <Icon icon=I::Search />
                    <input class="search-field__input" type="search" autofocus=true placeholder=t!("library-search-placeholder")
                        prop:value=move || query.get()
                        on:input=move |e| query.set(event_target_value(&e)) />
                </label>
            </Show>
            <Show when=move || share_busy.get().is_some()>
                <div class="busy busy--card" role="status">
                    <Spinner />
                    <span>{move || share_busy.get().map(Stage::label).unwrap_or_default()}</span>
                </div>
            </Show>
            {content}
            <AddSheet open=add_open mode=add_mode />
            <Sheet open=actions_open title=Signal::derive(move || actions_for.with(|d| d.as_ref().map(|d| d.meta.title.clone()).unwrap_or_default()))>
                <div class="stack">
                    <button class="row-item" type="button" on:click=play_doc>
                        <span class="row-item__icon"><Icon icon=I::Play /></span>
                        <span class="row-item__text"><span class="row-item__label">{t!("library-play")}</span></span>
                    </button>
                    <button class="row-item row-item--danger" type="button" on:click=delete>
                        <span class="row-item__icon"><Icon icon=I::Trash /></span>
                        <span class="row-item__text"><span class="row-item__label">{t!("library-delete")}</span></span>
                    </button>
                </div>
            </Sheet>
        </div>
    }
}
