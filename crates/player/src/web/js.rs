//! Bindings to the player's JS glue in `interop/`.

use wasm_bindgen::prelude::*;
use web_sys::{HtmlAudioElement, HtmlMediaElement};

#[wasm_bindgen(module = "/interop/mse.js")]
extern "C" {
    #[wasm_bindgen(js_name = mseSupported)]
    pub fn mse_supported(mime: &str) -> bool;

    #[derive(Debug, Clone)]
    pub type MseSession;

    /// Resolves with an [`MseSession`].
    #[wasm_bindgen(static_method_of = MseSession, js_name = open)]
    pub fn open(media: &HtmlMediaElement, mime: &str) -> js_sys::Promise;
    /// Resolves with the media time where the appended data ends.
    #[wasm_bindgen(method)]
    pub fn append(this: &MseSession, bytes: &js_sys::Uint8Array) -> js_sys::Promise;
    #[wasm_bindgen(method)]
    pub fn remove(this: &MseSession, start: f64, end: f64) -> js_sys::Promise;
    #[wasm_bindgen(method)]
    pub fn buffered(this: &MseSession) -> Vec<f64>;
    #[wasm_bindgen(method)]
    pub fn finish(this: &MseSession);
    #[wasm_bindgen(method)]
    pub fn close(this: &MseSession);
}

#[wasm_bindgen(module = "/interop/dom.js")]
extern "C" {
    #[wasm_bindgen(js_name = createHiddenAudio, catch)]
    pub fn create_hidden_audio() -> Result<HtmlAudioElement, JsValue>;
    /// Resolves with "" or the error name.
    #[wasm_bindgen(js_name = playMedia)]
    pub fn play_media(media: &HtmlMediaElement) -> js_sys::Promise;
    /// Resolves like [`play_media`].
    #[wasm_bindgen(js_name = switchSource)]
    pub fn switch_source(media: &HtmlMediaElement, url: &str, offset: f64, autoplay: bool) -> js_sys::Promise;
    #[wasm_bindgen(js_name = cancelSwitch)]
    pub fn cancel_switch(media: &HtmlMediaElement);
    /// NaN when no switch is pending.
    #[wasm_bindgen(js_name = pendingOffset)]
    pub fn pending_offset(media: &HtmlMediaElement) -> f64;
    /// False when no switch is pending.
    #[wasm_bindgen(js_name = retargetSwitch)]
    pub fn retarget_switch(media: &HtmlMediaElement, offset: f64) -> bool;
    #[wasm_bindgen(js_name = setSwitchAutoplay)]
    pub fn set_switch_autoplay(media: &HtmlMediaElement, autoplay: bool);
    #[wasm_bindgen(js_name = detachMedia)]
    pub fn detach_media(media: &HtmlMediaElement);
    #[wasm_bindgen(js_name = blobUrl, catch)]
    pub fn blob_url(bytes: &js_sys::Uint8Array, mime: &str) -> Result<String, JsValue>;
    #[wasm_bindgen(js_name = revokeUrl)]
    pub fn revoke_url(url: &str);
    #[wasm_bindgen(js_name = delay)]
    fn delay_promise(ms: u32) -> js_sys::Promise;
    #[wasm_bindgen(js_name = errorName)]
    pub fn error_name(err: &JsValue) -> String;
    #[wasm_bindgen(js_name = errorMessage)]
    pub fn error_message(err: &JsValue) -> String;
    #[wasm_bindgen(js_name = mediaErrorMessage)]
    pub fn media_error_message(media: &HtmlMediaElement) -> String;
}

#[wasm_bindgen(module = "/interop/media_session.js")]
extern "C" {
    #[wasm_bindgen(js_name = setMetadata)]
    pub fn session_set_metadata(title: &str, artist: &str, album: &str, artwork_url: Option<String>);
    #[wasm_bindgen(js_name = clearMetadata)]
    pub fn session_clear();
    #[wasm_bindgen(js_name = setPlaybackState)]
    pub fn session_set_playback_state(state: &str);
    #[wasm_bindgen(js_name = setPositionState)]
    pub fn session_set_position(duration: f64, position: f64, rate: f64);
    #[wasm_bindgen(js_name = clearPositionState)]
    pub fn session_clear_position();
    #[wasm_bindgen(js_name = setActionHandler)]
    pub fn session_set_action_handler(action: &str, handler: Option<&js_sys::Function>) -> bool;
}

/// Waits `ms` milliseconds.
pub async fn delay(ms: u32) {
    // `delay` never rejects.
    let _ = wasm_bindgen_futures::JsFuture::from(delay_promise(ms)).await;
}

/// Awaits a promise from [`play_media`] or [`switch_source`]: Ok on success, Err(error name) otherwise.
pub async fn play_outcome(promise: js_sys::Promise) -> Result<(), String> {
    let name = wasm_bindgen_futures::JsFuture::from(promise).await.ok().and_then(|v| v.as_string()).unwrap_or_default();
    if name.is_empty() { Ok(()) } else { Err(name) }
}
