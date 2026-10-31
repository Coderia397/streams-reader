//! One voice in a list: an orb (gradient from the voice's name), name and descriptor, an engine badge, a heart for
//! favourites, optional trailing controls (preview) and a check when selected.

use leptos::prelude::*;
use nolabs_core::proto::api::{Engine, VoiceInfo, VoiceState};

use super::cover::hues;
use super::icon::{I, Icon};
use crate::state::use_app;

/// "American · Female" plus the state of a recorded voice.
pub fn voice_meta(v: &VoiceInfo) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(a) = &v.accent {
        parts.push(match a.as_str() {
            "American" => t!("accent-american"),
            "British" => t!("accent-british"),
            other => other.to_owned(),
        });
    }
    if let Some(g) = &v.gender {
        parts.push(match g.as_str() {
            "female" => t!("gender-female"),
            "male" => t!("gender-male"),
            other => other.to_owned(),
        });
    }
    match v.state {
        VoiceState::Ready => {}
        VoiceState::AwaitingReference => parts.push(t!("voice-state-needs-recording")),
        VoiceState::Preparing => parts.push(t!("voice-state-preparing")),
        VoiceState::Failed => parts.push(t!("voice-state-failed")),
    }
    parts.join(" · ")
}

/// The voice's orb: a soft sphere in two hues from its name.
#[component]
pub fn Orb(#[prop(into)] name: String, #[prop(optional)] small: bool) -> impl IntoView {
    let (a, b) = hues(&name);
    let style = format!("--hue-a: {a}; --hue-b: {b};");
    view! { <span class="orb" class:orb--small=small style=style aria-hidden="true"></span> }
}

#[component]
pub fn VoiceRow(
    voice: VoiceInfo,
    #[prop(into)] selected: Signal<bool>,
    on_select: Callback<String>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let settings = use_app().settings;
    let id = voice.id.clone();
    let fav_id = voice.id.clone();
    let fav_id_toggle = voice.id.clone();
    let usable = voice.state == VoiceState::Ready;
    let meta = voice_meta(&voice);
    let expressive = voice.engine == Engine::Ming;
    let is_fav = move || settings.is_favourite(&fav_id);
    let is_fav_label = is_fav.clone();
    let heart_label = move || if is_fav_label() { t!("voice-unfavourite") } else { t!("voice-favourite") };
    view! {
        <div class="voice-row" class:is-selected=move || selected.get()>
            <button
                class="voice-row__main"
                type="button"
                disabled=!usable
                aria-pressed=move || if selected.get() { "true" } else { "false" }
                on:click=move |_| on_select.run(id.clone())
            >
                <Orb name=voice.name.clone() />
                <span class="voice-row__text">
                    <span class="voice-row__name">
                        <span class="voice-row__label" dir="auto">{voice.name.clone()}</span>
                        {expressive.then(|| view! { <span class="badge badge--brand">{t!("voice-badge-expressive")}</span> })}
                    </span>
                    <span class="voice-row__meta">{meta}</span>
                </span>
                <Show when=move || selected.get()>
                    <Icon icon=I::Check class="voice-row__check" />
                </Show>
            </button>
            <div class="voice-row__extra">
                {children.map(|c| c())}
                <button
                    class="icon-btn voice-row__heart"
                    class:is-on=is_fav.clone()
                    type="button"
                    aria-label=heart_label
                    aria-pressed=move || if is_fav() { "true" } else { "false" }
                    on:click=move |_| settings.toggle_favourite(&fav_id_toggle)
                >
                    {move || if settings.is_favourite(&voice.id) {
                        view! { <Icon icon=I::HeartFill /> }
                    } else {
                        view! { <Icon icon=I::Heart /> }
                    }}
                </button>
            </div>
        </div>
    }
}
