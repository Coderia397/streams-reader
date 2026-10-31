//! The player dock at the bottom of the reader: chapter chip, scrubber with times, transport (sleep, skip back, play,
//! skip forward, speed) and the voice chip. While playing it folds to a slim strip after a few seconds without a
//! touch; any touch or scroll up unfolds it (the reader owns `folded`). The sheets are mounted beside the dock, never
//! inside it, so their markup stays outside the folding dock (the open panel itself sits in the top layer).
//! The contents sheet belongs to the reader, which opens it from here and from More.

use leptos::prelude::*;

use super::play_button::PlayButton;
use super::scrubber::Scrubber;
use super::sheets::{SleepSheet, SpeedSheet, VoiceSheet};
use super::skip::SkipButton;
use crate::components::icon::{I, Icon};
use crate::components::voice_row::Orb;
use crate::state::player::SleepChoice;
use crate::state::use_app;
use crate::util::format;

#[component]
pub fn Dock(folded: Signal<bool>, on_more: Callback<()>, on_contents: Callback<()>) -> impl IntoView {
    let app = use_app();
    let player = app.player;
    let speed_open = RwSignal::new(false);
    let voice_open = RwSignal::new(false);
    let sleep_open = RwSignal::new(false);
    let voice_name = move || {
        let id = player.doc.with(|d| d.as_ref().map(|d| d.voice.clone())).unwrap_or_else(|| app.settings.voice.get());
        app.voices.name(&id)
    };
    let sleep_on = move || player.sleep.get() != SleepChoice::Off;
    let sleep_badge = move || match (player.sleep.get(), player.sleep_left_ms.get()) {
        (SleepChoice::Minutes(_), Some(ms)) => Some(format!("{}", ms.div_ceil(60_000))),
        _ => None,
    };
    let sleep_label = move || match player.sleep.get() {
        SleepChoice::Off => t!("sleep-title"),
        _ => t!("sleep-on"),
    };
    view! {
        <div class="dock" class:is-folded=move || folded.get()>
            <div class="dock__top">
                <button class="chip dock__chapter" type="button" disabled=move || !player.has_chapters()
                    on:click=move |_| on_contents.run(())>
                    <Icon icon=I::Contents />
                    <span class="chip__text">{move || player.chapter_title().unwrap_or_else(|| player.title())}</span>
                </button>
            </div>
            <Scrubber />
            <div class="dock__transport">
                <button class="icon-btn icon-btn--lg dock__sleep" class:is-on=sleep_on type="button"
                    aria-label=sleep_label on:click=move |_| sleep_open.set(true)>
                    <Icon icon=I::Sleep />
                    {move || sleep_badge().map(|m| view! { <span class="dock__badge">{m}</span> })}
                </button>
                <SkipButton forward=false />
                <PlayButton />
                <SkipButton forward=true />
                <button class="icon-btn icon-btn--lg dock__speed" type="button" aria-label=t!("speed-title")
                    on:click=move |_| speed_open.set(true)>
                    {move || format::speed(player.rate.get())}
                </button>
            </div>
            <div class="dock__tools">
                <button class="chip chip--voice" type="button" on:click=move |_| voice_open.set(true)>
                    {move || view! { <Orb name=voice_name() small=true /> }}
                    <span class="chip__text">{voice_name}</span>
                    <Icon icon=I::ChevronDown />
                </button>
                <button class="icon-btn" type="button" aria-label=t!("reader-more") on:click=move |_| on_more.run(())>
                    <Icon icon=I::More />
                </button>
            </div>
        </div>
        <SpeedSheet open=speed_open />
        <VoiceSheet open=voice_open />
        <SleepSheet open=sleep_open />
    }
}
