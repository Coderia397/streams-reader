//! Voice picker: Favourites, Recent, your own voices, Standard (featured, then more) and Expressive with today's GPU
//! budget. Choosing a voice applies at once from the current sentence, with Undo.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use nolabs_core::proto::api::{VoiceInfo, VoiceState};

use crate::components::icon::{I, Icon};
use crate::components::sheet::Sheet;
use crate::components::voice_row::VoiceRow;
use crate::state::use_app;
use crate::state::voices::{GpuBudget, group, pick};

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
    let all = move || app.voices.catalog.with(|c| c.as_ref().map(|c| c.voices.clone()).unwrap_or_default());
    let groups = move || group(&all());
    let switch = move |id: String| {
        app.settings.use_voice(&id);
        if player.has_doc() {
            spawn_local(player.switch_voice(id, app));
        }
    };
    let choose = Callback::new(move |id: String| {
        let before = current();
        open.set(false);
        if before == id {
            return;
        }
        switch(id.clone());
        app.toasts.undoable(t!("voice-switched", name = app.voices.name(&id)), move || switch(before.clone()));
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
                let b = GpuBudget::of(s);
                if b.is_spent() {
                    t!("error-gpu-budget")
                } else {
                    t!("voice-budget-left", minutes = b.left_s.div_ceil(60), runs = b.runs_left)
                }
            })
        })
    };
    let section = move |title: String, voices: Vec<VoiceInfo>| {
        (!voices.is_empty()).then(|| {
            view! {
                <section class="sheet-section">
                    <h3 class="overline">{title}</h3>
                    {rows(voices)}
                </section>
            }
        })
    };
    view! {
        <Sheet open=open title=t!("voice-title") tall=true>
            {move || section(t!("voices-favourites"), pick(&all(), &app.settings.favourites.get()))}
            {move || section(t!("voices-recent"), pick(&all(), &app.settings.recent.get()))}
            <section class="sheet-section">
                <h3 class="overline">{t!("voices-yours")}</h3>
                {move || {
                    let mine: Vec<VoiceInfo> = groups().mine.into_iter().filter(|v| v.state == VoiceState::Ready).collect();
                    if mine.is_empty() {
                        view! {
                            <A href="/voices/new" attr:class="choice">
                                <span class="choice__icon"><Icon icon=I::Mic /></span>
                                <span class="choice__text">
                                    <span class="choice__label">{t!("record-cta")}</span>
                                    <span class="choice__hint">{t!("record-cta-line")}</span>
                                </span>
                                <Icon icon=I::Forward class="choice__chev" />
                            </A>
                        }.into_any()
                    } else {
                        rows(mine).into_any()
                    }
                }}
            </section>
            <section class="sheet-section">
                <h3 class="overline">{t!("voices-standard")}</h3>
                <p class="section-hint">{t!("voices-standard-hint")}</p>
                {move || rows(groups().standard_featured)}
                <button class="disclosure" type="button" aria-expanded=move || show_more.get().to_string()
                    on:click=move |_| show_more.update(|v| *v = !*v)>
                    <span>{move || if show_more.get() { t!("voices-fewer") } else { t!("voices-more") }}</span>
                    <span class="disclosure__chev" class:is-open=move || show_more.get()><Icon icon=I::ChevronDown /></span>
                </button>
                <Show when=move || show_more.get()>{move || rows(groups().standard_more)}</Show>
            </section>
            <section class="sheet-section">
                <h3 class="overline">{t!("voices-expressive")}</h3>
                <p class="section-hint">{move || budget().unwrap_or_else(|| t!("voices-expressive-hint"))}</p>
                {move || rows(groups().expressive)}
            </section>
        </Sheet>
    }
}
