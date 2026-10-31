//! Playback speed, 0.5×–3.5× in 0.05 steps, with presets. Pitch is preserved by the player. Applies live and becomes
//! the default speed.

use leptos::prelude::*;

use crate::components::icon::{I, Icon};
use crate::components::sheet::Sheet;
use crate::config::{SPEED_CHIPS, SPEED_MAX, SPEED_MIN, SPEED_STEP};
use crate::state::use_app;
use crate::util::format;

#[component]
pub fn SpeedSheet(open: RwSignal<bool>) -> impl IntoView {
    let app = use_app();
    let player = app.player;
    let set = move |rate: f64| {
        player.set_rate(rate);
        app.settings.speed.set(player.rate.get_untracked());
    };
    let fill = move || format!("{:.2}%", (player.rate.get() - SPEED_MIN) / (SPEED_MAX - SPEED_MIN) * 100.0);
    view! {
        <Sheet open=open title=t!("speed-title")>
            <div class="speed">
                <p class="speed__value" aria-live="polite">{move || format::speed(player.rate.get())}</p>
                <div class="speed__row">
                    <button class="icon-btn" type="button" aria-label=t!("speed-slower")
                        on:click=move |_| set(player.rate.get_untracked() - SPEED_STEP)>
                        <span class="speed__sign" aria-hidden="true">"−"</span>
                    </button>
                    <input
                        class="slider"
                        type="range"
                        min=SPEED_MIN.to_string()
                        max=SPEED_MAX.to_string()
                        step=SPEED_STEP.to_string()
                        aria-label=t!("speed-title")
                        prop:value=move || player.rate.get().to_string()
                        style:--fill=fill
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<f64>() {
                                set(v);
                            }
                        }
                    />
                    <button class="icon-btn" type="button" aria-label=t!("speed-faster")
                        on:click=move |_| set(player.rate.get_untracked() + SPEED_STEP)>
                        <Icon icon=I::Plus />
                    </button>
                </div>
                <div class="chip-row">
                    {SPEED_CHIPS.iter().map(|&chip| view! {
                        <button
                            class="chip"
                            type="button"
                            aria-pressed={move || if (player.rate.get() - chip).abs() < 0.001 { "true" } else { "false" }}
                            on:click=move |_| set(chip)
                        >{format::speed(chip)}</button>
                    }).collect_view()}
                </div>
                <p class="sheet-note">{t!("speed-hint")}</p>
            </div>
        </Sheet>
    }
}
