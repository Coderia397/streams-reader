//! Voice picker: Standard (featured, then "More"), Expressive with today's GPU budget, and the user's own voices.
//! Choosing a voice re-segments the document for that engine and continues from the same place.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use nolabs_core::proto::api::{VoiceInfo, VoiceState};

use crate::components::icons::{I, Icon};
use crate::components::sheet::Sheet;
use crate::components::voice_row::VoiceRow;
use crate::state::use_app;
use crate::state::voices::{gpu_left, group};

#[component]
pub fn VoiceSheet(open: RwSignal<bool>) -> impl IntoView {
    let app = use_app();
    let player = app.player;
    let show_more = RwSignal::new(false);
    Effect::new(move |_| {
        if open.get() {
            spawn_local(app.voices.refresh(app.toasts));
            spawn_local(app.voices.refresh_status());
        }
    });
    let current =
        move || player.doc.with(|d| d.as_ref().map(|d| d.voice.clone())).unwrap_or_else(|| app.settings.voice.get());
    let groups = move || app.voices.catalog.with(|c| c.as_ref().map(|c| group(&c.voices)).unwrap_or_default());
    let choose = Callback::new(move |id: String| {
        app.settings.voice.set(id.clone());
        open.set(false);
        if player.has_doc() {
            spawn_local(player.switch_voice(id, app));
        }
    });
    let rows = move |voices: Vec<VoiceInfo>| {
        voices
            .into_iter()
            .map(|v| {
                let id = v.id.clone();
                view! { <VoiceRow voice=v selected=Signal::derive(move || current() == id) on_select=choose /> }
            })
            .collect_view()
    };
    let budget = move || {
        app.voices.status.with(|s| {
            s.as_ref().map(|s| {
                let (secs, runs) = gpu_left(s);
                format!("{secs} GPU seconds and {runs} runs left today, about {} min of audio.", secs / 60)
            })
        })
    };
    view! {
        <Sheet open=open title="Voice" tall=true>
            <section class="sheet-section">
                <h3 class="section-label">"Standard"</h3>
                <p class="section-hint">"Unlimited. Fast and clear."</p>
                {move || rows(groups().standard_featured)}
                <button class="disclosure" type="button" on:click=move |_| show_more.update(|v| *v = !*v)>
                    <span>{move || if show_more.get() { "Fewer voices" } else { "More voices" }}</span>
                    <span class="disclosure__chev" class:flip=move || show_more.get()><Icon icon=I::ChevronDown /></span>
                </button>
                <Show when=move || show_more.get()>{move || rows(groups().standard_more)}</Show>
            </section>
            <section class="sheet-section">
                <h3 class="section-label"><Icon icon=I::Sparkles />" Expressive"</h3>
                <p class="section-hint">{move || budget().unwrap_or_else(|| "Uses the daily GPU budget.".to_owned())}</p>
                {move || rows(groups().expressive)}
            </section>
            <section class="sheet-section">
                <h3 class="section-label">"My voices"</h3>
                {move || {
                    let mine: Vec<VoiceInfo> = groups().mine.into_iter().filter(|v| v.state == VoiceState::Ready).collect();
                    if mine.is_empty() {
                        view! {
                            <A href="/voices/new" attr:class="cta-row">
                                <Icon icon=I::Mic />
                                <span>"Record your voice"</span>
                                <Icon icon=I::ChevronRight />
                            </A>
                        }.into_any()
                    } else {
                        rows(mine).into_any()
                    }
                }}
            </section>
        </Sheet>
    }
}
