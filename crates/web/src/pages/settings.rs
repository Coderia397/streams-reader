//! `/settings`: playback defaults, theme, engine status, storage, account.

use leptos::prelude::*;
use leptos::task::spawn_local;
use nolabs_core::proto::api::SystemStatus;

use crate::api::session;
use crate::components::icons::{I, Icon, Spinner};
use crate::components::sheet::Sheet;
use crate::config::{APP_NAME, SPEED_CHIPS};
use crate::state::settings::Theme;
use crate::state::use_app;
use crate::state::voices::gpu_left;
use crate::storage::{audio_cache, prefs};
use crate::util::format;

fn stage_class(stage: &str, healthy: bool) -> &'static str {
    match (stage, healthy) {
        ("RUNNING", true) => "status-dot status-dot--ok",
        ("RUNNING", false) | ("SLEEPING" | "PAUSED" | "BUILDING" | "APP_STARTING" | "RUNNING_BUILDING", _) => {
            "status-dot status-dot--warn"
        }
        _ => "status-dot status-dot--bad",
    }
}

fn stage_label(stage: &str) -> String {
    match stage {
        "RUNNING" => "Awake".to_owned(),
        "SLEEPING" | "PAUSED" => "Asleep (wakes on first use)".to_owned(),
        "BUILDING" | "RUNNING_BUILDING" | "APP_STARTING" => "Starting up".to_owned(),
        other => other.to_lowercase().replace('_', " "),
    }
}

#[component]
fn StatusCard(status: SystemStatus) -> impl IntoView {
    let (left, runs) = gpu_left(&status);
    let left_pct = f64::from(left) / f64::from(status.gpu.cap_s.max(1)) * 100.0;
    view! {
        <div class="card">
            <div class="kv">
                <span>"Voice engine"</span>
                <span class="kv__val"><i class=stage_class(&status.space.stage, status.space.healthy)></i>{stage_label(&status.space.stage)}</span>
            </div>
            <div class="kv">
                <span>"GPU today"</span>
                <span class="kv__val">{format!("{left} s left of {}", status.gpu.cap_s)}</span>
            </div>
            <div class="bar"><div class="bar__fill bar__fill--warm" style:width=format!("{left_pct:.1}%")></div></div>
            <div class="kv">
                <span>"GPU runs left"</span>
                <span class="kv__val">{format!("{runs} of {}", status.gpu.runs_cap)}</span>
            </div>
            <div class="kv">
                <span>"Waiting to render"</span>
                <span class="kv__val">{status.queued}</span>
            </div>
            <div class="kv kv--muted">
                <span>"Version"</span>
                <span class="kv__val">{status.version.clone()}</span>
            </div>
        </div>
    }
}

