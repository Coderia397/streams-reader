//! Playback speed, 0.5x–3x in 0.05 steps, with quick chips. Pitch is preserved by the player. The choice becomes the
//! default speed.

use leptos::prelude::*;

use crate::components::icons::{I, Icon};
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
    view! {
        <Sheet open=open title="Playback speed">
            <div class="speed">
                <p class="speed__value">{move || format::speed(player.rate.get())}</p>
                <div class="speed__row">
                    <button class="icon-btn" type="button" aria-label="Slower"
                        on:click=move |_| set(player.rate.get_untracked() - SPEED_STEP)>
                        <span class="speed__sign">"−"</span>
                    </button>
                    <input
                        class="range"
                        type="range"
                        min=SPEED_MIN.to_string()
                        max=SPEED_MAX.to_string()
                        step=SPEED_STEP.to_string()
                        aria-label="Speed"
                        prop:value=move || player.rate.get().to_string()
                        style:--fill=move || format!("{:.2}%", (player.rate.get() - SPEED_MIN) / (SPEED_MAX - SPEED_MIN) * 100.0)
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<f64>() {
                                set(v);
                            }
                        }
                    />
                    <button class="icon-btn" type="button" aria-label="Faster"
                        on:click=move |_| set(player.rate.get_untracked() + SPEED_STEP)>
                        <Icon icon=I::Plus />
                    </button>
                </div>
                <div class="chips">
                    {SPEED_CHIPS.iter().map(|&chip| view! {
                        <button
                            class="chip"
                            class:is-on=move || (player.rate.get() - chip).abs() < 0.001
                            type="button"
                            on:click=move |_| set(chip)
                        >{format::speed(chip)}</button>
                    }).collect_view()}
                </div>
                <p class="hint">"Pitch stays natural at every speed."</p>
            </div>
        </Sheet>
    }
}
