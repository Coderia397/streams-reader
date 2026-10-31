//! `/voices/new`: record your voice, line by line, with a live meter and per-line checks.

mod script;
mod submit;
mod takes;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::audio::analysis::meter;
use crate::components::icon::{I, Icon, Spinner};
use crate::components::spot::Spot;
use crate::i18n;
use crate::interop::recorder::{self, Recording};
use crate::state::use_app;
use crate::util::format;
use script::SCRIPT;
use submit::{Phase, submit};
use takes::Takes;

#[component]
fn LineCard(
    idx: usize,
    takes: Takes,
    active: RwSignal<Option<usize>>,
    level: RwSignal<f32>,
    on_toggle: Callback<usize>,
) -> impl IntoView {
    let line = &SCRIPT[idx];
    let info = move || takes.info.with(|v| v.get(idx).cloned().flatten());
    let is_active = move || active.get() == Some(idx);
    let busy_elsewhere = move || active.get().is_some_and(|a| a != idx);
    view! {
        <li class="line" class:is-active=is_active class:is-done=move || info().is_some_and(|t| t.usable())>
            <div class="line__head">
                <span class="line__num">{idx + 1}</span>
                <span class="line__mood">{i18n::tr(line.mood, &[])}</span>
                {move || info().map(|t| view! { <span class="line__len">{format::clock(t.duration_ms)}</span> })}
            </div>
            <p class="line__text" lang="en" dir="ltr">{line.text}</p>
            <Show when=is_active>
                <div class="meter" aria-hidden="true">
                    <div class="meter__fill" style:transform=move || format!("scaleX({:.3})", level.get())></div>
                </div>
            </Show>
            {move || info().map(|t| t.issues.iter().map(|i| view! {
                <p class="line__issue" class:is-warn=!i.blocking()><Icon icon=I::Alert />{i.message()}</p>
            }).collect_view())}
            <button
                class="btn btn--sm"
                class:btn--secondary=move || !is_active()
                class:btn--record=is_active
                type="button"
                disabled=busy_elsewhere
                on:click=move |_| on_toggle.run(idx)
            >
                {move || if is_active() {
                    view! { <Icon icon=I::Stop /><span>{t!("record-stop")}</span> }.into_any()
                } else if info().is_some() {
                    view! { <Icon icon=I::Refresh /><span>{t!("record-again")}</span> }.into_any()
                } else {
                    view! { <Icon icon=I::Mic /><span>{t!("record-start")}</span> }.into_any()
                }}
            </button>
        </li>
    }
}

#[component]
pub fn RecordPage() -> impl IntoView {
    let app = use_app();
    let takes = Takes::new();
    let name = RwSignal::new(t!("record-default-name"));
    let active = RwSignal::new(None::<usize>);
    let level = RwSignal::new(0.0f32);
    let phase = RwSignal::new(Phase::Recording);
    let rec = StoredValue::new_local(None::<Recording>);

    on_cleanup(move || {
        if rec.with_value(Option::is_some) {
            rec.set_value(None);
            spawn_local(recorder::cancel());
        }
    });

    let stop = move |idx: usize| {
        let Some(r) = rec.try_update_value(Option::take).flatten() else { return };
        spawn_local(async move {
            active.set(None);
            level.set(0.0);
            match r.stop().await {
                Ok(take) => {
                    let info = takes.set(idx, take);
                    if info.usable() && info.issues.is_empty() {
                        app.toasts.success(t!("record-line-good", line = idx + 1));
                    }
                }
                Err(e) => app.toasts.error(t!("record-failed", detail = e.to_string())),
            }
        });
    };
    let on_toggle = Callback::new(move |idx: usize| {
        if active.get_untracked() == Some(idx) {
            stop(idx);
            return;
        }
        app.player.pause();
        active.set(Some(idx));
        spawn_local(async move {
            match recorder::start(move |rms, _peak| level.set(meter(rms as f32))).await {
                Ok(r) => rec.set_value(Some(r)),
                Err(e) => {
                    active.set(None);
                    app.toasts.error(recorder::start_error_message(&e));
                }
            }
        });
    });
    let create = move |_| {
        let n = name.get_untracked().trim().to_owned();
        let n = if n.is_empty() { t!("record-default-name") } else { n };
        spawn_local(submit(n, takes, phase));
    };
    let use_voice = move |voice_id: String| {
        app.settings.use_voice(&voice_id);
        spawn_local(app.voices.refresh(app.toasts));
        app.nav.go("/voices");
    };

    view! {
        <div class="page page--record">
            <header class="topbar">
                <button class="icon-btn" type="button" aria-label=t!("action-back") on:click=move |_| app.nav.go("/voices")>
                    <Icon icon=I::Back />
                </button>
                <h1 class="topbar__title">{t!("record-title")}</h1>
                <span class="icon-btn-spacer"></span>
            </header>
            {move || match phase.get() {
                Phase::Recording => view! {
                    <section class="tips">
                        <Spot icon=I::Mic />
                        <p class="tips__line">{t!("record-tips")}</p>
                        <label class="field">
                            <span class="field__label">{t!("record-name")}</span>
                            <input class="input" type="text" maxlength="40"
                                prop:value=move || name.get()
                                on:input=move |e| name.set(event_target_value(&e)) />
                        </label>
                    </section>
                    <ol class="lines">
                        {(0..SCRIPT.len()).map(|i| view! {
                            <LineCard idx=i takes=takes active=active level=level on_toggle=on_toggle />
                        }).collect_view()}
                    </ol>
                    <div class="sticky-cta">
                        <p class="sticky-cta__count">{move || t!("record-progress", done = takes.count(), total = SCRIPT.len())}</p>
                        <button class="btn btn--primary btn--block" type="button"
                            disabled=move || !takes.all_usable() || active.get().is_some()
                            on:click=create>
                            {t!("record-create")}
                        </button>
                    </div>
                }.into_any(),
                Phase::Uploading | Phase::Preparing => view! {
                    <div class="progress-card" role="status">
                        <Spinner class="spin spin--lg" />
                        <h2>{move || if phase.get() == Phase::Uploading { t!("record-uploading") } else { t!("record-preparing") }}</h2>
                        <p class="progress-card__line">{t!("record-wait")}</p>
                    </div>
                }.into_any(),
                Phase::Ready { voice_id } => view! {
                    <div class="progress-card">
                        <Spot icon=I::CheckCircle />
                        <h2>{t!("record-ready")}</h2>
                        <p class="progress-card__line">{t!("record-ready-line")}</p>
                        <button class="btn btn--primary btn--block" type="button" on:click=move |_| use_voice(voice_id.clone())>
                            {t!("record-use")}
                        </button>
                    </div>
                }.into_any(),
                Phase::Failed(msg) => view! {
                    <div class="progress-card">
                        <Spot icon=I::Alert />
                        <h2>{t!("record-failed-title")}</h2>
                        <p class="progress-card__line">{msg}</p>
                        <button class="btn btn--secondary btn--block" type="button" on:click=move |_| phase.set(Phase::Recording)>{t!("record-back")}</button>
                    </div>
                }.into_any(),
            }}
        </div>
    }
}
