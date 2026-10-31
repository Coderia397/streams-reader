//! "Continue with Google / Microsoft / Apple": only the providers switched on in the project (`/auth/v1/settings`).
//! A new account binds its invite to the typed email first (`reserve_invite`), because an OAuth sign-up cannot carry
//! the code; the sign-up hook then admits the provider's verified email.

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::Form;
use super::email::{clean_invite, looks_like_email};
use crate::auth::types::AuthSettings;
use crate::auth::{Provider, http, pkce};
use crate::components::icon::{I, Icon};
use crate::db::rpc;
use crate::util::dom;

pub fn provider_icon(p: Provider) -> I {
    match p {
        Provider::Google => I::Google,
        Provider::Azure => I::Microsoft,
        Provider::Apple => I::Apple,
        Provider::Email => I::Mail,
    }
}

/// Sends the browser to the provider (PKCE); it comes back to `/app/auth/callback`.
pub fn start_oauth(p: Provider) {
    let Some(challenge) = pkce::begin() else { return };
    if let Some(w) = dom::window() {
        let _ = w.location().set_href(&http::authorize_url(p, &challenge, &http::redirect_to()));
    }
}

async fn continue_with(p: Provider, form: Form) {
    if form.new_account.get_untracked() {
        let email = form.email.get_untracked().trim().to_owned();
        let code = clean_invite(&form.invite.get_untracked());
        if !looks_like_email(&email) || code.is_empty() {
            form.error.set(Some(t!("sign-in-oauth-invite")));
            return;
        }
        match rpc::reserve_invite(&code, &email).await {
            Ok(status) if status.usable() => {}
            Ok(status) => {
                form.error.set(status.problem());
                return;
            }
            Err(e) => {
                form.error.set(Some(e.friendly()));
                return;
            }
        }
    }
    start_oauth(p);
}

#[component]
pub(super) fn Providers(form: Form) -> impl IntoView {
    let settings = RwSignal::new(None::<AuthSettings>);
    spawn_local(async move { settings.set(http::settings().await.ok()) });
    let list = move || settings.with(|s| s.as_ref().map(AuthSettings::providers).unwrap_or_default());
    view! {
        <Show when=move || !list().is_empty()>
            <div class="divider"><span>{t!("sign-in-or")}</span></div>
            <div class="stack stack--snug">
                {move || list().into_iter().map(|p| view! {
                    <button class="btn btn--secondary btn--block btn--lg" type="button" disabled=move || form.busy.get()
                        on:click=move |_| {
                            form.error.set(None);
                            spawn_local(continue_with(p, form));
                        }>
                        <Icon icon=provider_icon(p) />
                        <span>{t!("sign-in-with", provider = p.label())}</span>
                    </button>
                }).collect_view()}
            </div>
        </Show>
    }
}
