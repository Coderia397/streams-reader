//! Rolling offline cache of every fetched clip (Cache API, [`AUDIO_CACHE`]). Clip keys are content addresses, so a
//! cached clip never goes stale; the cache is cleared only from Settings. The service worker never touches `/api`,
//! so this module is the only writer.

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::api::client::{js_error_text, read_bytes};
use crate::api::{ClientError, audio};
use crate::config::AUDIO_CACHE;
use crate::util::dom;

async fn open() -> Option<web_sys::Cache> {
    let caches = dom::window()?.caches().ok()?;
    let cache = JsFuture::from(caches.open(AUDIO_CACHE)).await.ok()?;
    cache.dyn_into().ok()
}

async fn cached(cache: &web_sys::Cache, url: &str) -> Option<web_sys::Response> {
    let hit = JsFuture::from(cache.match_with_str(url)).await.ok()?;
    if hit.is_undefined() { None } else { hit.dyn_into().ok() }
}

/// Clip bytes from the cache, or from the Worker (then stored). A cache failure never blocks playback.
pub async fn fetch(key: &str) -> Result<js_sys::Uint8Array, ClientError> {
    // Cached by the token-free path, so a renewed token still hits the cache.
    let url = audio::clip_path(key);
    let cache = open().await;
    if let Some(cache) = &cache
        && let Some(resp) = cached(cache, &url).await
        && let Ok(bytes) = read_bytes(&resp).await
    {
        return Ok(bytes);
    }
    let resp = audio::fetch(key).await?;
    if let Some(cache) = &cache
        && let Ok(copy) = resp.clone()
    {
        let _ = JsFuture::from(cache.put_with_str(&url, &copy)).await;
    }
    read_bytes(&resp).await
}

/// Deletes every cached clip.
pub async fn clear() -> Result<(), ClientError> {
    let caches = dom::window()
        .ok_or_else(|| ClientError::Browser("no window".into()))?
        .caches()
        .map_err(|e| ClientError::Browser(js_error_text(e)))?;
    JsFuture::from(caches.delete(AUDIO_CACHE)).await.map_err(|e| ClientError::Browser(js_error_text(e)))?;
    Ok(())
}

/// Bytes used and available to this origin, from `navigator.storage.estimate()`.
pub async fn estimate() -> Option<(f64, f64)> {
    let manager = dom::window()?.navigator().storage();
    let est = JsFuture::from(manager.estimate().ok()?).await.ok()?;
    let est: web_sys::StorageEstimate = est.unchecked_into();
    Some((est.get_usage()?, est.get_quota()?))
}
