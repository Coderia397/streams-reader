//! Position over the whole document, estimated from char counts (audio length is unknown until rendered). Dragging
//! previews the target; releasing jumps to the segment under it.

use leptos::prelude::*;

use crate::reading::doc::seconds_for_chars;
use crate::state::use_app;
use crate::util::format;

const STEPS: f64 = 1000.0;

#[component]
pub fn Scrubber() -> impl IntoView {
    let player = use_app().player;
    let dragging = RwSignal::new(None::<f64>);
    let shown = move || dragging.get().unwrap_or_else(|| player.fraction());
    let left_label = move || match dragging.get() {
        Some(f) => {
            let rate = player.rate.get();
            let total = player.doc.with(|d| d.as_ref().map_or(0, |d| d.model.total_chars));
            let left = f64::from(total) * (1.0 - f);
            format::time_left(seconds_for_chars(left as u64, rate))
        }
        None => format::time_left(player.seconds_left()),
    };
    let percent = move || format!("{:.0}%", shown() * 100.0);
    let commit = move || {
        if let Some(f) = dragging.get_untracked() {
            let target = player.doc.with_untracked(|d| d.as_ref().map(|d| d.model.segment_for_fraction(f)));
            if let Some(seg) = target {
                player.jump_to(seg);
            }
        }
        dragging.set(None);
    };
    view! {
        <div class="scrubber">
            <input
                class="scrubber__range"
                type="range"
                min="0"
                max=STEPS.to_string()
                step="1"
                aria-label="Position in document"
                prop:value=move || (shown() * STEPS).round().to_string()
                style:--fill=move || format!("{:.2}%", shown() * 100.0)
                on:input=move |e| {
                    let v = event_target_value(&e).parse::<f64>().unwrap_or(0.0);
                    dragging.set(Some(v / STEPS));
                }
                on:change=move |_| commit()
            />
            <div class="scrubber__labels">
                <span>{percent}</span>
                <span>{left_label}</span>
            </div>
        </div>
    }
}
