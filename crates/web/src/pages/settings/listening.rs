//! Listening defaults: voice, speed, skip lengths.

use leptos::prelude::*;

use crate::components::controls::{Group, NavRow, Segmented};
use crate::components::icon::I;
use crate::config::{SKIP_CHOICES_S, SPEED_CHIPS};
use crate::state::use_app;
use crate::util::format;

#[component]
pub fn ListeningGroup() -> impl IntoView {
    let app = use_app();
    let s = app.settings;
    let skip_options = || SKIP_CHOICES_S.iter().map(|&v| (v, t!("seconds-short", seconds = v))).collect::<Vec<_>>();
    view! {
        <Group title=t!("settings-listening")>
            <NavRow
                icon=I::Voices
                label=t!("settings-default-voice")
                value=Signal::derive(move || app.voices.name(&s.voice.get()))
                chevron=true
                on_click=Callback::new(move |()| app.nav.go("/voices"))
            />
            <div class="row-item row-item--stack">
                <span class="row-item__head">
                    <span class="row-item__icon"><crate::components::icon::Icon icon=I::Speed /></span>
                    <span class="row-item__label">{t!("settings-default-speed")}</span>
                    <span class="row-item__value">{move || format::speed(s.speed.get())}</span>
                </span>
                <div class="chip-row chip-row--thirds">
                    {SPEED_CHIPS.iter().map(|&v| view! {
                        <button class="chip" type="button"
                            aria-pressed={move || if (s.speed.get() - v).abs() < 0.001 { "true" } else { "false" }}
                            on:click=move |_| { s.speed.set(v); app.player.set_rate(v); }>
                            {format::speed(v)}
                        </button>
                    }).collect_view()}
                </div>
            </div>
            <div class="row-item row-item--stack">
                <span class="row-item__head">
                    <span class="row-item__icon"><crate::components::icon::Icon icon=I::Replay15 /></span>
                    <span class="row-item__label">{t!("settings-skip-back")}</span>
                </span>
                <Segmented label=t!("settings-skip-back") options=skip_options() value=s.skip_back_s.into()
                    on_pick=Callback::new(move |v| s.skip_back_s.set(v)) />
            </div>
            <div class="row-item row-item--stack">
                <span class="row-item__head">
                    <span class="row-item__icon"><crate::components::icon::Icon icon=I::Forward30 /></span>
                    <span class="row-item__label">{t!("settings-skip-forward")}</span>
                </span>
                <Segmented label=t!("settings-skip-forward") options=skip_options() value=s.skip_forward_s.into()
                    on_pick=Callback::new(move |v| s.skip_forward_s.set(v)) />
            </div>
        </Group>
    }
}
