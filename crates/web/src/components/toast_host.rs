//! Renders the toast (one at a time). Errors are announced assertively, everything else politely.
//! An open sheet renders its own copy (`in_sheet`): a modal dialog makes the app's region inert, so the copy inside
//! the dialog is the one people can see, reach and hear.

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::icon::{I, Icon};
use crate::interop::pwa;
use crate::state::toast::{ToastAction, ToastKind};
use crate::state::use_app;

#[component]
pub fn ToastHost(#[prop(optional)] in_sheet: bool) -> impl IntoView {
    let toasts = use_app().toasts;
    let urgent = move || toasts.list.with(|l| l.iter().any(|t| t.kind == ToastKind::Error));
    view! {
        <div class="toasts" class:toasts--sheet=in_sheet role="status" aria-live=move || if urgent() { "assertive" } else { "polite" }>
            <For each=move || toasts.list.get() key=|t| t.id let:toast>
                {
                    let id = toast.id;
                    let (class, icon) = match toast.kind {
                        ToastKind::Info => ("toast", None),
                        ToastKind::Success => ("toast toast--success", Some(I::CheckCircle)),
                        ToastKind::Error => ("toast toast--error", Some(I::Alert)),
                    };
                    view! {
                        <div class=class>
                            {icon.map(|i| view! { <Icon icon=i class="toast__icon" /> })}
                            <p class="toast__text">{toast.text.clone()}</p>
                            {toast.action.map(|action| view! {
                                <button class="toast__action" type="button" on:click=move |_| match action {
                                    ToastAction::Update => {
                                        toasts.dismiss(id);
                                        spawn_local(pwa::apply_update());
                                    }
                                    ToastAction::Undo => toasts.run_undo(id),
                                }>{action.label()}</button>
                            })}
                            <button class="toast__close" type="button" aria-label=t!("action-dismiss") on:click=move |_| toasts.dismiss(id)>
                                <Icon icon=I::Close />
                            </button>
                        </div>
                    }
                }
            </For>
        </div>
    }
}
