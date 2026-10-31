//! Coming back from outside the app: `/auth/callback` (Google / Microsoft / Apple, PKCE `?code=`, or an implicit
//! `#access_token=` fragment) and `/auth/confirm?token_hash=…&type=…` (the link in a sign-in email). Both finish the
//! sign-in and continue to the library, or to the screen that started the round trip.

use std::future::Future;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_query_map;

use crate::auth::{self, AuthError, Session, http, pkce};
use crate::components::icon::{I, Spinner};
use crate::components::spot::Empty;
use crate::state::use_app;
use crate::storage::prefs;
use crate::util::dom;

/// `access_token` and `refresh_token` from a `#a=1&b=2` fragment.
pub fn fragment_tokens(hash: &str) -> Option<(String, String, u32)> {
    let mut access = None;
    let mut refresh = None;
    let mut expires = 3600;
    for kv in hash.trim_start_matches('#').split('&') {
        match kv.split_once('=') {
            Some(("access_token", v)) => access = Some(v.to_owned()),
            Some(("refresh_token", v)) => refresh = Some(v.to_owned()),
            Some(("expires_in", v)) => expires = v.parse().unwrap_or(3600),
            _ => {}
        }
    }
    Some((access?, refresh?, expires))
}

/// Link types GoTrue verifies with a `token_hash`.
pub fn link_type(kind: &str) -> Option<&'static str> {
    ["magiclink", "signup", "email", "invite", "recovery", "email_change"].into_iter().find(|k| *k == kind)
}

async fn from_callback(code: Option<String>) -> Result<(), AuthError> {
    if let Some(code) = code {
        let verifier = pkce::take().ok_or_else(|| AuthError::network("missing verifier"))?;
        let session = http::exchange_code(&code, &verifier).await?;
        auth::establish(session).await;
        return Ok(());
    }
    let hash = dom::window().and_then(|w| w.location().hash().ok()).unwrap_or_default();
    let (access_token, refresh_token, expires_in) =
        fragment_tokens(&hash).ok_or_else(|| AuthError::network("no code"))?;
    let user = http::user(&access_token).await?;
    auth::establish(Session { access_token, refresh_token, expires_in, user }).await;
    Ok(())
}

async fn from_link(token_hash: Option<String>, kind: Option<String>) -> Result<(), AuthError> {
    let kind = kind.as_deref().and_then(link_type).unwrap_or("magiclink");
    let token_hash = token_hash.ok_or_else(|| AuthError::network("no token"))?;
    let session = http::verify_link(&token_hash, kind).await?;
    auth::establish(session).await;
    Ok(())
}

/// The screen shown while the sign-in finishes, and its error state.
#[component]
fn Returning<F>(work: F, provider_error: Option<String>) -> impl IntoView
where
    F: Future<Output = Result<(), AuthError>> + 'static,
{
    let app = use_app();
    let failed = RwSignal::new(None::<String>);
    spawn_local(async move {
        if let Some(e) = provider_error {
            failed.set(Some(e.replace('+', " ")));
            return;
        }
        match work.await {
            // Linking a sign-in method from Settings returns there; a fresh sign-in goes home (the guard may then
            // show the first-run screen).
            Ok(()) => {
                let to = prefs::get::<String>(prefs::AFTER_AUTH).unwrap_or_else(|| "/".to_owned());
                prefs::remove(prefs::AFTER_AUTH);
                app.nav.replace(&to);
            }
            Err(e) => failed.set(Some(e.friendly())),
        }
    });
    view! {
        <div class="auth">
            <div class="auth__panel">
                {move || match failed.get() {
                    None => view! {
                        <div class="center-stack" role="status">
                            <Spinner class="spin spin--lg" />
                            <p>{t!("auth-callback-busy")}</p>
                        </div>
                    }.into_any(),
                    Some(msg) => view! {
                        <Empty icon=I::Alert title=t!("auth-callback-failed") line=msg>
                            <button class="btn btn--primary" type="button" on:click=move |_| app.nav.replace("/sign-in")>
                                {t!("auth-callback-back")}
                            </button>
                        </Empty>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}

#[component]
pub fn AuthCallbackPage() -> impl IntoView {
    let q = use_query_map().get_untracked();
    let provider_error = q.get("error_description").or_else(|| q.get("error"));
    view! { <Returning work=from_callback(q.get("code")) provider_error=provider_error /> }
}

#[component]
pub fn AuthConfirmPage() -> impl IntoView {
    let q = use_query_map().get_untracked();
    let provider_error = q.get("error_description").or_else(|| q.get("error"));
    view! { <Returning work=from_link(q.get("token_hash"), q.get("type")) provider_error=provider_error /> }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_implicit_fragments() {
        assert_eq!(
            fragment_tokens("#access_token=a&expires_in=60&refresh_token=r&token_type=bearer"),
            Some(("a".into(), "r".into(), 60))
        );
        assert_eq!(fragment_tokens("#error=access_denied"), None);
    }

    #[test]
    fn only_known_link_types_are_verified() {
        assert_eq!(link_type("magiclink"), Some("magiclink"));
        assert_eq!(link_type("signup"), Some("signup"));
        assert_eq!(link_type("<script>"), None);
    }
}
