//! Signed-in state. Checked once at start-up (`GET /api/session`); any 401 later flips it to signed out.

use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    Checking,
    SignedIn,
    SignedOut,
    /// No answer from the Worker; the app works from local data where it can.
    Offline,
}

#[derive(Clone, Copy)]
pub struct SessionState {
    pub auth: RwSignal<Auth>,
}

impl SessionState {
    pub fn new() -> SessionState {
        SessionState { auth: RwSignal::new(Auth::Checking) }
    }
}
