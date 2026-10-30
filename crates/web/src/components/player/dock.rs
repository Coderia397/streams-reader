//! The full player dock at the bottom of the reader: scrubber, transport, and the sheet buttons.

use leptos::prelude::*;

use super::play_button::PlayButton;
use super::scrubber::Scrubber;
use super::sheets::{ChaptersSheet, SleepSheet, SpeedSheet, VoiceSheet};
use crate::components::icons::{I, Icon};
use crate::state::player::SleepChoice;
use crate::state::use_app;
use crate::util::format;

#[component]
pub fn Dock() -> impl IntoView {
    let app = use_app();
    let player = app.player;
    let speed_open = RwSignal::new(false);
    let voice_open = RwSignal::new(false);
    let sleep_open = RwSignal::new(false);
    let chapters_open = RwSignal::new(false);
    let voice_name = move || {
        let id = player.doc.with(|d| d.as_ref().map(|d| d.voice.clone())).unwrap_or_else(|| app.settings.voice.get());
        app.voices.name(&id)
    };
    let sleep_label = move || match (player.sleep.get(), player.sleep_left_ms.get()) {
        (SleepChoice::Off, _) => "Sleep".to_owned(),
        (SleepChoice::EndOfChapter, _) => "Chapter end".to_owned(),
        (SleepChoice::Minutes(_), Some(ms)) => format::clock(ms),
        (SleepChoice::Minutes(m), None) => format!("{m} min"),
    };
    let has_chapters = move || player.doc.with(|d| d.as_ref().is_some_and(|d| !d.model.chapters.is_empty()));
    view! {
        <div class="dock">
            {move || player.error.get().map(|e| view! { <p class="dock__error" role="alert">{e}</p> })}
            <Scrubber />
            <div class="dock__transport">
                <button class="icon-btn" type="button" aria-label="Previous paragraph" on:click=move |_| player.previous()>
                    <Icon icon=I::SkipBack />
                </button>
                <button class="icon-btn icon-btn--seek" type="button" aria-label="Back 15 seconds" on:click=move |_| player.back()>
                    <Icon icon=I::Back15 />
                    <span class="seek-label">"15"</span>
                </button>
                <PlayButton />
                <button class="icon-btn icon-btn--seek" type="button" aria-label="Forward 15 seconds" on:click=move |_| player.forward()>
                    <Icon icon=I::Forward15 />
                    <span class="seek-label">"15"</span>
                </button>
                <button class="icon-btn" type="button" aria-label="Next paragraph" on:click=move |_| player.next()>
                    <Icon icon=I::SkipForward />
                </button>
            </div>
            <div class="dock__tools">
                <button class="tool" type="button" on:click=move |_| speed_open.set(true)>
                    <Icon icon=I::Gauge />
                    <span>{move || format::speed(player.rate.get())}</span>
                </button>
                <button class="tool" type="button" on:click=move |_| voice_open.set(true)>
                    <Icon icon=I::Voices />
                    <span class="tool__ellipsis">{voice_name}</span>
                </button>
                <button class="tool" class:tool--on=move || player.sleep.get() != SleepChoice::Off type="button" on:click=move |_| sleep_open.set(true)>
                    <Icon icon=I::Moon />
                    <span>{sleep_label}</span>
                </button>
                <button class="tool" type="button" disabled=move || !has_chapters() on:click=move |_| chapters_open.set(true)>
                    <Icon icon=I::List />
                    <span>"Chapters"</span>
                </button>
            </div>
            <SpeedSheet open=speed_open />
            <VoiceSheet open=voice_open />
            <SleepSheet open=sleep_open />
            <ChaptersSheet open=chapters_open />
        </div>
    }
}
