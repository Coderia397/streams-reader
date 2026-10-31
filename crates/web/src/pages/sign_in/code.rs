//! Second step: the 6-digit code from the email, with resend (after a cooldown) and "use another email".

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::Step;
use super::email::send;
use crate::auth::{self, http};
use crate::components::icon::{I, Icon, Spinner};

const CODE_LEN: usize = 6;
const RESEND_AFTER_S: u32 = 45;

/// Keeps the digits of a typed or pasted code, at most six.
pub fn clean_code(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).take(CODE_LEN).collect()
}

#[component]
pub(super) fn CodeStep(email: String, invite: Option<String>, step: RwSignal<Step>) -> impl IntoView {
    let code = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let wait = RwSignal::new(RESEND_AFTER_S);
    let tick = set_interval_with_handle(move || wait.update(|w| *w = w.saturating_sub(1)), Duration::from_secs(1)).ok();
    on_cleanup(move || {
        if let Some(t) = tick {
            t.clear();
        }
    });
    let email = StoredValue::new(email);
    let invite = StoredValue::new(invite);
    let verify = move || {
        let c = code.get_untracked();
        if c.len() != CODE_LEN || busy.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match http::verify_code(&email.get_value(), &c).await {
                Ok(session) => auth::establish(session).await,
                Err(e) => {
                    error.set(Some(e.friendly()));
                    code.set(String::new());
                }
            }
            busy.set(false);
        });
    };
    let resend = move |_: leptos::ev::MouseEvent| {
        wait.set(RESEND_AFTER_S);
        error.set(None);
        spawn_local(async move {
            match send(&email.get_value(), invite.get_value().as_deref()).await {
                Ok(()) => {}
                Err(e) => error.set(Some(e.friendly())),
            }
        });
    };
    view! {
        <form class="form" on:submit=move |ev| { ev.prevent_default(); verify(); }>
            <div class="auth__sent">
                <h1 class="auth__title">{t!("sign-in-code-title")}</h1>
                <p class="auth__lead">{t!("sign-in-code-sent-before")}<bdi class="auth__email" dir="ltr">{email.get_value()}</bdi>{t!("sign-in-code-sent-after")}</p>
            </div>
            <label class="field">
                <span class="visually-hidden">{t!("sign-in-code-label")}</span>
                <input class="input input--code" type="text" inputmode="numeric" autocomplete="one-time-code" dir="ltr"
                    maxlength="12" placeholder="000000" autofocus=true
                    prop:value=move || code.get()
                    on:input=move |ev| {
                        code.set(clean_code(&event_target_value(&ev)));
                        if code.get_untracked().len() == CODE_LEN { verify(); }
                    } />
            </label>
            {move || error.get().map(|e| view! { <p class="form__error" role="alert"><Icon icon=I::Alert />{e}</p> })}
            <button class="btn btn--primary btn--block btn--lg" type="submit" disabled=move || busy.get() || code.get().len() != CODE_LEN>
                <Show when=move || busy.get() fallback=|| view! { <span>{t!("sign-in-verify")}</span> }>
                    <Spinner />
                </Show>
            </button>
            <p class="form__hint">{t!("sign-in-link-hint")}</p>
            <div class="form__links">
                <button class="btn btn--ghost btn--sm" type="button" disabled={move || wait.get() > 0} on:click=resend>
                    {move || if wait.get() > 0 { t!("sign-in-resend-wait", seconds = wait.get()) } else { t!("sign-in-resend") }}
                </button>
                <button class="btn btn--ghost btn--sm" type="button" on:click=move |_| step.set(Step::Email)>
                    {t!("sign-in-other-email")}
                </button>
            </div>
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_keep_six_digits() {
        assert_eq!(clean_code("123 456"), "123456");
        assert_eq!(clean_code("12-34-56-78"), "123456");
        assert_eq!(clean_code("abc"), "");
    }
}
