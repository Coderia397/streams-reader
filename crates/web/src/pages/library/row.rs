//! One document in the library list. A row whose body is still uploading says "Preparing…"; one whose upload was
//! cut off says so in the danger colour and offers Retry and Delete.

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::meta::{Upload, fraction, is_finished, meta_line, ms_until_stuck, source_icon, upload_state};
use crate::components::cover::{Cover, CoverSize};
use crate::components::icon::{I, Icon, Spinner};
use crate::model::DocumentSummary;
use crate::state::use_app;
use crate::util::timer::now_ms;

#[component]
pub fn DocRow(doc: DocumentSummary, on_more: Callback<DocumentSummary>) -> impl IntoView {
    let app = use_app();
    let id = StoredValue::new(doc.id.clone());
    let rate = app.settings.speed;
    let line_doc = doc.clone();
    let more_doc = StoredValue::new(doc.clone());
    let pct = fraction(&doc) * 100.0;
    let finished = is_finished(&doc);
    let icon = source_icon(&doc);
    let title = doc.meta.title.clone();
    let playing_here = move || id.with_value(|id| app.player.doc_id().as_deref() == Some(id.as_str()));

    let upload = RwSignal::new(upload_state(&doc, now_ms()));
    if upload.get_untracked() == Upload::Preparing {
        let wait = ms_until_stuck(&doc, now_ms()).unsigned_abs() + 1;
        set_timeout(
            move || {
                // The row may be gone by now (the signal is then disposed).
                if upload.try_get_untracked() == Some(Upload::Preparing) {
                    let _ = upload.try_set(Upload::Stuck);
                }
            },
            Duration::from_millis(wait),
        );
    }
    let retrying = RwSignal::new(false);
    let retry = move |_| {
        retrying.set(true);
        spawn_local(async move {
            let done = app.library.retry_upload(&id.get_value(), app.toasts).await;
            let _ = retrying.try_set(false);
            if done {
                let _ = upload.try_set(Upload::Ready);
            }
        });
    };
    let delete = move |_| app.library.delete_with_undo(more_doc.get_value(), app.toasts);

    let meta = move || match upload.get() {
        Upload::Ready => view! { <span>{meta_line(&line_doc, rate.get())}</span> }.into_any(),
        Upload::Preparing => view! { <span>{t!("library-preparing")}</span> }.into_any(),
        Upload::Stuck => view! { <span class="doc-row__failed">{t!("library-upload-failed")}</span> }.into_any(),
    };
    view! {
        <li class="doc-row" class:is-finished=finished>
            <div class="doc-row__line">
                <button class="doc-row__main" type="button" on:click=move |_| app.nav.go(&format!("/read/{}", id.get_value()))>
                    <Cover title=title.clone() size=CoverSize::Small />
                    <span class="doc-row__body">
                        <span class="doc-row__title" dir="auto">{title}</span>
                        <span class="doc-row__meta">
                            <Show when=playing_here fallback=move || view! { <Icon icon=icon class="icon--sm" /> }>
                                <span class="eq" aria-label=t!("player-now-playing")><i></i><i></i><i></i></span>
                            </Show>
                            {finished.then(|| view! { <Icon icon=I::CheckCircle class="icon--sm doc-row__done" /> })}
                            {meta}
                        </span>
                        <Show when=move || { pct > 0.0 && !finished }>
                            <span class="progress progress--thin progress--playback"><span class="progress__fill" style:width=format!("{pct:.2}%")></span></span>
                        </Show>
                    </span>
                </button>
                <Show when=move || upload.get() == Upload::Preparing>
                    <span class="doc-row__pending" role="status" aria-label=t!("library-preparing")>
                        <Spinner />
                    </span>
                </Show>
                <button class="icon-btn" type="button" aria-label=t!("library-more") on:click=move |_| on_more.run(more_doc.get_value())>
                    <Icon icon=I::MoreVertical />
                </button>
            </div>
            <Show when=move || upload.get() == Upload::Stuck>
                <div class="doc-row__actions" role="group" aria-label=t!("library-upload-failed")>
                    <button class="btn btn--soft btn--sm" type="button" disabled=move || retrying.get() on:click=retry>
                        {move || if retrying.get() { view! { <Spinner /> }.into_any() } else { view! { <Icon icon=I::Refresh /> }.into_any() }}
                        <span>{t!("action-retry")}</span>
                    </button>
                    <button class="btn btn--ghost btn--sm doc-row__delete" type="button" on:click=delete>
                        <Icon icon=I::Trash />
                        <span>{t!("action-delete")}</span>
                    </button>
                </div>
            </Show>
        </li>
    }
}
