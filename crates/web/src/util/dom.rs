//! Thin, non-panicking access to browser globals.

use wasm_bindgen::JsCast;

pub fn window() -> Option<web_sys::Window> {
    web_sys::window()
}

pub fn document() -> Option<web_sys::Document> {
    window().and_then(|w| w.document())
}

pub fn html_element() -> Option<web_sys::Element> {
    document().and_then(|d| d.document_element())
}

pub fn local_storage() -> Option<web_sys::Storage> {
    window().and_then(|w| w.local_storage().ok().flatten())
}

pub fn media_matches(query: &str) -> bool {
    window().and_then(|w| w.match_media(query).ok().flatten()).is_some_and(|m| m.matches())
}

pub fn prefers_reduced_motion() -> bool {
    media_matches("(prefers-reduced-motion: reduce)")
}

pub fn reload() {
    if let Some(w) = window() {
        let _ = w.location().reload();
    }
}

pub fn is_hidden() -> bool {
    document().is_some_and(|d| d.visibility_state() == web_sys::VisibilityState::Hidden)
}

/// The element with `data-seg="{idx}"` in the reader, if rendered.
pub fn segment_element(idx: u32) -> Option<web_sys::HtmlElement> {
    document()?.query_selector(&format!("[data-seg=\"{idx}\"]")).ok().flatten()?.dyn_into().ok()
}

pub fn viewport_height() -> f64 {
    window().and_then(|w| w.inner_height().ok()).and_then(|v| v.as_f64()).unwrap_or(800.0)
}

/// Scrolls the element to the vertical centre of the viewport.
pub fn scroll_into_center(el: &web_sys::Element, smooth: bool) {
    let opts = web_sys::ScrollIntoViewOptions::new();
    opts.set_block(web_sys::ScrollLogicalPosition::Center);
    opts.set_behavior(if smooth { web_sys::ScrollBehavior::Smooth } else { web_sys::ScrollBehavior::Instant });
    el.scroll_into_view_with_scroll_into_view_options(&opts);
}
