//! One document in the library list.

use leptos::prelude::*;
use nolabs_core::proto::api::DocumentSummary;

use super::meta::{fraction, is_finished, meta_line};
use crate::components::cover::{Cover, CoverSize};
use crate::components::icons::{I, Icon, Spinner};
use crate::state::use_app;

#[component]
pub fn DocRow(doc: DocumentSummary, on_more: Callback<DocumentSummary>) -> impl IntoView {
    let app = use_app();
    let id = doc.id.clone();
    let id_open = doc.id.clone();
    let rate = app.settings.speed;
    let line_doc = doc.clone();
    let more_doc = doc.clone();
    let pct = fraction(&doc) * 100.0;
    let finished = is_finished(&doc);
    let playing_here = move || app.player.doc_id().as_deref() == Some(id.as_str());
    view! {
        <li class="row" class:is-finished=finished>
            <button class="row__main" type="button" on:click=move |_| app.nav.go(&format!("/read/{id_open}"))>
                <Cover title=doc.meta.title.clone() size=CoverSize::Small />
                <span class="row__body">
                    <span class="row__title">{doc.meta.title.clone()}</span>
                    <span class="row__meta">
                        <Show when=playing_here.clone()>
                            <span class="eq" aria-label="Now playing"><i></i><i></i><i></i></span>
                        </Show>
                        {move || meta_line(&line_doc, rate.get())}
                    </span>
                    <Show when=move || { pct > 0.0 && !finished }>
                        <span class="bar bar--thin"><span class="bar__fill" style:width=format!("{pct:.2}%")></span></span>
                    </Show>
                </span>
                {(!doc.ready).then(|| view! { <Spinner class="spin row__pending" /> })}
            </button>
            <button class="icon-btn" type="button" aria-label="More actions" on:click=move |_| on_more.run(more_doc.clone())>
                <Icon icon=I::More />
            </button>
        </li>
    }
}
