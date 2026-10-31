//! `/welcome`: first run. Install the app, and on Android keep playback going with the screen off (Android's battery
//! saver can pause Chrome in the background). Samsung's extra sleeping-apps lists are shown only on Samsung phones.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::icon::{I, Icon};
use crate::components::spot::Spot;
use crate::interop::pwa::{self, InstallOutcome};
use crate::state::use_app;
use crate::storage::prefs;
use crate::util::dom;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Samsung,
    Android,
    Other,
}

/// What kind of device a user agent string describes.
pub fn device(ua: &str) -> Device {
    if !ua.contains("Android") {
        return Device::Other;
    }
    if ua.contains("SamsungBrowser") || ua.contains("SM-") || ua.to_ascii_lowercase().contains("samsung") {
        Device::Samsung
    } else {
        Device::Android
    }
}

fn user_agent() -> String {
    dom::window().and_then(|w| w.navigator().user_agent().ok()).unwrap_or_default()
}

#[component]
fn Step(n: u32, #[prop(into)] title: String, children: Children) -> impl IntoView {
    view! {
        <section class="step">
            <span class="step__num" aria-hidden="true">{n}</span>
            <div class="step__body">
                <h2 class="step__title">{title}</h2>
                {children()}
            </div>
        </section>
    }
}

#[component]
pub fn WelcomePage() -> impl IntoView {
    let app = use_app();
    let device = device(&user_agent());
    let install = move |_| {
        spawn_local(async move {
            match pwa::install().await {
                InstallOutcome::Accepted => app.toasts.success(t!("welcome-installed-toast")),
                InstallOutcome::Dismissed => {}
                InstallOutcome::Unavailable => app.toasts.info(t!("welcome-install-manual")),
            }
            app.pwa.can_install.set(pwa::can_install());
            app.pwa.standalone.set(pwa::is_standalone());
        });
    };
    let finish = move |_| {
        prefs::set(prefs::WELCOMED, &true);
        app.nav.replace("/");
    };
    let android_steps = [t!("welcome-android-1"), t!("welcome-android-2"), t!("welcome-android-3")];
    let samsung_steps = [t!("welcome-samsung-1"), t!("welcome-samsung-2")];
    view! {
        <div class="page page--narrow welcome">
            <header class="welcome__hero">
                <Spot icon=I::Headphones />
                <h1 class="welcome__title">{t!("welcome-title")}</h1>
                <p class="welcome__lead">{t!("welcome-lead")}</p>
            </header>

            <Step n=1 title=t!("welcome-install-title")>
                {move || if app.pwa.standalone.get() {
                    view! { <p class="step__done"><Icon icon=I::CheckCircle />{t!("welcome-installed")}</p> }.into_any()
                } else if app.pwa.can_install.get() {
                    view! {
                        <p class="step__line">{t!("welcome-install-line")}</p>
                        <button class="btn btn--secondary" type="button" on:click=install>
                            <Icon icon=I::Download /><span>{t!("welcome-install")}</span>
                        </button>
                    }.into_any()
                } else {
                    view! { <p class="step__line">{t!("welcome-install-manual")}</p> }.into_any()
                }}
            </Step>

            {(device != Device::Other).then(|| view! {
                <Step n=2 title=t!("welcome-background-title")>
                    <p class="step__line">{t!("welcome-background-line")}</p>
                    <ol class="step__list">
                        {android_steps.into_iter().map(|s| view! { <li>{s}</li> }).collect_view()}
                    </ol>
                    {(device == Device::Samsung).then(|| view! {
                        <p class="step__line step__line--sub">{t!("welcome-samsung-line")}</p>
                        <ol class="step__list">
                            {samsung_steps.into_iter().map(|s| view! { <li>{s}</li> }).collect_view()}
                        </ol>
                    })}
                </Step>
            })}

            {(device == Device::Other).then(|| view! {
                <Step n=2 title=t!("welcome-keys-title")>
                    <p class="step__line">{t!("welcome-keys-line")}</p>
                </Step>
            })}

            <button class="btn btn--primary btn--block welcome__go" type="button" on:click=finish>
                {t!("welcome-start")}
            </button>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_from_user_agents() {
        let pixel = "Mozilla/5.0 (Linux; Android 15; Pixel 8) AppleWebKit/537.36 Chrome/130.0 Mobile Safari/537.36";
        let galaxy = "Mozilla/5.0 (Linux; Android 14; SM-S921B) AppleWebKit/537.36 Chrome/130.0 Mobile Safari/537.36";
        let mac = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/130.0 Safari/537.36";
        assert_eq!(device(pixel), Device::Android);
        assert_eq!(device(galaxy), Device::Samsung);
        assert_eq!(device(mac), Device::Other);
    }
}
