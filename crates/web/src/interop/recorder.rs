//! Microphone recording through `interop/recorder.js` (AudioWorklet, browser DSP off).

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::prop;
use crate::api::client::js_error_text;

#[wasm_bindgen(module = "/interop/recorder.js")]
extern "C" {
    #[wasm_bindgen(js_name = startRecording)]
    fn start_recording(on_level: &Closure<dyn FnMut(f64, f64)>) -> js_sys::Promise;
    #[wasm_bindgen(js_name = stopRecording)]
    fn stop_recording() -> js_sys::Promise;
    #[wasm_bindgen(js_name = cancelRecording)]
    fn cancel_recording() -> js_sys::Promise;
}

/// A running recording. Keeps the level callback alive until stopped or dropped.
pub struct Recording {
    _on_level: Closure<dyn FnMut(f64, f64)>,
}

pub struct Take {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Starts capturing; `on_level(rms, peak)` runs for each ~40 ms chunk.
pub async fn start(on_level: impl FnMut(f64, f64) + 'static) -> Result<Recording, String> {
    let closure = Closure::<dyn FnMut(f64, f64)>::new(on_level);
    JsFuture::from(start_recording(&closure)).await.map_err(js_error_text)?;
    Ok(Recording { _on_level: closure })
}

impl Recording {
    pub async fn stop(self) -> Result<Take, String> {
        let result = JsFuture::from(stop_recording()).await.map_err(js_error_text)?;
        let samples: js_sys::Float32Array = prop(&result, "samples").dyn_into().map_err(|_| "no samples".to_owned())?;
        let sample_rate = prop(&result, "sampleRate").as_f64().unwrap_or(48_000.0) as u32;
        Ok(Take { samples: samples.to_vec(), sample_rate })
    }
}

/// Stops any recording without keeping it (leaving the page, re-recording).
pub async fn cancel() {
    let _ = JsFuture::from(cancel_recording()).await;
}

/// Friendly reason for a failed start (permission denied, no microphone).
pub fn start_error_message(raw: &str) -> String {
    if raw.contains("NotAllowed") || raw.to_lowercase().contains("permission") {
        "Microphone access is blocked. Allow it in Chrome's site settings and try again.".to_owned()
    } else if raw.contains("NotFound") {
        "No microphone found.".to_owned()
    } else {
        format!("Couldn't start recording: {raw}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_errors_are_friendly() {
        assert!(start_error_message("NotAllowedError: Permission denied").contains("blocked"));
        assert_eq!(start_error_message("NotFoundError"), "No microphone found.");
        assert!(start_error_message("weird").ends_with("weird"));
    }
}
