//! Accounts through Supabase Auth: email code (with a magic-link fallback), Google / Microsoft / Apple with PKCE,
//! and the session. The access token lives in memory only; the refresh token is kept in IndexedDB so the app stays
//! signed in across launches. Tokens are refreshed a minute before they expire, when the page becomes visible
//! again, and once on any 401 from the API (see `api::client`).
//!
//! Cross-device sync (Supabase Realtime) is phase 2: [`on_signed_in`] is the hook where it will subscribe.

pub mod error;
pub mod http;
pub mod pkce;
pub mod types;

use std::cell::RefCell;
use std::time::Duration;

use futures::FutureExt;
use futures::future::{LocalBoxFuture, Shared};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::state::session::{Auth, SessionState};
use crate::storage::idb;
use crate::util::timer::now_ms;
pub use error::{AuthError, AuthErrorKind};
pub use types::{Provider, Session, User};

const REFRESH_KEY: &str = "refresh";
/// Refresh this long before the access token expires.
const REFRESH_EARLY_MS: i64 = 60_000;

struct Tokens {
    access: String,
    expires_at_ms: i64,
}

type RefreshFuture = Shared<LocalBoxFuture<'static, Result<(), AuthError>>>;

thread_local! {
    static TOKENS: RefCell<Option<Tokens>> = const { RefCell::new(None) };
    static STATE: RefCell<Option<SessionState>> = const { RefCell::new(None) };
    static REFRESHING: RefCell<Option<RefreshFuture>> = const { RefCell::new(None) };
    static TIMER: RefCell<Option<TimeoutHandle>> = const { RefCell::new(None) };
}

/// The current access token, if signed in.
pub fn access_token() -> Option<String> {
    TOKENS.with(|t| t.borrow().as_ref().map(|t| t.access.clone()))
}

fn state() -> Option<SessionState> {
    STATE.with(|s| *s.borrow())
}

fn set_auth(auth: Auth) {
    if let Some(s) = state() {
        s.auth.set(auth);
    }
}

/// Hook for phase 2 (cross-device sync over Supabase Realtime): runs after every sign-in and token refresh.
fn on_signed_in(_user: &User) {}

fn schedule_refresh(expires_in_s: u32) {
    let delay = (i64::from(expires_in_s) * 1000 - REFRESH_EARLY_MS).max(5_000);
    let handle = set_timeout_with_handle(
        || {
            spawn_local(async {
                let _ = refresh_now().await;
            })
        },
        Duration::from_millis(delay as u64),
    )
    .ok();
    TIMER.with(|t| {
        if let Some(old) = t.borrow_mut().take() {
            old.clear();
        }
        *t.borrow_mut() = handle;
    });
}

/// Keeps a new session: token in memory, refresh token on disk, user in the app state, refresh timer.
pub async fn establish(session: Session) {
    TOKENS.with(|t| {
        *t.borrow_mut() = Some(Tokens {
            access: session.access_token.clone(),
            expires_at_ms: now_ms() + i64::from(session.expires_in) * 1000,
        })
    });
    let _ = idb::put(idb::AUTH, REFRESH_KEY, &session.refresh_token).await;
    schedule_refresh(session.expires_in);
    on_signed_in(&session.user);
    if let Some(s) = state() {
        s.user.set(Some(session.user));
        s.auth.set(Auth::SignedIn);
    }
}

async fn forget() {
    TOKENS.with(|t| *t.borrow_mut() = None);
    TIMER.with(|t| {
        if let Some(old) = t.borrow_mut().take() {
            old.clear();
        }
    });
    crate::api::audio::forget();
    let _ = idb::delete(idb::AUTH, REFRESH_KEY).await;
    if let Some(s) = state() {
        s.user.set(None);
    }
}

async fn do_refresh() -> Result<(), AuthError> {
    let stored: Option<String> = idb::get(idb::AUTH, REFRESH_KEY).await.ok().flatten();
    let Some(token) = stored else {
        set_auth(Auth::SignedOut);
        return Err(AuthError { kind: AuthErrorKind::SessionGone, status: 0, message: "no session".into() });
    };
    match http::refresh(&token).await {
        Ok(session) => {
            establish(session).await;
            Ok(())
        }
        Err(e) if e.kind == AuthErrorKind::Network => {
            // Keep the refresh token: the app works offline and retries when it can.
            if access_token().is_none() {
                set_auth(Auth::Offline);
            }
            Err(e)
        }
        Err(e) => {
            if matches!(e.kind, AuthErrorKind::SessionGone) || e.status == 400 || e.status == 401 {
                forget().await;
                set_auth(Auth::SignedOut);
            }
            Err(e)
        }
    }
}

/// Refreshes the session; concurrent callers share one request.
pub async fn refresh_now() -> Result<(), AuthError> {
    let fut = REFRESHING.with(|r| {
        r.borrow_mut()
            .get_or_insert_with(|| {
                async {
                    let out = do_refresh().await;
                    REFRESHING.with(|r| *r.borrow_mut() = None);
                    out
                }
                .boxed_local()
                .shared()
            })
            .clone()
    });
    fut.await
}

/// Restores the session at start-up (called once by the app root).
pub async fn start(state: SessionState) {
    STATE.with(|s| *s.borrow_mut() = Some(state));
    watch_visibility();
    let _ = refresh_now().await;
}

/// Phones throttle timers in the background; refresh on return when the token is about to lapse.
fn watch_visibility() {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::Closure;
    let Some(doc) = crate::util::dom::document() else { return };
    let cb = Closure::<dyn Fn()>::new(|| {
        if crate::util::dom::is_hidden() {
            return;
        }
        let due = TOKENS.with(|t| t.borrow().as_ref().is_some_and(|t| t.expires_at_ms - now_ms() < REFRESH_EARLY_MS));
        if due {
            spawn_local(async {
                let _ = refresh_now().await;
            });
        }
    })
    .into_js_value();
    let _ = doc.add_event_listener_with_callback("visibilitychange", cb.unchecked_ref());
}

/// Signs out on this device, or on every device with `everywhere`. Local state is cleared even when offline.
pub async fn sign_out(everywhere: bool) {
    if let Some(token) = access_token() {
        let _ = http::logout(&token, if everywhere { "global" } else { "local" }).await;
    }
    forget().await;
    set_auth(Auth::SignedOut);
}

/// Re-reads the user (after linking or unlinking a sign-in method).
pub async fn reload_user() -> Result<(), AuthError> {
    let token = access_token().ok_or_else(|| AuthError::network("signed out"))?;
    let user = http::user(&token).await?;
    if let Some(s) = state() {
        s.user.set(Some(user));
    }
    Ok(())
}
