//! Bottom sheet: the app's one modal surface. A drag handle (and the header) follows the finger; a flick or a drag
//! past 40 % of the height closes it. On wide screens it becomes a centred dialog (CSS). Changes made inside sheets
//! apply at once, so there is no Save button.
//!
//! The panel is a native `<dialog>` opened with `showModal()`: the page behind it is inert, focus moves into the
//! sheet and stays there, Escape (and Android's back gesture) closes it, and focus returns to the control that
//! opened it. While it is open, toasts render inside it, because the app's own toast region is inert too.

use std::sync::atomic::{AtomicU32, Ordering};

use leptos::ev;
use leptos::html;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use super::icon::{I, Icon};
use super::toast_host::ToastHost;
use crate::util::timer::now_ms;

/// Closes when the finger moved faster than this (px per ms) downward on release.
const FLICK_PX_PER_MS: f64 = 0.5;
const DISMISS_FRACTION: f64 = 0.4;

/// Source of unique `id`s for the sheet titles (`aria-labelledby`).
static NEXT_TITLE: AtomicU32 = AtomicU32::new(0);

/// `(clientX, clientY)`: `f64` in web-sys for wasm and `i32` in the native stubs used by tests.
#[allow(clippy::useless_conversion)]
fn client_xy(e: &ev::MouseEvent) -> (f64, f64) {
    (f64::from(e.client_x()), f64::from(e.client_y()))
}

#[derive(Clone, Copy)]
struct Drag {
    start_y: f64,
    start_ms: i64,
    dy: f64,
}

#[component]
pub fn Sheet(
    open: RwSignal<bool>,
    #[prop(into)] title: Signal<String>,
    #[prop(optional, into)] tall: Signal<bool>,
    children: ChildrenFn,
) -> impl IntoView {
    let children = StoredValue::new(children);
    view! {
        <Show when=move || open.get()>
            <Panel open=open title=title tall=tall children=children />
        </Show>
    }
}

/// The open sheet. It exists only while `open` is true, so mounting is opening and cleanup is closing.
#[component]
fn Panel(
    open: RwSignal<bool>,
    title: Signal<String>,
    tall: Signal<bool>,
    children: StoredValue<ChildrenFn>,
) -> impl IntoView {
    let drag = RwSignal::new(None::<Drag>);
    let panel = NodeRef::<html::Dialog>::new();
    let title_id = format!("sheet-title-{}", NEXT_TITLE.fetch_add(1, Ordering::Relaxed));
    let label_id = title_id.clone();
    // The control that opened the sheet; it gets focus back when the sheet closes.
    let opener = StoredValue::new_local(document().active_element());
    let shown = StoredValue::new_local(None::<web_sys::HtmlDialogElement>);

    Effect::new(move |_| {
        let Some(dialog) = panel.get() else { return };
        if !dialog.open() {
            let _ = dialog.show_modal();
        }
        shown.set_value(Some(dialog));
    });
    on_cleanup(move || {
        // Closed before focus moves back: the page stays inert while a modal dialog is open.
        if let Some(Some(dialog)) = shown.try_get_value()
            && dialog.open()
        {
            dialog.close();
        }
        let back = opener.try_get_value().flatten().and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
        if let Some(el) = back.filter(|e| e.is_connected()) {
            let _ = el.focus();
        }
    });

    let close = move || {
        drag.try_set(None);
        open.try_set(false);
    };
    // A click whose point lies outside the panel's box landed on the backdrop.
    let backdrop_click = move |e: ev::MouseEvent| {
        let Some(dialog) = panel.get_untracked() else { return };
        let on_dialog = e.target().is_some_and(|t| t.dyn_ref::<web_sys::Element>() == Some(dialog.as_ref()));
        if !on_dialog {
            return;
        }
        let r = dialog.get_bounding_client_rect();
        let (x, y) = client_xy(&e);
        if x < r.left() || x > r.right() || y < r.top() || y > r.bottom() {
            close();
        }
    };

    let down = move |e: ev::PointerEvent| {
        if let Some(el) = e.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
            let _ = el.set_pointer_capture(e.pointer_id());
        }
        drag.set(Some(Drag { start_y: client_xy(&e).1, start_ms: now_ms(), dy: 0.0 }));
    };
    let moved = move |e: ev::PointerEvent| {
        drag.update(|d| {
            if let Some(d) = d {
                d.dy = (client_xy(&e).1 - d.start_y).max(0.0);
            }
        });
    };
    let up = move |_: ev::PointerEvent| {
        let Some(d) = drag.get_untracked() else { return };
        let height = panel.get_untracked().map_or(600.0, |p| f64::from(p.offset_height()));
        let speed = d.dy / ((now_ms() - d.start_ms).max(1) as f64);
        if d.dy > height * DISMISS_FRACTION || (speed > FLICK_PX_PER_MS && d.dy > 24.0) {
            close();
        } else {
            drag.set(None);
        }
    };
    let transform = move || drag.get().filter(|d| d.dy > 0.0).map(|d| format!("translateY({:.0}px)", d.dy));

    view! {
        <dialog
            class="sheet"
            class:sheet--tall=move || tall.get()
            class:is-dragging=move || drag.get().is_some()
            aria-labelledby=label_id
            node_ref=panel
            style:transform=transform
            // Escape and the back gesture close the dialog natively; keep the signal in step.
            on:close=move |_| close()
            on:click=backdrop_click
        >
            <header class="sheet__head" on:pointerdown=down on:pointermove=moved on:pointerup=up on:pointercancel=up>
                <span class="sheet__handle" aria-hidden="true"></span>
                <h2 class="sheet__title" id=title_id>{move || title.get()}</h2>
                <button
                    class="icon-btn sheet__close"
                    type="button"
                    aria-label=t!("action-close")
                    on:pointerdown=|e| e.stop_propagation()
                    on:click=move |_| close()
                >
                    <Icon icon=I::Close />
                </button>
            </header>
            <div class="sheet__body">{move || children.with_value(|c| c())}</div>
            <ToastHost in_sheet=true />
        </dialog>
    }
}
