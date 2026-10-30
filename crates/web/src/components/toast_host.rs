//! Renders the toast stack.

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::icons::{I, Icon};
use crate::interop::pwa;
use crate::state::toast::{ToastAction, ToastKind};
use crate::state::use_app;

#[component]
pub fn ToastHost() -> impl IntoView {
    let app = use_app();
    let toasts = app.toasts;
    view! {
        <div class="toasts" role="status" aria-live="polite">
            <For each=move || toasts.list.get() key=|t| t.id let:toast>
                {
                    let id = toast.id;
                    let kind = match toast.kind {
                        ToastKind::Info => "toast",
                        ToastKind::Success => "toast toast--success",
                        ToastKind::Error => "toast toast--error",
                    };
                    let icon = match toast.kind {
                        ToastKind::Error => I::Alert,
                        ToastKind::Success => I::Check,
                        ToastKind::Info => I::Sparkles,
                    };
                    view! {
                        <div class=kind>
                            <Icon icon=icon class="toast__icon" />
                            <p class="toast__text">{toast.text.clone()}</p>
                            {toast.action.map(|action| view! {
                                <button class="toast__action" type="button" on:click=move |_| {
                                    toasts.dismiss(id);
                                    match action {
                                        ToastAction::Update => spawn_local(pwa::apply_update()),
                                    }
                                }>{action.label()}</button>
                            })}
                            <button class="toast__close" type="button" aria-label="Dismiss" on:click=move |_| toasts.dismiss(id)>
                                <Icon icon=I::X />
                            </button>
                        </div>
                    }
                }
            </For>
        </div>
    }
}
