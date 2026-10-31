//! Session commands. Rule: inside the app the core is the only holder of the Supabase refresh token and the only
//! refresher. Supabase rotates refresh tokens and revokes a token family on reuse, so two refreshers (webview and
//! core) would sign the user out at random; the core also needs fresh tokens while the webview sleeps.
//!
//! Flow:
//! 1. The UI signs in exactly as on the web (email code via `verifyOtp`, magic link, or OAuth with PKCE). For OAuth
//!    it calls `auth_open_browser(authorize URL with redirect_to=nolabs://auth/callback)`; the system browser
//!    returns through the deep link, the core emits `streams:auth-callback { url }`, and the UI (which holds the
//!    PKCE verifier) exchanges the code.
//! 2. The UI calls `auth_store(NewSession)`: refresh token → keystore, access token kept in memory here.
//! 3. Afterwards the UI asks `auth_session` (refreshed here when < 60 s from expiry) and `auth_refresh` after a 401.
//!    The core refreshes with `POST {SUPABASE_URL}/auth/v1/token?grant_type=refresh_token` (publishable key).
//! 4. `auth_sign_out`: `POST /auth/v1/logout`, delete the keystore entry, stop feeder and downloads.
//!
//! Redirect URLs already allowed in Supabase (note 17): `nolabs://auth`, `nolabs://auth/**`, `tauri://localhost`.
//!
//! TODO(phase 1): implement with `crate::secrets` and a `Mutex<Option<Session>>` in `AppState`.

use streams_native_bridge::{BridgeError, ErrorCode, NewSession, SessionView};
use tauri::State;
use tauri_plugin_opener::OpenerExt;

use super::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub fn auth_store(_state: State<'_, AppState>, session: NewSession) -> CmdResult<SessionView> {
    let _ = session;
    Err(BridgeError::unsupported("keystore session (phase 1)"))
}

#[tauri::command]
pub fn auth_session(_state: State<'_, AppState>) -> CmdResult<Option<SessionView>> {
    Err(BridgeError::unsupported("keystore session (phase 1)"))
}

#[tauri::command]
pub fn auth_refresh(_state: State<'_, AppState>) -> CmdResult<SessionView> {
    Err(BridgeError::new(ErrorCode::NotSignedIn, "no stored session"))
}

#[tauri::command]
pub fn auth_sign_out(_state: State<'_, AppState>) -> CmdResult<()> {
    Ok(())
}

/// Only Supabase's authorize endpoint may be opened, so the webview cannot open arbitrary URLs through this.
#[tauri::command]
pub fn auth_open_browser(app: tauri::AppHandle, url: String) -> CmdResult<()> {
    const ALLOWED_PREFIX: &str = "https://atxbmgfuxjtcwmoaldqy.supabase.co/auth/v1/authorize?";
    if !url.starts_with(ALLOWED_PREFIX) {
        return Err(BridgeError::new(ErrorCode::Invalid, "not a Supabase authorize URL"));
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| BridgeError::internal(e.to_string()))
}
