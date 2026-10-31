//! `/sign-in`: email → 6-digit code (the same email carries a link to `/app/auth/confirm`), or Google / Microsoft /
//! Apple when the project has them switched on. An existing account signs straight in; a new one is asked for its
//! invite code, which is checked (`invite_status`) before the code is sent, or bound to the email
//! (`reserve_invite`) before an OAuth round trip.

mod code;
mod email;
mod providers;

pub use providers::provider_icon;

use leptos::prelude::*;

use crate::components::icon::I;
use crate::components::logo::{Logo, LogoSize};
use crate::components::spot::Spot;
use crate::config::{PRIVACY_URL, TERMS_URL};
use code::CodeStep;
use email::EmailStep;
use providers::Providers;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Step {
    Email,
    /// `invite` is set when the code was sent for a new account (resending must send it again).
    Code {
        email: String,
        invite: Option<String>,
    },
}

/// What the email step and the provider buttons share.
#[derive(Clone, Copy)]
pub(super) struct Form {
    pub email: RwSignal<String>,
    pub invite: RwSignal<String>,
    /// The invite field is shown (no account yet, or "I have an invite").
    pub new_account: RwSignal<bool>,
    pub error: RwSignal<Option<String>>,
    /// A calm hint (not an error), e.g. "New here? Add your invite code".
    pub note: RwSignal<Option<String>>,
    pub busy: RwSignal<bool>,
}

#[component]
pub fn SignInPage() -> impl IntoView {
    let step = RwSignal::new(Step::Email);
    let form = Form {
        email: RwSignal::new(String::new()),
        invite: RwSignal::new(String::new()),
        new_account: RwSignal::new(false),
        error: RwSignal::new(None),
        note: RwSignal::new(None),
        busy: RwSignal::new(false),
    };
    view! {
        <div class="auth">
            <div class="auth__panel">
                {move || match step.get() {
                    Step::Email => view! {
                        <header class="auth__head">
                            <Logo size=LogoSize::Large tile=true />
                            <h1 class="auth__title">{t!("sign-in-title")}</h1>
                            <p class="auth__lead">{t!("sign-in-lead")}</p>
                        </header>
                        <EmailStep step=step form=form />
                        <Providers form=form />
                    }.into_any(),
                    Step::Code { email, invite } => view! {
                        <header class="auth__head">
                            <Spot icon=I::MailSent small=true />
                        </header>
                        <CodeStep email=email invite=invite step=step />
                    }.into_any(),
                }}
                <p class="auth__legal">
                    {t!("sign-in-legal-before")}
                    <a href=TERMS_URL target="_blank" rel="noopener">{t!("legal-terms")}</a>
                    {t!("sign-in-legal-and")}
                    <a href=PRIVACY_URL target="_blank" rel="noopener">{t!("legal-privacy")}</a>
                    {t!("sign-in-legal-after")}
                </p>
            </div>
        </div>
    }
}
