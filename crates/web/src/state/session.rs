//! Signed-in state, driven by `crate::auth`. Starts as `Checking` while the stored session is restored.

use leptos::prelude::*;

use crate::auth::User;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    Checking,
    SignedIn,
    SignedOut,
    /// A session is stored but Supabase could not be reached; the app works from local data where it can.
    Offline,
}

#[derive(Clone, Copy)]
pub struct SessionState {
    pub auth: RwSignal<Auth>,
    pub user: RwSignal<Option<User>>,
}

impl SessionState {
    pub fn new() -> SessionState {
        SessionState { auth: RwSignal::new(Auth::Checking), user: RwSignal::new(None) }
    }

    /// The account's email, for the settings screen.
    pub fn email(&self) -> Option<String> {
        self.user.with(|u| u.as_ref().and_then(|u| u.email.clone()))
    }
}
