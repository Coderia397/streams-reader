//! Root component: app state, router, and the layout around the routes (mini-player, tab bar, toasts).

use std::rc::Rc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::path;

use crate::api::{client, session};
use crate::components::player::mini::MiniPlayer;
use crate::components::tabbar::TabBar;
use crate::components::toast_host::ToastHost;
use crate::pages::NotFound;
use crate::pages::library::LibraryPage;
use crate::pages::login::LoginPage;
use crate::pages::reader::ReaderPage;
use crate::pages::record::RecordPage;
use crate::pages::settings::SettingsPage;
use crate::pages::voices::VoicesPage;
use crate::pages::welcome::WelcomePage;
use crate::state::session::Auth;
use crate::state::{AppState, use_app};
use crate::storage::prefs;

#[component]
pub fn App() -> impl IntoView {
    AppState::provide();
    view! {
        <Router>
            <Shell />
        </Router>
    }
}

/// Screens that take the whole viewport (no tab bar, no mini-player).
fn is_immersive(path: &str) -> bool {
    path.starts_with("/read/") || path == "/login" || path == "/welcome" || path == "/voices/new"
}

#[component]
fn Shell() -> impl IntoView {
    let app = use_app();
    app.nav.install(use_navigate());
    client::on_auth_lost(Rc::new(move || {
        app.session.auth.set(Auth::SignedOut);
        app.player.pause();
    }));

    spawn_local(async move {
        let auth = match session::info().await {
            Ok(_) => Auth::SignedIn,
            Err(e) if e.is_unauthorized() => Auth::SignedOut,
            Err(_) => Auth::Offline,
        };
        app.session.auth.set(auth);
    });

    let location = use_location();
    // Route guard: signed out → /login; signed in on /login → library (or first-run setup).
    Effect::new(move |_| {
        let path = location.pathname.get();
        match app.session.auth.get() {
            Auth::SignedOut if path != "/login" => app.nav.replace("/login"),
            Auth::SignedIn if path == "/login" => app.nav.replace("/"),
            Auth::SignedIn if path == "/" && !prefs::get::<bool>(prefs::WELCOMED).unwrap_or(false) => {
                app.nav.replace("/welcome");
            }
            _ => {}
        }
    });

    let immersive = move || is_immersive(&location.pathname.get());
    view! {
        <div class="app" class:app--immersive=immersive>
            <main class="main">
                <Routes fallback=|| view! { <NotFound /> }>
                    <Route path=path!("/") view=LibraryPage />
                    <Route path=path!("/read/:id") view=ReaderPage />
                    <Route path=path!("/voices") view=VoicesPage />
                    <Route path=path!("/voices/new") view=RecordPage />
                    <Route path=path!("/settings") view=SettingsPage />
                    <Route path=path!("/login") view=LoginPage />
                    <Route path=path!("/welcome") view=WelcomePage />
                </Routes>
            </main>
            <Show when=move || !immersive()>
                <div class="chrome">
                    <MiniPlayer />
                    <TabBar />
                </div>
            </Show>
            <ToastHost />
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immersive_routes() {
        assert!(is_immersive("/read/abc"));
        assert!(is_immersive("/login"));
        assert!(is_immersive("/voices/new"));
        assert!(!is_immersive("/voices"));
        assert!(!is_immersive("/"));
    }
}
