//! Installability and service worker updates.

use leptos::prelude::*;

use super::toast::Toasts;
use crate::interop::pwa;

#[derive(Clone, Copy)]
pub struct PwaState {
    pub can_install: RwSignal<bool>,
    pub standalone: RwSignal<bool>,
}

impl PwaState {
    pub fn start(toasts: Toasts) -> PwaState {
        let s = PwaState {
            can_install: RwSignal::new(pwa::can_install()),
            standalone: RwSignal::new(pwa::is_standalone()),
        };
        pwa::watch_install(move || {
            s.can_install.set(pwa::can_install());
            s.standalone.set(pwa::is_standalone());
        });
        pwa::register(move || toasts.update_ready());
        s
    }
}
