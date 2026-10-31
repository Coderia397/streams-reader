//! App-wide reactive state, provided once as context. Every field is a cheap `Copy` handle over signals.

pub mod library;
pub mod nav;
pub mod player;
pub mod profile;
pub mod pwa;
pub mod session;
pub mod settings;
pub mod toast;
pub mod voices;

use leptos::prelude::*;

use library::LibraryState;
use nav::Nav;
use player::PlayerBridge;
use pwa::PwaState;
use session::SessionState;
use settings::Settings;
use toast::Toasts;
use voices::VoicesState;

#[derive(Clone, Copy)]
pub struct AppState {
    pub session: SessionState,
    pub settings: Settings,
    pub library: LibraryState,
    pub voices: VoicesState,
    pub toasts: Toasts,
    pub player: PlayerBridge,
    pub pwa: PwaState,
    pub nav: Nav,
}

impl AppState {
    /// Creates the state, starts background work and provides it as context.
    pub fn provide() -> AppState {
        let toasts = Toasts::new();
        let app = AppState {
            session: SessionState::new(),
            settings: Settings::load(),
            library: LibraryState::new(),
            voices: VoicesState::new(),
            toasts,
            player: PlayerBridge::new(),
            pwa: PwaState::start(toasts),
            nav: Nav::new(),
        };
        app.player.start_background(app);
        profile::start(app);
        provide_context(app);
        app
    }
}

/// The app state from context (provided by the root component).
pub fn use_app() -> AppState {
    expect_context::<AppState>()
}
