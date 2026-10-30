//! `/voices`: choose the default voice, hear previews, manage recorded voices.

mod preview;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use nolabs_core::proto::api::{Engine, VoiceInfo, VoiceKind};

use crate::api::voices as voices_api;
use crate::components::icons::{I, Icon, Spinner};
use crate::components::sheet::Sheet;
use crate::components::voice_row::VoiceRow;
use crate::config::PREVIEW_TEXT;
use crate::state::use_app;
use crate::state::voices::{gpu_left, group, preview_gpu_seconds};
use preview::Preview;

#[component]
fn PreviewButton(voice: VoiceInfo, preview: Preview, on_ask: Callback<VoiceInfo>) -> impl IntoView {
    let app = use_app();
    let id_loading = voice.id.clone();
    let id_playing = voice.id.clone();
    let expressive = voice.engine == Engine::Ming && voice.preview_key.is_none();
    let label = format!("Preview {}", voice.name);
    let click = move |_| {
        let playing = preview.playing.get_untracked().as_deref() == Some(voice.id.as_str());
        if expressive && !playing {
            on_ask.run(voice.clone());
        } else {
            preview.toggle(app, voice.id.clone(), voice.preview_key.clone());
        }
    };
    view! {
        <button class="icon-btn icon-btn--soft" type="button" aria-label=label on:click=click>
            {move || {
                if preview.loading.get().as_deref() == Some(id_loading.as_str()) {
                    view! { <Spinner /> }.into_any()
                } else if preview.playing.get().as_deref() == Some(id_playing.as_str()) {
                    view! { <Icon icon=I::Stop /> }.into_any()
                } else {
                    view! { <Icon icon=I::Volume /> }.into_any()
                }
            }}
        </button>
    }
}

