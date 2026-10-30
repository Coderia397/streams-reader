//! Voice previews: one short clip at a time through a plain `<audio>` element with a blob URL, separate from the
//! document player (which is paused while a preview plays).

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

use crate::provider::oneshot;
use crate::state::AppState;

#[derive(Clone, Copy)]
pub struct Preview {
    /// Voice id being rendered.
    pub loading: RwSignal<Option<String>>,
    /// Voice id playing.
    pub playing: RwSignal<Option<String>>,
    audio: StoredValue<Option<(web_sys::HtmlAudioElement, String)>, LocalStorage>,
}

impl Preview {
    pub fn new() -> Preview {
        let p =
            Preview { loading: RwSignal::new(None), playing: RwSignal::new(None), audio: StoredValue::new_local(None) };
        on_cleanup(move || p.stop());
        p
    }

    pub fn stop(&self) {
        if let Some((audio, url)) = self.audio.get_value() {
            let _ = audio.pause();
            let _ = web_sys::Url::revoke_object_url(&url);
        }
        self.audio.set_value(None);
        self.playing.set(None);
    }

    /// Plays the preview for `voice`, or stops it when it is already playing.
    pub fn toggle(self, app: AppState, voice: String, preview_key: Option<String>) {
        if self.playing.get_untracked().as_deref() == Some(voice.as_str()) {
            self.stop();
            return;
        }
        if self.loading.get_untracked().is_some() {
            return;
        }
        self.stop();
        self.loading.set(Some(voice.clone()));
        spawn_local(async move {
            let result = oneshot::preview(&voice, preview_key.as_deref()).await;
            self.loading.set(None);
            match result {
                Ok(bytes) => self.play_bytes(app, voice, &bytes),
                Err(e) => app.toasts.api_error(&e),
            }
        });
    }

    fn play_bytes(self, app: AppState, voice: String, bytes: &js_sys::Uint8Array) {
        let parts = js_sys::Array::of1(bytes);
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type("audio/webm");
        let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts) else { return };
        let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
        let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&url) else { return };
        let ended = Closure::once_into_js(move || self.stop());
        audio.set_onended(Some(ended.unchecked_ref()));
        app.player.pause();
        let _ = audio.play();
        self.audio.set_value(Some((audio, url)));
        self.playing.set(Some(voice));
    }
}