#[component]
pub fn SettingsPage() -> impl IntoView {
    let app = use_app();
    let storage = RwSignal::new(None::<(f64, f64)>);
    let clear_open = RwSignal::new(false);
    let refresh_storage = move || spawn_local(async move { storage.set(audio_cache::estimate().await) });
    spawn_local(app.voices.refresh_status());
    spawn_local(app.voices.refresh(app.toasts));
    refresh_storage();

    let clear_cache = move |_| {
        clear_open.set(false);
        spawn_local(async move {
            match audio_cache::clear().await {
                Ok(()) => app.toasts.success("Saved audio cleared."),
                Err(e) => app.toasts.api_error(&e),
            }
            storage.set(audio_cache::estimate().await);
        });
    };
    let sign_out = move |_| {
        spawn_local(async move {
            app.player.close();
            let _ = session::logout().await;
            app.session.auth.set(crate::state::session::Auth::SignedOut);
            app.nav.replace("/login");
        });
    };
    let rerun_setup = move |_| {
        prefs::remove(prefs::WELCOMED);
        app.nav.go("/welcome");
    };
    let theme_btn = move |t: Theme| {
        view! {
            <button class="segmented__btn" class:is-on=move || app.settings.theme.get() == t type="button"
                aria-pressed=move || if app.settings.theme.get() == t { "true" } else { "false" }
                on:click=move |_| app.settings.theme.set(t)>
                {t.label()}
            </button>
        }
    };

    view! {
        <div class="page">
            <header class="topbar"><h1 class="page-title page-title--bar">"Settings"</h1></header>

            <section class="section">
                <h2 class="section-label">"Listening"</h2>
                <div class="card card--list">
                    <button class="list-item" type="button" on:click=move |_| app.nav.go("/voices")>
                        <Icon icon=I::Voices />
                        <span class="list-item__text">"Default voice"</span>
                        <span class="list-item__value">{move || app.voices.name(&app.settings.voice.get())}</span>
                        <Icon icon=I::ChevronRight />
                    </button>
                    <div class="list-item list-item--stack">
                        <div class="list-item__row">
                            <Icon icon=I::Gauge />
                            <span class="list-item__text">"Default speed"</span>
                            <span class="list-item__value">{move || format::speed(app.settings.speed.get())}</span>
                        </div>
                        <div class="chips">
                            {SPEED_CHIPS.iter().map(|&s| view! {
                                <button class="chip" class:is-on=move || (app.settings.speed.get() - s).abs() < 0.001 type="button"
                                    on:click=move |_| {
                                        app.settings.speed.set(s);
                                        app.player.set_rate(s);
                                    }>{format::speed(s)}</button>
                            }).collect_view()}
                        </div>
                    </div>
                </div>
            </section>

            <section class="section">
                <h2 class="section-label">"Appearance"</h2>
                <div class="segmented segmented--wide" role="group" aria-label="Theme">
                    {theme_btn(Theme::Ink)}
                    {theme_btn(Theme::Paper)}
                    {theme_btn(Theme::System)}
                </div>
            </section>

            <section class="section">
                <div class="list-head">
                    <h2 class="section-label">"Engine"</h2>
                    <button class="icon-btn" type="button" aria-label="Refresh status" on:click=move |_| spawn_local(app.voices.refresh_status())>
                        <Icon icon=I::Refresh />
                    </button>
                </div>
                {move || match app.voices.status.get() {
                    Some(s) => view! { <StatusCard status=s /> }.into_any(),
                    None => view! { <div class="card center-spinner"><Spinner /></div> }.into_any(),
                }}
            </section>

            <section class="section">
                <h2 class="section-label">"Storage"</h2>
                <div class="card card--list">
                    <div class="list-item">
                        <Icon icon=I::Storage />
                        <span class="list-item__text">"Used on this phone"</span>
                        <span class="list-item__value">{move || storage.get().map_or_else(|| "…".to_owned(), |(used, _)| format::bytes(used))}</span>
                    </div>
                    <button class="list-item" type="button" on:click=move |_| clear_open.set(true)>
                        <Icon icon=I::Trash />
                        <span class="list-item__text">"Clear saved audio"</span>
                        <Icon icon=I::ChevronRight />
                    </button>
                </div>
            </section>

            <section class="section">
                <h2 class="section-label">"Account"</h2>
                <div class="card card--list">
                    <button class="list-item" type="button" on:click=rerun_setup>
                        <Icon icon=I::Phone />
                        <span class="list-item__text">"Install and battery setup"</span>
                        <Icon icon=I::ChevronRight />
                    </button>
                    <button class="list-item list-item--danger" type="button" on:click=sign_out>
                        <Icon icon=I::LogOut />
                        <span class="list-item__text">"Sign out"</span>
                    </button>
                </div>
            </section>
            <p class="colophon">{APP_NAME}" · free, private, yours"</p>

            <Sheet open=clear_open title="Clear saved audio?">
                <p class="sheet-lead">"Clips you've listened to are kept so they replay offline. Clearing frees space; they download again when needed."</p>
                <div class="btn-row">
                    <button class="btn" type="button" on:click=move |_| clear_open.set(false)>"Cancel"</button>
                    <button class="btn btn--danger" type="button" on:click=clear_cache>"Clear"</button>
                </div>
            </Sheet>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_labels_and_classes() {
        assert_eq!(stage_label("RUNNING"), "Awake");
        assert_eq!(stage_label("RUNTIME_ERROR"), "runtime error");
        assert!(stage_class("RUNNING", true).ends_with("ok"));
        assert!(stage_class("SLEEPING", false).ends_with("warn"));
        assert!(stage_class("RUNTIME_ERROR", false).ends_with("bad"));
    }
}
