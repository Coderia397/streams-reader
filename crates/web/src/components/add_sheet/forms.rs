//! The Add sheet's two forms: paste text and paste a link. Each field has a visible label; a problem with what was
//! entered shows under the field it concerns (announced, and tied to the field with `aria-describedby`), not as a
//! toast that would cover the field on a phone.

use leptos::prelude::*;

use super::start_import;
use crate::components::icon::{I, Icon};
use crate::import_flow::source::normalise_url;
use crate::import_flow::{ImportSource, Stage};
use crate::state::use_app;

/// The id of the error line, for `aria-describedby`.
const ERROR_ID: &str = "add-error";

/// Shared by both forms: the field's error, if any, and what to do once the import has finished.
#[derive(Clone, Copy)]
pub struct FormCtx {
    pub error: RwSignal<Option<String>>,
    pub busy: RwSignal<Option<Stage>>,
    pub done: Callback<()>,
    pub back: Callback<()>,
}

impl FormCtx {
    fn invalid(self) -> Signal<Option<&'static str>> {
        Signal::derive(move || self.error.with(Option::is_some).then_some("true"))
    }

    fn described_by(self) -> Signal<Option<&'static str>> {
        Signal::derive(move || self.error.with(Option::is_some).then_some(ERROR_ID))
    }
}

#[component]
fn FieldError(error: RwSignal<Option<String>>) -> impl IntoView {
    move || {
        error.get().map(|e| {
            view! { <p class="field__error" id=ERROR_ID role="alert"><Icon icon=I::Alert />{e}</p> }
        })
    }
}

#[component]
fn Actions(back: Callback<()>, submit: String) -> impl IntoView {
    view! {
        <div class="form__actions">
            <button class="btn btn--ghost" type="button" on:click=move |_| back.run(())>{t!("action-back")}</button>
            <button class="btn btn--primary" type="submit">{submit}</button>
        </div>
    }
}

#[component]
pub fn PasteForm(ctx: FormCtx, title: RwSignal<String>, text: RwSignal<String>) -> impl IntoView {
    let app = use_app();
    let submit = move || {
        let body = text.get_untracked();
        if body.trim().is_empty() {
            ctx.error.set(Some(t!("add-paste-empty")));
            return;
        }
        let t = title.get_untracked();
        let title = (!t.trim().is_empty()).then(|| t.trim().to_owned());
        start_import(app, ImportSource::Paste { text: body, title }, ctx.busy, move || ctx.done.run(()));
    };
    view! {
        <form class="form" novalidate=true on:submit=move |e| { e.prevent_default(); submit(); }>
            <div class="field">
                <label class="field__label" for="add-paste-title">{t!("add-paste-title-label")}</label>
                <input class="input" id="add-paste-title" type="text" maxlength="200"
                    prop:value=move || title.get()
                    on:input=move |e| title.set(event_target_value(&e)) />
            </div>
            <div class="field">
                <label class="field__label" for="add-paste-text">{t!("add-paste-text-label")}</label>
                <textarea class="input input--area" id="add-paste-text" rows="8" placeholder=t!("add-paste-placeholder")
                    aria-invalid=ctx.invalid() aria-describedby=ctx.described_by()
                    prop:value=move || text.get()
                    on:input=move |e| { text.set(event_target_value(&e)); ctx.error.set(None); }></textarea>
                <FieldError error=ctx.error />
            </div>
            <Actions back=ctx.back submit=t!("add-paste-submit") />
        </form>
    }
}

#[component]
pub fn LinkForm(ctx: FormCtx, url: RwSignal<String>) -> impl IntoView {
    let app = use_app();
    let submit = move || match normalise_url(&url.get_untracked()) {
        Some(u) => start_import(app, ImportSource::Link { url: u }, ctx.busy, move || ctx.done.run(())),
        None => ctx.error.set(Some(t!("add-link-invalid"))),
    };
    view! {
        <form class="form" novalidate=true on:submit=move |e| { e.prevent_default(); submit(); }>
            <div class="field">
                <label class="field__label" for="add-link-url">{t!("add-link-label")}</label>
                <input class="input" id="add-link-url" type="url" inputmode="url" autocomplete="off" dir="ltr"
                    placeholder="https://"
                    aria-invalid=ctx.invalid() aria-describedby=ctx.described_by()
                    prop:value=move || url.get()
                    on:input=move |e| { url.set(event_target_value(&e)); ctx.error.set(None); } />
                <FieldError error=ctx.error />
            </div>
            <p class="form__hint">{t!("add-link-help")}</p>
            <Actions back=ctx.back submit=t!("add-link-submit") />
        </form>
    }
}
