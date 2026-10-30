//! `/welcome`: first run. Install the app and stop Samsung's battery manager from pausing playback with the screen
//! off (One UI puts Chrome to sleep otherwise).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::icons::{I, Icon};
use crate::components::logo::Logo;
use crate::config::APP_NAME;
use crate::interop::pwa::{self, InstallOutcome};
use crate::state::use_app;
use crate::storage::prefs;

const BATTERY_STEPS: [&str; 3] = [
    "Open Settings › Apps › Chrome › Battery and choose Unrestricted.",
    "Open Settings › Battery › Background usage limits.",
    "Remove Chrome from Sleeping apps and from Deep sleeping apps.",
];

#[component]
pub fn WelcomePage() -> impl IntoView {
    let app = use_app();
    let install = move |_| {
        spawn_local(async move {
            match pwa::install().await {
                InstallOutcome::Accepted => app.toasts.success("Installed. Open it from your home screen."),
                InstallOutcome::Dismissed => {}
                InstallOutcome::Unavailable => app.toasts.info("Use Chrome's menu, then Add to Home screen."),
            }
            app.pwa.can_install.set(pwa::can_install());
            app.pwa.standalone.set(pwa::is_standalone());
        });
    };
    let finish = move |_| {
        prefs::set(prefs::WELCOMED, &true);
        app.nav.replace("/");
    };
    view! {
        <div class="page welcome">
            <header class="welcome__hero">
                <Logo large=true />
                <h1 class="welcome__title">{format!("Welcome to {APP_NAME}")}</h1>
                <p class="welcome__lead">"Two quick things so it plays reliably, even with the screen off."</p>
            </header>

            <section class="step">
                <span class="step__num">"1"</span>
                <div class="step__body">
                    <h2 class="step__title">"Add it to your home screen"</h2>
                    {move || if app.pwa.standalone.get() {
                        view! { <p class="step__done"><Icon icon=I::Check />"Installed"</p> }.into_any()
                    } else if app.pwa.can_install.get() {
                        view! {
                            <p class="step__line">"It opens full screen and shows up in your media controls."</p>
                            <button class="btn btn--primary" type="button" on:click=install>
                                <Icon icon=I::Download /><span>"Install"</span>
                            </button>
                        }.into_any()
                    } else {
                        view! { <p class="step__line">"In Chrome, tap the ⋮ menu, then Add to Home screen (or Install app)."</p> }.into_any()
                    }}
                </div>
            </section>

            <section class="step">
                <span class="step__num">"2"</span>
                <div class="step__body">
                    <h2 class="step__title">"Let it play in the background"</h2>
                    <p class="step__line">"On Samsung phones, battery saving can stop audio a few minutes after the screen turns off."</p>
                    <ol class="step__list">
                        {BATTERY_STEPS.iter().map(|s| view! { <li><Icon icon=I::Battery />{*s}</li> }).collect_view()}
                    </ol>
                </div>
            </section>

            <button class="btn btn--primary btn--wide welcome__go" type="button" on:click=finish>"Start listening"</button>
        </div>
    }
}
