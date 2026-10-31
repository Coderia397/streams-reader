//! `/voices`: choose the default voice, hear previews, keep favourites, manage recorded voices.

mod preview;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use nolabs_core::proto::api::{Engine, VoiceInfo, VoiceKind};

use crate::api::voices as voices_api;
use crate::components::icon::{I, Icon, Spinner};
use crate::components::sheet::Sheet;
use crate::components::spot::Spot;
use crate::components::voice_row::VoiceRow;
use crate::config::PREVIEW_TEXT;
use crate::state::use_app;
use crate::state::voices::{GpuBudget, group, pick, preview_gpu_seconds};
use preview::Preview;

#[component]
fn PreviewButton(voice: VoiceInfo, preview: Preview, on_ask: Callback<VoiceInfo>) -> impl IntoView {
    let app = use_app();
    let id_loading = voice.id.clone();
    let id_playing = voice.id.clone();
    let expressive = voice.engine == Engine::Ming && voice.preview_key.is_none();
    let label = t!("voice-preview", name = voice.name.clone());
    let click = move |_| {
        let playing = preview.playing.get_untracked().as_deref() == Some(voice.id.as_str());
        if expressive && !playing {
            on_ask.run(voice.clone());
        } else {
            preview.toggle(app, voice.id.clone(), voice.preview_key.clone());
        }
    };
    view! {
        <button class="icon-btn" type="button" aria-label=label on:click=click>
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
        let before = app.settings.voice.get_untracked();
        if before == id {
            return;
        }
        app.settings.use_voice(&id);
        app.toasts.undoable(t!("voice-default-set", name = app.voices.name(&id)), move || {
            app.settings.voice.set(before.clone())
        });
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
                                <button class="icon-btn" type="button" aria-label=t!("voice-manage") on:click=move |_| {
                                    managing.set(Some(v.clone()));
                                    manage_open.set(true);
                                }><Icon icon=I::MoreVertical /></button>
                            }
                        })}
                    </VoiceRow>
                }
            })
            .collect_view()
    };
    let budget = move || app.voices.status.with(|s| s.as_ref().map(GpuBudget::of));
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
                    app.toasts.info(t!("voice-deleted", name = v.name.clone()));
                    app.voices.refresh(app.toasts).await;
                }
                Err(e) => app.toasts.api_error(&e),
            }
        });
    };

    let all = move || app.voices.catalog.with(|c| c.as_ref().map(|c| c.voices.clone()).unwrap_or_default());
    let section = move |title: String, voices: Vec<VoiceInfo>| {
        (!voices.is_empty()).then(|| {
            view! {
                <section class="section">
                    <h2 class="overline">{title}</h2>
                    <div class="list-card">{rows(voices)}</div>
                </section>
            }
        })
    };

    view! {
        <div class="page">
            <header class="page-head"><h1 class="page-title">{t!("voices-title")}</h1></header>
            <p class="page-lead">{t!("voices-lead")}</p>
            {move || match groups() {
                None => view! { <div class="center-stack"><Spinner /></div> }.into_any(),
                Some(g) => view! {
                    {section(t!("voices-favourites"), pick(&all(), &app.settings.favourites.get()))}
                    {section(t!("voices-recent"), pick(&all(), &app.settings.recent.get()))}
                    <section class="section">
                        <h2 class="overline">{t!("voices-yours")}</h2>
                        <A href="/voices/new" attr:class="feature-card">
                            <Spot icon=I::Mic small=true />
                            <span class="feature-card__text">
                                <span class="feature-card__title">{t!("record-cta")}</span>
                                <span class="feature-card__line">{t!("record-cta-line")}</span>
                            </span>
                            <Icon icon=I::Forward class="feature-card__chev" />
                        </A>
                        {(!g.mine.is_empty()).then(|| view! { <div class="list-card">{rows(g.mine.clone())}</div> })}
                    </section>
                    <section class="section">
                        <h2 class="overline">{t!("voices-standard")}</h2>
                        <p class="section-hint">{t!("voices-standard-hint")}</p>
                        <div class="list-card">
                            {rows(g.standard_featured)}
                            <Show when=move || show_more.get()>{rows(g.standard_more.clone())}</Show>
                        </div>
                        <button class="disclosure" type="button" aria-expanded=move || show_more.get().to_string()
                            on:click=move |_| show_more.update(|v| *v = !*v)>
                            <span>{move || if show_more.get() { t!("voices-fewer") } else { t!("voices-more") }}</span>
                            <span class="disclosure__chev" class:is-open=move || show_more.get()><Icon icon=I::ChevronDown /></span>
                        </button>
                    </section>
                    <section class="section">
                        <h2 class="overline">{t!("voices-expressive")}</h2>
                        {move || budget().map(|b| view! {
                            <div class="budget">
                                <div class="budget__row">
                                    <span>{t!("voices-budget-today")}</span>
                                    <span class="budget__num">{t!("voices-budget-numbers", left = b.left_s, cap = b.share_s, runs = b.runs_left)}</span>
                                </div>
                                <div class="progress"><span class="progress__fill"
                                    style:width=format!("{:.1}%", b.percent_left())></span></div>
                            </div>
                        })}
                        <p class="section-hint">{t!("voices-expressive-line")}</p>
                        <div class="list-card">{rows(g.expressive)}</div>
                    </section>
                }.into_any(),
            }}
            <Sheet open=ask_open title=t!("voice-ask-title")>
                <p class="sheet-lead">
                    {move || t!("voice-ask-line", seconds = preview_gpu_seconds(PREVIEW_TEXT.chars().count()))}
                </p>
                {move || budget().map(|b| view! {
                    <p class="sheet-note">{t!("voice-ask-left", left = b.left_s, runs = b.runs_left)}</p>
                })}
                <div class="btn-row">
                    <button class="btn btn--secondary" type="button" on:click=move |_| ask_open.set(false)>{t!("action-not-now")}</button>
                    <button class="btn btn--primary" type="button" on:click=confirm_preview>{t!("voice-ask-play")}</button>
                </div>
            </Sheet>
            <Sheet open=manage_open title=Signal::derive(move || managing.with(|m| m.as_ref().map(|v| v.name.clone()).unwrap_or_default()))>
                <p class="sheet-lead">{t!("voice-manage-line")}</p>
                <button class="row-item row-item--danger" type="button" on:click=delete_voice>
                    <span class="row-item__icon"><Icon icon=I::Trash /></span>
                    <span class="row-item__text"><span class="row-item__label">{t!("voice-delete")}</span></span>
                </button>
            </Sheet>
        </div>
    }
}
