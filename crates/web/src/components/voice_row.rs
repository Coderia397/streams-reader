//! One voice in a list: avatar, name, accent and description, a check when selected, and optional trailing controls
//! (preview button on the Voices page).

use leptos::prelude::*;
use nolabs_core::proto::api::{VoiceInfo, VoiceState};

use super::cover::hues;
use super::icons::{I, Icon};

pub fn voice_meta(v: &VoiceInfo) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(a) = &v.accent {
        parts.push(a.clone());
    }
    if let Some(g) = &v.gender {
        parts.push(g.clone());
    }
    match v.state {
        VoiceState::Ready => {}
        VoiceState::AwaitingReference => parts.push("needs a recording".into()),
        VoiceState::Preparing => parts.push("preparing…".into()),
        VoiceState::Failed => parts.push("failed".into()),
    }
    parts.join(" · ")
}

#[component]
pub fn VoiceAvatar(#[prop(into)] name: String) -> impl IntoView {
    let (a, b) = hues(&name);
    let style = format!("background: linear-gradient(140deg, hsl({a} 55% 46%), hsl({b} 60% 30%));");
    let initial: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    view! { <span class="avatar" style=style aria-hidden="true">{initial}</span> }
}

#[component]
pub fn VoiceRow(
    voice: VoiceInfo,
    #[prop(into)] selected: Signal<bool>,
    on_select: Callback<String>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let id = voice.id.clone();
    let usable = voice.state == VoiceState::Ready;
    let meta = voice_meta(&voice);
    view! {
        <div class="voice-row" class:is-selected=move || selected.get()>
            <button
                class="voice-row__main"
                type="button"
                disabled=!usable
                aria-pressed=move || if selected.get() { "true" } else { "false" }
                on:click=move |_| on_select.run(id.clone())
            >
                <VoiceAvatar name=voice.name.clone() />
                <span class="voice-row__text">
                    <span class="voice-row__name">{voice.name.clone()}</span>
                    <span class="voice-row__meta">{meta}</span>
                    {(!voice.description.is_empty()).then(|| view! {
                        <span class="voice-row__desc">{voice.description.clone()}</span>
                    })}
                </span>
                <span class="voice-row__check" aria-hidden="true">
                    <Show when=move || selected.get()><Icon icon=I::Check /></Show>
                </span>
            </button>
            {children.map(|c| view! { <div class="voice-row__extra">{c()}</div> })}
        </div>
    }
}
