//! Storage, the voice engine's status, and About (setup, source, privacy, version).

use leptos::prelude::*;
use leptos::task::spawn_local;
use nolabs_core::proto::api::SystemStatus;

use crate::components::controls::{Group, InfoRow, NavRow};
use crate::components::icon::{I, Spinner};
use crate::components::sheet::Sheet;
use crate::config::{PRIVACY_URL, SOURCE_URL};
use crate::state::use_app;
use crate::state::voices::GpuBudget;
use crate::storage::{audio_cache, prefs};
use crate::util::{dom, format};

pub fn stage_class(stage: &str, healthy: bool) -> &'static str {
    match (stage, healthy) {
        ("RUNNING", true) => "dot dot--ok",
        ("RUNNING", false) | ("SLEEPING" | "PAUSED" | "BUILDING" | "APP_STARTING" | "RUNNING_BUILDING", _) => {
            "dot dot--warn"
        }
        _ => "dot dot--bad",
    }
}

pub fn stage_label(stage: &str) -> String {
    match stage {
        "RUNNING" => t!("engine-awake"),
        "SLEEPING" | "PAUSED" => t!("engine-asleep"),
        "BUILDING" | "RUNNING_BUILDING" | "APP_STARTING" => t!("engine-starting"),
        _ => t!("engine-unavailable"),
    }
}

fn open_url(url: &str) {
    if let Some(w) = dom::window() {
        let _ = w.open_with_url_and_target(url, "_blank");
    }
}

#[component]
pub fn StorageGroup() -> impl IntoView {
    let app = use_app();
    let storage = RwSignal::new(None::<(f64, f64)>);
    let clear_open = RwSignal::new(false);
    spawn_local(async move { storage.set(audio_cache::estimate().await) });
    let clear = move |_| {
        clear_open.set(false);
        spawn_local(async move {
            match audio_cache::clear().await {
                Ok(()) => app.toasts.success(t!("storage-cleared")),
                Err(e) => app.toasts.api_error(&e),
            }
            storage.set(audio_cache::estimate().await);
        });
    };
    view! {
        <Group title=t!("settings-storage") hint=t!("storage-hint")>
            <InfoRow icon=I::Storage label=t!("storage-used")
                value=Signal::derive(move || storage.get().map_or_else(|| "…".to_owned(), |(used, _)| format::bytes(used))) />
            <NavRow icon=I::Trash label=t!("storage-clear") chevron=true on_click=Callback::new(move |()| clear_open.set(true)) />
        </Group>
        <Sheet open=clear_open title=t!("storage-clear-title")>
            <p class="sheet-lead">{t!("storage-clear-line")}</p>
            <div class="btn-row">
                <button class="btn btn--secondary" type="button" on:click=move |_| clear_open.set(false)>{t!("action-cancel")}</button>
                <button class="btn btn--danger" type="button" on:click=clear>{t!("storage-clear-confirm")}</button>
            </div>
        </Sheet>
    }
}

#[component]
fn StatusRows(status: SystemStatus) -> impl IntoView {
    let budget = GpuBudget::of(&status);
    let pct = budget.percent_left();
    view! {
        <div class="row-item row-item--static">
            <span class="row-item__icon"><i class=stage_class(&status.space.stage, status.space.healthy)></i></span>
            <span class="row-item__text"><span class="row-item__label">{t!("engine-voice")}</span></span>
            <span class="row-item__value">{stage_label(&status.space.stage)}</span>
        </div>
        <div class="row-item row-item--stack row-item--static">
            <span class="row-item__head">
                <span class="row-item__icon"><crate::components::icon::Icon icon=I::Sparkle /></span>
                // The numbers sit under the label: a value column would cut them in the longer languages.
                <span class="row-item__text">
                    <span class="row-item__label">{t!("engine-gpu-today")}</span>
                    <span class="row-item__hint">{t!("engine-gpu-left", left = budget.left_s, cap = budget.share_s, runs = budget.runs_left)}</span>
                </span>
            </span>
            <div class="progress"><span class="progress__fill" style:width=format!("{pct:.1}%")></span></div>
        </div>
        <InfoRow icon=I::List label=t!("engine-queue") value=Signal::stored(status.queued.to_string()) />
        <InfoRow icon=I::Info label=t!("about-version") value=Signal::stored(status.version.clone()) />
    }
}

#[component]
pub fn EngineGroup() -> impl IntoView {
    let app = use_app();
    spawn_local(app.voices.refresh_status());
    view! {
        <Group title=t!("settings-engine")>
            {move || match app.voices.status.get() {
                Some(s) => view! { <StatusRows status=s /> }.into_any(),
                None => view! { <div class="row-item row-item--static center-stack"><Spinner /></div> }.into_any(),
            }}
            <NavRow icon=I::Refresh label=t!("engine-refresh") on_click=Callback::new(move |()| spawn_local(app.voices.refresh_status())) />
        </Group>
    }
}

#[component]
pub fn AboutGroup() -> impl IntoView {
    let app = use_app();
    view! {
        <Group title=t!("settings-about")>
            <NavRow icon=I::Phone label=t!("about-setup") chevron=true on_click=Callback::new(move |()| {
                prefs::remove(prefs::WELCOMED);
                app.nav.go("/welcome");
            }) />
            <NavRow icon=I::GitHub label=t!("about-source") chevron=true on_click=Callback::new(|()| open_url(SOURCE_URL)) />
            <NavRow icon=I::Shield label=t!("legal-privacy") chevron=true on_click=Callback::new(|()| open_url(PRIVACY_URL)) />
        </Group>
        <p class="colophon">{t!("about-colophon")}</p>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_labels_and_classes() {
        assert_eq!(stage_label("RUNNING"), "Awake");
        assert_eq!(stage_label("RUNTIME_ERROR"), "Unavailable");
        assert!(stage_class("RUNNING", true).ends_with("ok"));
        assert!(stage_class("SLEEPING", false).ends_with("warn"));
        assert!(stage_class("RUNTIME_ERROR", false).ends_with("bad"));
    }
}
