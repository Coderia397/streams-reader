//! `/login`: the passphrase screen.

use leptos::prelude::*;
use leptos::task::spawn_local;
use nolabs_core::proto::ErrorCode;

use crate::api::session;
use crate::components::icons::{I, Icon, Spinner};
use crate::components::logo::Logo;
use crate::config::APP_NAME;
use crate::state::session::Auth;
use crate::state::use_app;
use crate::storage::prefs;

#[component]
pub fn LoginPage() -> impl IntoView {
    let app = use_app();
    let passphrase = RwSignal::new(String::new());
    let reveal = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    let submit = move || {
        let p = passphrase.get_untracked();
        if p.trim().is_empty() || busy.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match session::login(p.trim()).await {
                Ok(()) => {
                    passphrase.set(String::new());
                    app.session.auth.set(Auth::SignedIn);
                    let welcomed = prefs::get::<bool>(prefs::WELCOMED).unwrap_or(false);
                    app.nav.replace(if welcomed { "/" } else { "/welcome" });
                }
                Err(e) => error.set(Some(match e.code() {
                    Some(ErrorCode::Unauthorized | ErrorCode::Forbidden) => "That passphrase didn't match.".to_owned(),
                    _ => e.friendly(),
                })),
            }
            busy.set(false);
        });
    };

    view! {
        <div class="login">
            <div class="login__card">
                <Logo large=true />
                <h1 class="login__name">{APP_NAME}</h1>
                <p class="login__tag">"Your library, read aloud."</p>
                <form class="form" on:submit=move |e| {
                    e.prevent_default();
                    submit();
                }>
                    <label class="field-wrap">
                        <span class="visually-hidden">"Passphrase"</span>
                        <Icon icon=I::Lock class="field-wrap__icon" />
                        <input
                            class="field field--icon"
                            type=move || if reveal.get() { "text" } else { "password" }
                            name="password"
                            autocomplete="current-password"
                            autocapitalize="off"
                            spellcheck="false"
                            placeholder="Passphrase"
                            prop:value=move || passphrase.get()
                            on:input=move |e| passphrase.set(event_target_value(&e))
                        />
                        <button class="field-wrap__end icon-btn" type="button"
                            aria-label=move || if reveal.get() { "Hide passphrase" } else { "Show passphrase" }
                            on:click=move |_| reveal.update(|r| *r = !*r)>
                            {move || if reveal.get() { view! { <Icon icon=I::EyeOff /> } } else { view! { <Icon icon=I::Eye /> } }}
                        </button>
                    </label>
                    {move || error.get().map(|e| view! { <p class="form__error" role="alert">{e}</p> })}
                    <button class="btn btn--primary btn--wide" type="submit" disabled=move || busy.get()>
                        <Show when=move || busy.get() fallback=|| view! { <span>"Unlock"</span> }>
                            <Spinner />
                        </Show>
                    </button>
                </form>
            </div>
        </div>
    }
}
