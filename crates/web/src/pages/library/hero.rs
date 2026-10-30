//! "Continue listening": the most recent unfinished document, one tap from playing.

use leptos::prelude::*;
use leptos::task::spawn_local;
use nolabs_core::proto::api::DocumentSummary;

use super::meta::{fraction, time_line};
use crate::components::cover::{Cover, CoverSize};
use crate::components::icons::{I, Icon, Spinner};
use crate::state::player::OpenRequest;
use crate::state::use_app;

#[component]
pub fn Hero(doc: DocumentSummary) -> impl IntoView {
    let app = use_app();
    let player = app.player;
    let id = doc.id.clone();
    let id_open = id.clone();
    let id_play = id.clone();
    let rate = app.settings.speed;
    let doc_for_line = doc.clone();
    let is_current = move || player.doc_id().as_deref() == Some(id.as_str());
    let opening = {
        let id = doc.id.clone();
        move || player.opening.get().as_deref() == Some(id.as_str())
    };
    let is_current_play = is_current.clone();
    let play = move |e: leptos::ev::MouseEvent| {
        e.stop_propagation();
        if is_current_play() {
            player.toggle();
        } else {
            spawn_local(player.open(OpenRequest::doc(id_play.clone(), true), app));
        }
    };
    let shows_pause = move || is_current() && player.shows_pause();
    let shows_pause_label = shows_pause.clone();
    view! {
        <section class="hero" aria-label="Continue listening">
            <button class="hero__open" type="button" on:click=move |_| app.nav.go(&format!("/read/{id_open}"))>
                <Cover title=doc.meta.title.clone() size=CoverSize::Large />
                <span class="hero__text">
                    <span class="eyebrow">"Continue listening"</span>
                    <span class="hero__title">{doc.meta.title.clone()}</span>
                    {doc.meta.author.clone().map(|a| view! { <span class="hero__author">{a}</span> })}
                    <span class="hero__meta">{move || time_line(&doc_for_line, rate.get())}</span>
                </span>
            </button>
            <div class="hero__foot">
                <div class="bar"><div class="bar__fill" style:width=format!("{:.2}%", fraction(&doc) * 100.0)></div></div>
                <button class="play-btn" type="button" aria-label=move || if shows_pause_label() { "Pause" } else { "Play" } on:click=play>
                    {move || if opening() {
                        view! { <Spinner /> }.into_any()
                    } else if shows_pause() {
                        view! { <Icon icon=I::Pause /> }.into_any()
                    } else {
                        view! { <Icon icon=I::Play class="play-btn__play" /> }.into_any()
                    }}
                </button>
            </div>
        </section>
    }
}
