//! Library: "Continue listening", every document, the Add sheet, and the share-target entry point.

mod hero;
pub mod meta;
mod row;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_query_map;
use nolabs_core::proto::api::DocumentSummary;

use crate::components::add_sheet::{AddSheet, start_import};
use crate::components::icons::{I, Icon, Spinner};
use crate::components::logo::Logo;
use crate::components::sheet::Sheet;
use crate::components::skeleton::{Empty, RowSkeleton};
use crate::config::APP_NAME;
use crate::import_flow::Stage;
use crate::import_flow::source::from_share;
use crate::state::library::in_progress;
use crate::state::use_app;
use hero::Hero;
use row::DocRow;

#[component]
pub fn LibraryPage() -> impl IntoView {
    let app = use_app();
    let add_open = RwSignal::new(false);
    let share_busy = RwSignal::new(None::<Stage>);
    let actions_for = RwSignal::new(None::<DocumentSummary>);
    let actions_open = RwSignal::new(false);
    let confirm_delete = RwSignal::new(false);

    spawn_local(app.library.refresh(app.toasts));

    // Web Share Target: `/?title=…&text=…&url=…`.
    let query = use_query_map();
    Effect::new(move |_| {
        let q = query.get();
        let (title, text, url) = (q.get("title"), q.get("text"), q.get("url"));
        if title.is_none() && text.is_none() && url.is_none() {
            return;
        }
        app.nav.replace("/");
        if let Some(src) = from_share(title.as_deref(), text.as_deref(), url.as_deref()) {
            start_import(app, src, share_busy, || {});
        }
    });

    let on_more = Callback::new(move |doc: DocumentSummary| {
        actions_for.set(Some(doc));
        confirm_delete.set(false);
        actions_open.set(true);
    });
    let delete = move |_| {
        let Some(doc) = actions_for.get_untracked() else { return };
        actions_open.set(false);
        if app.player.doc_id().as_deref() == Some(doc.id.as_str()) {
            app.player.close();
        }
        spawn_local(async move {
            match app.library.delete(doc.id.clone()).await {
                Ok(()) => app.toasts.info(format!("Removed \u{201c}{}\u{201d}.", doc.meta.title)),
                Err(e) => app.toasts.api_error(&e),
            }
        });
    };

    let content = move || {
        let loading = app.library.loading.get();
        let failed = app.library.failed.get();
        app.library.docs.with(|docs| match docs {
            None if failed => view! {
                <Empty icon=I::Alert title="Can't reach your library" line="Check your connection, then try again.">
                    <button class="btn" type="button" on:click=move |_| spawn_local(app.library.refresh(app.toasts))>
                        <Icon icon=I::Refresh />"Try again"
                    </button>
                </Empty>
            }.into_any(),
            None => view! {
                <div class="hero hero--skeleton" aria-hidden="true"><div class="skel skel--hero"></div></div>
                <RowSkeleton />
            }.into_any(),
            Some(list) if list.is_empty() => view! {
                <Empty icon=I::Headphones title="Your shelf is empty" line="Paste text, add a link or open a book, then press play.">
                    <button class="btn btn--primary" type="button" on:click=move |_| add_open.set(true)>
                        <Icon icon=I::Plus />"Add your first"
                    </button>
                </Empty>
            }.into_any(),
            Some(list) => {
                let hero = in_progress(list).cloned();
                let hero_id = hero.as_ref().map(|h| h.id.clone());
                let rest: Vec<DocumentSummary> =
                    list.iter().filter(|d| Some(&d.id) != hero_id.as_ref()).cloned().collect();
                view! {
                    {hero.map(|doc| view! { <Hero doc=doc /> })}
                    <div class="list-head">
                        <h2 class="section-label">"All documents"</h2>
                        <Show when=move || loading><Spinner /></Show>
                    </div>
                    <ul class="rows">
                        {rest.into_iter().map(|doc| view! { <DocRow doc=doc on_more=on_more /> }).collect_view()}
                    </ul>
                }.into_any()
            }
        })
    };

    view! {
        <div class="page">
            <header class="topbar">
                <div class="brand"><Logo /><span class="brand__name">{APP_NAME}</span></div>
                <button class="btn btn--soft" type="button" on:click=move |_| add_open.set(true)>
                    <Icon icon=I::Plus /><span>"Add"</span>
                </button>
            </header>
            <h1 class="page-title">"Library"</h1>
            <Show when=move || share_busy.get().is_some()>
                <div class="busy busy--card" role="status">
                    <Spinner />
                    <span>{move || share_busy.get().map(Stage::label).unwrap_or_default()}</span>
                </div>
            </Show>
            {content}
            <AddSheet open=add_open />
            <Sheet open=actions_open title="Document">
                {move || actions_for.get().map(|doc| view! {
                    <p class="sheet-lead">{doc.meta.title.clone()}</p>
                    <Show
                        when=move || confirm_delete.get()
                        fallback=move || view! {
                            <button class="option option--danger" type="button" on:click=move |_| confirm_delete.set(true)>
                                <Icon icon=I::Trash /><span>"Delete from library"</span>
                            </button>
                        }
                    >
                        <p class="hint">"This removes the document, your place in it and its saved audio."</p>
                        <div class="btn-row">
                            <button class="btn" type="button" on:click=move |_| actions_open.set(false)>"Cancel"</button>
                            <button class="btn btn--danger" type="button" on:click=delete>"Delete"</button>
                        </div>
                    </Show>
                })}
            </Sheet>
        </div>
    }
}
