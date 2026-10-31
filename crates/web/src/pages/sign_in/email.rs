//! First step: the email address, and the invite code when the account does not exist yet.

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{Form, Step};
use crate::auth::types::SignUpData;
use crate::auth::{AuthError, AuthErrorKind, http};
use crate::components::icon::{I, Icon, Spinner};
use crate::db::rpc;
use crate::i18n;

/// Loose shape check; the server is the judge.
pub fn looks_like_email(s: &str) -> bool {
    let s = s.trim();
    let Some((local, domain)) = s.split_once('@') else { return false };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') && !s.contains(' ')
}

/// Invite codes are typed loosely (`STREAMS-1234`, spaces); the database normalises them the same way.
pub fn clean_invite(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace() && *c != '-').flat_map(char::to_lowercase).collect()
}

/// Sends the code: for an existing account when `invite` is `None`, else for a new account with that invite.
pub(super) async fn send(email: &str, invite: Option<&str>) -> Result<(), AuthError> {
    match invite {
        None => http::send_code(email, false, None).await,
        Some(code) => {
            let data = SignUpData { invite_code: Some(code.to_owned()), locale: i18n::current().code().to_owned() };
            http::send_code(email, true, Some(&data)).await
        }
    }
}

/// Checks the invite first, so a bad code is reported on this screen and no email is sent for it.
async fn submit(form: Form, step: RwSignal<Step>, email: String) {
    let invite = form.new_account.get_untracked().then(|| clean_invite(&form.invite.get_untracked()));
    if let Some(code) = &invite {
        if code.is_empty() {
            form.error.set(Some(t!("auth-invite-needed")));
            return;
        }
        match rpc::invite_status(code).await {
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
    match send(&email, invite.as_deref()).await {
        Ok(()) => step.set(Step::Code { email, invite }),
        Err(err) => {
            if matches!(err.kind, AuthErrorKind::NoAccount | AuthErrorKind::InviteRequired | AuthErrorKind::BadInvite) {
                form.new_account.set(true);
            }
            if err.kind == AuthErrorKind::NoAccount {
                form.note.set(Some(t!("sign-in-new-here")));
            } else {
                form.error.set(Some(err.friendly()));
            }
        }
    }
}

#[component]
pub(super) fn EmailStep(step: RwSignal<Step>, form: Form) -> impl IntoView {
    let on_submit = move || {
        let e = form.email.get_untracked().trim().to_owned();
        if !looks_like_email(&e) {
            form.error.set(Some(t!("sign-in-email-invalid")));
            return;
        }
        if form.busy.get_untracked() {
            return;
        }
        form.busy.set(true);
        form.error.set(None);
        form.note.set(None);
        spawn_local(async move {
            submit(form, step, e).await;
            form.busy.set(false);
        });
    };
    view! {
        <form class="form" novalidate=true on:submit=move |ev| { ev.prevent_default(); on_submit(); }>
            <label class="field">
                <span class="field__label">{t!("sign-in-email-label")}</span>
                <input class="input" type="email" name="email" autocomplete="email" inputmode="email" dir="ltr"
                    autocapitalize="off" spellcheck="false" required=true placeholder=t!("sign-in-email-placeholder")
                    prop:value=move || form.email.get()
                    on:input=move |ev| form.email.set(event_target_value(&ev)) />
            </label>
            <Show when=move || form.new_account.get()>
                <label class="field field--reveal">
                    <span class="field__label">{t!("sign-in-invite-label")}</span>
                    <input class="input input--mono" type="text" name="invite" autocomplete="off" autocapitalize="off"
                        spellcheck="false" dir="ltr" placeholder=t!("sign-in-invite-placeholder")
                        prop:value=move || form.invite.get()
                        on:input=move |ev| form.invite.set(event_target_value(&ev)) />
                    <span class="field__hint">{t!("sign-in-invite-hint")}</span>
                </label>
            </Show>
            {move || form.note.get().map(|n| view! { <p class="form__note" role="status"><Icon icon=I::Info />{n}</p> })}
            {move || form.error.get().map(|e| view! { <p class="form__error" role="alert"><Icon icon=I::Alert />{e}</p> })}
            <button class="btn btn--primary btn--block btn--lg" type="submit" disabled=move || form.busy.get()>
                <Show when=move || form.busy.get() fallback=|| view! { <span>{t!("sign-in-continue")}</span> }>
                    <Spinner />
                </Show>
            </button>
            <Show when=move || !form.new_account.get()>
                <button class="link-btn" type="button" on:click=move |_| form.new_account.set(true)>
                    {t!("sign-in-have-invite")}
                </button>
            </Show>
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_shapes() {
        assert!(looks_like_email(" a@b.co "));
        assert!(!looks_like_email("a@b"));
        assert!(!looks_like_email("@b.co"));
        assert!(!looks_like_email("a b@c.de"));
        assert!(!looks_like_email("a@.de"));
    }

    #[test]
    fn invites_are_typed_loosely() {
        assert_eq!(clean_invite(" AB12-cd34 ef "), "ab12cd34ef");
    }
}