#[component]
pub fn VoicesPage() -> impl IntoView {
    let app = use_app();
    let preview = Preview::new();
    let show_more = RwSignal::new(false);
    let ask_open = RwSignal::new(false);
    let asking = RwSignal::new(None::<VoiceInfo>);
    let manage_open = RwSignal::new(false);
    let managing = RwSignal::new(None::<VoiceInfo>);

    spawn_local(app.voices.refresh(app.toasts));
    spawn_local(app.voices.refresh_status());

    let groups = move || app.voices.catalog.with(|c| c.as_ref().map(|c| group(&c.voices)));
    let choose = Callback::new(move |id: String| {
        app.settings.voice.set(id.clone());
        app.toasts.success(format!("{} is now your default voice.", app.voices.name(&id)));
    });
    let on_ask = Callback::new(move |v: VoiceInfo| {
        asking.set(Some(v));
        ask_open.set(true);
    });
    let rows = move |voices: Vec<VoiceInfo>| {
        voices
            .into_iter()
            .map(|v| {
                let id = v.id.clone();
                let mine = v.kind != VoiceKind::Preset;
                let for_preview = v.clone();
                let for_manage = v.clone();
                view! {
                    <VoiceRow voice=v selected=Signal::derive(move || app.settings.voice.get() == id) on_select=choose>
                        <PreviewButton voice=for_preview.clone() preview=preview on_ask=on_ask />
                        {mine.then(|| {
                            let v = for_manage.clone();
                            view! {
                                <button class="icon-btn" type="button" aria-label="Manage voice" on:click=move |_| {
                                    managing.set(Some(v.clone()));
                                    manage_open.set(true);
                                }><Icon icon=I::More /></button>
                            }
                        })}
                    </VoiceRow>
                }
            })
            .collect_view()
    };
    let budget = move || app.voices.status.with(|s| s.as_ref().map(|s| (gpu_left(s), s.gpu.cap_s)));
    let confirm_preview = move |_| {
        ask_open.set(false);
        if let Some(v) = asking.get_untracked() {
            preview.toggle(app, v.id, v.preview_key);
        }
    };
    let delete_voice = move |_| {
        let Some(v) = managing.get_untracked() else { return };
        manage_open.set(false);
        spawn_local(async move {
            match voices_api::delete(&v.id).await {
                Ok(()) => {
                    if app.settings.voice.get_untracked() == v.id {
                        app.settings.voice.set(nolabs_core::voices::DEFAULT_VOICE.to_owned());
                    }
                    app.toasts.info(format!("Deleted {}.", v.name));
                    app.voices.refresh(app.toasts).await;
                }
                Err(e) => app.toasts.api_error(&e),
            }
        });
    };

    view! {
        <div class="page">
            <header class="topbar"><h1 class="page-title page-title--bar">"Voices"</h1></header>
            <p class="page-lead">"Pick the voice that reads to you. Tap the speaker to hear it first."</p>
            {move || match groups() {
                None => view! { <div class="center-spinner"><Spinner /></div> }.into_any(),
                Some(g) => view! {
                    <section class="section">
                        <h2 class="section-label">"My voices"</h2>
                        <A href="/voices/new" attr:class="record-card">
                            <span class="record-card__icon"><Icon icon=I::Mic /></span>
                            <span class="record-card__text">
                                <span class="record-card__title">"Record your voice"</span>
                                <span class="record-card__line">"Read eight short lines. About two minutes."</span>
                            </span>
                            <Icon icon=I::ChevronRight />
                        </A>
                        {rows(g.mine)}
                    </section>
                    <section class="section">
                        <h2 class="section-label">"Standard"</h2>
                        <p class="section-hint">"Unlimited listening. Quick to start."</p>
                        {rows(g.standard_featured)}
                        <button class="disclosure" type="button" on:click=move |_| show_more.update(|v| *v = !*v)>
                            <span>{move || if show_more.get() { "Fewer voices" } else { "More voices" }}</span>
                            <span class="disclosure__chev" class:flip=move || show_more.get()><Icon icon=I::ChevronDown /></span>
                        </button>
                        <Show when=move || show_more.get()>{rows(g.standard_more.clone())}</Show>
                    </section>
                    <section class="section">
                        <h2 class="section-label"><Icon icon=I::Sparkles />" Expressive"</h2>
                        {move || budget().map(|((left, runs), cap)| view! {
                            <div class="budget">
                                <div class="budget__row">
                                    <span>"Today's GPU budget"</span>
                                    <span class="budget__num">{format!("{left} of {cap} s · {runs} runs")}</span>
                                </div>
                                <div class="bar"><div class="bar__fill bar__fill--warm"
                                    style:width=format!("{:.1}%", f64::from(left) / f64::from(cap.max(1)) * 100.0)></div></div>
                            </div>
                        })}
                        <p class="section-hint">"Richer, emotional delivery. Each preview and minute of audio spends the daily budget."</p>
                        {rows(g.expressive)}
                    </section>
                }.into_any(),
            }}
            <Sheet open=ask_open title="Play an expressive preview?">
                <p class="sheet-lead">
                    {move || format!(
                        "This renders a short sample on the GPU and uses about {} seconds of today's budget.",
                        preview_gpu_seconds(PREVIEW_TEXT.chars().count())
                    )}
                </p>
                {move || budget().map(|((left, runs), _)| view! {
                    <p class="hint">{format!("{left} seconds and {runs} runs left today.")}</p>
                })}
                <div class="btn-row">
                    <button class="btn" type="button" on:click=move |_| ask_open.set(false)>"Not now"</button>
                    <button class="btn btn--primary" type="button" on:click=confirm_preview>"Play preview"</button>
                </div>
            </Sheet>
            <Sheet open=manage_open title="Your voice">
                {move || managing.get().map(|v| view! { <p class="sheet-lead">{v.name.clone()}</p> })}
                <button class="option option--danger" type="button" on:click=delete_voice>
                    <Icon icon=I::Trash /><span>"Delete this voice and its audio"</span>
                </button>
            </Sheet>
        </div>
    }
}
