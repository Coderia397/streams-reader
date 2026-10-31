//! The Supabase refresh token in the OS keystore. Only the core reads it; the UI only ever sees access tokens.
//!
//! - macOS: Keychain (`apple-native-keyring-store`), Windows: Credential Manager (`windows-native-keyring-store`),
//!   Linux: Secret Service over D-Bus (`zbus-secret-service-keyring-store`), all behind `keyring-core` 1.0.
//! - Android: AES-GCM with a Keystore key, in the Kotlin plugin (`SecretStore.kt`).
//! - Linux without a Secret Service (bare window managers, the old laptop): fall back to a 0600 file under the app
//!   data dir and say so in Settings (assumed acceptable; the token alone cannot read other users' data).
//!
//! Service name `watch.streams.reader`, account `supabase-refresh`.
//!
//! TODO(phase 1): set the default `keyring_core` store per platform at start-up, implement get/set/delete, and the
//! Linux file fallback.

use streams_native_bridge::BridgeError;
use tauri::{AppHandle, Runtime};

pub const SERVICE: &str = "watch.streams.reader";
pub const REFRESH_ACCOUNT: &str = "supabase-refresh";

pub struct Secrets {
    _private: (),
}

impl Secrets {
    pub fn new<R: Runtime>(_app: &AppHandle<R>) -> Self {
        Self { _private: () }
    }

    pub fn get(&self, _account: &str) -> Result<Option<String>, BridgeError> {
        Err(BridgeError::unsupported("keystore (phase 1)"))
    }

    pub fn set(&self, _account: &str, _value: &str) -> Result<(), BridgeError> {
        Err(BridgeError::unsupported("keystore (phase 1)"))
    }

    pub fn delete(&self, _account: &str) -> Result<(), BridgeError> {
        Err(BridgeError::unsupported("keystore (phase 1)"))
    }
}
