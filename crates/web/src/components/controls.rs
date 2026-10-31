//! Small controls used across screens, each defined once: segmented choice, switch, settings row, section.

use leptos::prelude::*;

use super::icon::{I, Icon};

/// A row of mutually exclusive options. `options` are `(value, label)`.
#[component]
pub fn Segmented<T>(
    #[prop(into)] label: String,
    options: Vec<(T, String)>,
    value: Signal<T>,
    on_pick: Callback<T>,
) -> impl IntoView
where
    T: Copy + PartialEq + Send + Sync + 'static,
{
    view! {
        <div class="segmented" role="radiogroup" aria-label=label>
            {options.into_iter().map(|(v, text)| view! {
                <button
                    class="segmented__opt"
                    type="button"
                    role="radio"
                    aria-checked=move || if value.get() == v { "true" } else { "false" }
                    on:click=move |_| on_pick.run(v)
                >{text}</button>
            }).collect_view()}
        </div>
    }
}

/// An on/off switch with its label (the whole row toggles).
#[component]
pub fn Toggle(
    #[prop(into)] label: String,
    #[prop(optional, into)] hint: Option<String>,
    #[prop(optional)] icon: Option<I>,
    on: Signal<bool>,
    on_change: Callback<bool>,
) -> impl IntoView {
    view! {
        <button
            class="row-item"
            type="button"
            role="switch"
            aria-checked=move || if on.get() { "true" } else { "false" }
            on:click=move |_| on_change.run(!on.get_untracked())
        >
            {icon.map(|i| view! { <span class="row-item__icon"><Icon icon=i /></span> })}
            <span class="row-item__text">
                <span class="row-item__label">{label}</span>
                {hint.map(|h| view! { <span class="row-item__hint">{h}</span> })}
            </span>
            <span class="switch" aria-hidden="true"><span class="switch__knob"></span></span>
        </button>
    }
}

/// A tappable settings row: icon, label, optional value, chevron.
#[component]
pub fn NavRow(
    icon: I,
    #[prop(into)] label: String,
    #[prop(optional, into)] value: Option<Signal<String>>,
    #[prop(optional)] danger: bool,
    #[prop(optional)] chevron: bool,
    on_click: Callback<()>,
) -> impl IntoView {
    view! {
        <button class="row-item" class:row-item--danger=danger type="button" on:click=move |_| on_click.run(())>
            <span class="row-item__icon"><Icon icon=icon /></span>
            <span class="row-item__text"><span class="row-item__label">{label}</span></span>
            {value.map(|v| view! { <span class="row-item__value">{move || v.get()}</span> })}
            {chevron.then(|| view! { <Icon icon=I::Forward class="row-item__chev" /> })}
        </button>
    }
}

/// A static row: icon, label, value (no action).
#[component]
pub fn InfoRow(icon: I, #[prop(into)] label: String, #[prop(into)] value: Signal<String>) -> impl IntoView {
    view! {
        <div class="row-item row-item--static">
            <span class="row-item__icon"><Icon icon=icon /></span>
            <span class="row-item__text"><span class="row-item__label">{label}</span></span>
            <span class="row-item__value">{move || value.get()}</span>
        </div>
    }
}

/// A titled group of rows on a card.
#[component]
pub fn Group(
    #[prop(optional, into)] title: Option<String>,
    #[prop(optional, into)] hint: Option<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <section class="group">
            {title.map(|t| view! { <h2 class="group__title">{t}</h2> })}
            <div class="group__card">{children()}</div>
            {hint.map(|h| view! { <p class="group__hint">{h}</p> })}
        </section>
    }
}
