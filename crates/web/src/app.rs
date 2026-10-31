//! Root component: app state, router (under [`BASE`]), the sign-in guard, and the layout around the routes
//! (mini-player, tab bar, toasts).

use std::rc::Rc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::path;

use crate::api::client;
use crate::auth;
use crate::components::logo::{Logo, LogoSize};
use crate::components::player::mini::MiniPlayer;
use crate::components::tabbar::TabBar;
use crate::components::toast_host::ToastHost;
use crate::config::BASE;
use crate::pages::NotFound;
use crate::pages::auth_callback::{AuthCallbackPage, AuthConfirmPage};
use crate::pages::library::LibraryPage;
use crate::pages::reader::ReaderPage;
use crate::pages::record::RecordPage;
use crate::pages::settings::SettingsPage;
use crate::pages::sign_in::SignInPage;
use crate::pages::voices::VoicesPage;
use crate::pages::welcome::WelcomePage;
use crate::state::session::Auth;
use crate::state::{AppState, use_app};
use crate::storage::prefs;

#[component]
pub fn App() -> impl IntoView {
    AppState::provide();
    view! {
        <Router base=BASE>
            <Shell />
        </Router>
    }
}

/// The route inside the app: `/app/voices` → `/voices`, `/app` → `/`.
pub fn route_of(pathname: &str) -> &str {
    match pathname.strip_prefix(BASE) {
        Some("") | None => "/",
        Some(rest) if rest.starts_with('/') => rest,
        Some(_) => "/",
    }
}

/// Screens that take the whole viewport (no tab bar, no mini-player).
fn is_immersive(route: &str) -> bool {
    route.starts_with("/read/")
        || route.starts_with("/auth/")
        || matches!(route, "/sign-in" | "/welcome" | "/voices/new")
}

/// Screens reachable while signed out.
fn is_public(route: &str) -> bool {
    route == "/sign-in" || route.starts_with("/auth/")
}

/// Where the guard sends someone on `route`, if anywhere.
pub fn redirect(route: &str, auth: Auth, welcomed: bool) -> Option<&'static str> {
    match auth {
        Auth::SignedOut if !is_public(route) => Some("/sign-in"),
        Auth::SignedIn if route == "/sign-in" => Some(if welcomed { "/" } else { "/welcome" }),
        Auth::SignedIn if route == "/" && !welcomed => Some("/welcome"),
        _ => None,
    }
}

#[component]
fn Shell() -> impl IntoView {
    let app = use_app();
    app.nav.install(use_navigate());
    client::on_auth_lost(Rc::new(move || {
        app.session.auth.set(Auth::SignedOut);
        app.player.pause();
    }));
    spawn_local(auth::start(app.session));

    let location = use_location();
    let route = Memo::new(move |_| route_of(&location.pathname.get()).to_owned());
    Effect::new(move |_| {
        let welcomed = prefs::get::<bool>(prefs::WELCOMED).unwrap_or(false);
        if let Some(to) = redirect(&route.get(), app.session.auth.get(), welcomed) {
            app.nav.replace(to);
        }
    });

    let immersive = move || is_immersive(&route.get());
    let checking = move || app.session.auth.get() == Auth::Checking && !is_public(&route.get());
    view! {
        <div class="app" class:app--immersive=immersive class:app--no-mini=move || !app.player.has_doc()>
            <Show when=move || !checking() fallback=|| view! { <div class="splash"><Logo size=LogoSize::Large tile=true /></div> }>
                <Show when=move || !immersive()>
                    <TabBar />
                </Show>
                <main class="main">
                    <Routes fallback=|| view! { <NotFound /> }>
                        <Route path=path!("/") view=LibraryPage />
                        <Route path=path!("/read/:id") view=ReaderPage />
                        <Route path=path!("/voices") view=VoicesPage />
                        <Route path=path!("/voices/new") view=RecordPage />
                        <Route path=path!("/settings") view=SettingsPage />
                        <Route path=path!("/sign-in") view=SignInPage />
                        <Route path=path!("/auth/callback") view=AuthCallbackPage />
                        <Route path=path!("/auth/confirm") view=AuthConfirmPage />
                        <Route path=path!("/welcome") view=WelcomePage />
                    </Routes>
                </main>
                <Show when=move || !immersive()>
                    <div class="chrome"><MiniPlayer /></div>
                </Show>
            </Show>
            <ToastHost />
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_strip_the_base() {
        assert_eq!(route_of("/app"), "/");
        assert_eq!(route_of("/app/"), "/");
        assert_eq!(route_of("/app/read/x"), "/read/x");
        assert_eq!(route_of("/apple"), "/");
        assert_eq!(route_of("/elsewhere"), "/");
    }

    #[test]
    fn immersive_routes() {
        assert!(is_immersive("/read/abc"));
        assert!(is_immersive("/sign-in"));
        assert!(is_immersive("/auth/callback"));
        assert!(is_immersive("/voices/new"));
        assert!(!is_immersive("/voices"));
        assert!(!is_immersive("/"));
    }

    #[test]
    fn guard() {
        assert_eq!(redirect("/", Auth::SignedOut, true), Some("/sign-in"));
        assert_eq!(redirect("/auth/callback", Auth::SignedOut, true), None);
        assert_eq!(redirect("/sign-in", Auth::SignedIn, false), Some("/welcome"));
        assert_eq!(redirect("/sign-in", Auth::SignedIn, true), Some("/"));
        assert_eq!(redirect("/", Auth::SignedIn, false), Some("/welcome"));
        assert_eq!(redirect("/settings", Auth::Offline, true), None);
        assert_eq!(redirect("/", Auth::Checking, true), None);
    }
}
