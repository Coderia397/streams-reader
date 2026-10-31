//! Settings → Account: email, sign-in methods (rows from `nolabs_core::auth::identity_rows`: link the enabled
//! providers that are not linked, unlink while another method remains), sign out here or, after a confirmation, on
//! every device. Deleting the account is not offered: the Worker has no route for it yet.

use leptos::prelude::*;
use leptos::task::spawn_local;
use nolabs_core::auth::{IdentityRow, identity_rows};

use crate::auth::types::AuthSettings;
use crate::auth::{self, Provider, http, pkce};
use crate::components::controls::{Group, InfoRow, NavRow};
use crate::components::icon::{I, Icon};
use crate::components::sheet::Sheet;
use crate::pages::sign_in::provider_icon;
use crate::state::session::Auth;
use crate::state::use_app;
use crate::storage::prefs;
use crate::util::dom;

fn method_name(p: Provider) -> String {
    match p {
        Provider::Email => t!("account-method-email"),
        other => other.label().to_owned(),
    }
}

async fn link(p: Provider) -> Result<(), auth::AuthError> {
    let token = auth::access_token().ok_or_else(|| auth::AuthError::network("signed out"))?;
    let challenge = pkce::begin().ok_or_else(|| auth::AuthError::network("no crypto"))?;
    let url = http::link_url(&token, p, &challenge).await?;
    prefs::set(prefs::AFTER_AUTH, &"/settings");
    if let Some(w) = dom::window() {
        let _ = w.location().set_href(&url);
    }
    Ok(())
}

#[component]
fn MethodRow(row: IdentityRow, on_unlink: Callback<String>, on_link: Callback<Provider>) -> impl IntoView {
    let p = row.provider;
    let detail = row.identity.as_ref().and_then(|i| i.email.clone());
    let linked = row.identity.is_some();
    let identity_id = row.identity.as_ref().map(|i| i.identity_id.clone()).unwrap_or_default();
    view! {
        <div class="row-item row-item--static">
            <span class="row-item__icon"><Icon icon=provider_icon(p) /></span>
            <span class="row-item__text">
                <span class="row-item__label">{method_name(p)}</span>
                <span class="row-item__hint">
                    {match (&detail, linked) {
                        (Some(email), true) => view! { <bdi dir="ltr">{email.clone()}</bdi> }.into_any(),
                        (None, true) => view! { <span>{t!("account-linked")}</span> }.into_any(),
                        _ => view! { <span>{t!("account-not-linked")}</span> }.into_any(),
                    }}
                </span>
            </span>
            {row.can_unlink.then(|| view! {
                <button class="btn btn--ghost btn--sm" type="button" on:click=move |_| on_unlink.run(identity_id.clone())>
                    <Icon icon=I::Unlink /><span>{t!("account-unlink")}</span>
                </button>
            })}
            {row.can_link.then(|| view! {
                <button class="btn btn--soft btn--sm" type="button" on:click=move |_| on_link.run(p)>
                    <Icon icon=I::Link /><span>{t!("account-link-short")}</span>
                </button>
            })}
        </div>
    }
}

#[component]
pub fn AccountGroup() -> impl IntoView {
    let app = use_app();
    let session = app.session;
    let settings = RwSignal::new(None::<AuthSettings>);
    spawn_local(async move {
        settings.set(http::settings().await.ok());
        let _ = auth::reload_user().await;
    });

    // Only methods that are linked or can be linked are listed; providers switched off in the project stay hidden.
    let rows = move || {
        let identities = session.user.with(|u| u.as_ref().map(|u| u.core_identities()).unwrap_or_default());
        let enabled = settings.with(|s| s.as_ref().map(AuthSettings::providers).unwrap_or_default());
        identity_rows(&identities, &enabled)
            .into_iter()
            .filter(|r| r.identity.is_some() || r.can_link)
            .collect::<Vec<_>>()
    };
    let on_unlink = Callback::new(move |identity_id: String| {
        spawn_local(async move {
            let Some(token) = auth::access_token() else { return };
            match http::unlink(&token, &identity_id).await {
                Ok(()) => {
                    let _ = auth::reload_user().await;
                    app.toasts.success(t!("account-unlinked"));
                }
                Err(e) => app.toasts.error(e.friendly()),
            }
        });
    });
    let on_link = Callback::new(move |p: Provider| {
        spawn_local(async move {
            if let Err(e) = link(p).await {
                app.toasts.error(e.friendly());
            }
        });
    });
    // Signing out every device cannot be undone from here, so it asks first; signing out this device does not.
    let confirm_all = RwSignal::new(false);
    let sign_out = move |everywhere: bool| {
        confirm_all.set(false);
        spawn_local(async move {
            app.player.close();
            auth::sign_out(everywhere).await;
        });
    };

    view! {
        <Group title=t!("settings-account")>
            <InfoRow icon=I::Account label=t!("account-email")
                value=Signal::derive(move || session.email().unwrap_or_else(|| {
                    if session.auth.get() == Auth::Offline { t!("account-offline") } else { "…".to_owned() }
                })) />
        </Group>
        <Group title=t!("account-methods")>
            {move || rows().into_iter().map(|row| view! { <MethodRow row=row on_unlink=on_unlink on_link=on_link /> }).collect_view()}
        </Group>
        <Group>
            <NavRow icon=I::SignOut label=t!("account-sign-out") on_click=Callback::new(move |()| sign_out(false)) />
            <NavRow icon=I::SignOutAll label=t!("account-sign-out-all") danger=true on_click=Callback::new(move |()| confirm_all.set(true)) />
        </Group>
        <Sheet open=confirm_all title=t!("account-sign-out-all-confirm")>
            <p class="sheet-lead">{t!("account-sign-out-all-line")}</p>
            <div class="btn-row">
                <button class="btn btn--secondary" type="button" on:click=move |_| confirm_all.set(false)>{t!("action-cancel")}</button>
                <button class="btn btn--danger" type="button" on:click=move |_| sign_out(true)>{t!("account-sign-out-all-action")}</button>
            </div>
        </Sheet>
    }
}
