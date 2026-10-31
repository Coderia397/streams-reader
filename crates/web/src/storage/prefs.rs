//! Small preferences in localStorage (read synchronously at start-up, before any network call).

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::util::dom;

pub const SETTINGS: &str = "nolabs.settings";
pub const WELCOMED: &str = "nolabs.welcomed";
/// Route to open after an OAuth round trip that started inside the app (linking a sign-in method).
pub const AFTER_AUTH: &str = "nolabs.after_auth";

pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
    let text = dom::local_storage()?.get_item(key).ok().flatten()?;
    serde_json::from_str(&text).ok()
}

pub fn set<T: Serialize>(key: &str, value: &T) {
    if let (Some(storage), Ok(text)) = (dom::local_storage(), serde_json::to_string(value)) {
        let _ = storage.set_item(key, &text);
    }
}

pub fn remove(key: &str) {
    if let Some(storage) = dom::local_storage() {
        let _ = storage.remove_item(key);
    }
}
