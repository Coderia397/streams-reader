//! "Continue listening": the most recent unfinished document, one tap from playing.

use crate::model::DocumentSummary;
use leptos::prelude::*;
use leptos::task::spawn_local;

use super::meta::{fraction, time_line};
use crate::components::cover::{Cover, CoverSize};
use crate::components::icon::{I, Icon, Spinner};
use crate::state::player::OpenRequest;
use crate::state::use_app;

#[component]
pub fn Hero(doc: DocumentSummary) -> impl IntoView {
    let app = use_app();
    let player = app.player;
    let id = StoredValue::new(doc.id.clone());
    let rate = app.settings.speed;
    let line_doc = doc.clone();
    let is_current = move || id.with_value(|id| player.doc_id().as_deref() == Some(id.as_str()));
    let opening = move || id.with_value(|id| player.opening.get().as_deref() == Some(id.as_str()));
    let play = move |e: leptos::ev::MouseEvent| {
        e.stop_propagation();
        if is_current() {
            player.toggle();
        } else {
            spawn_local(player.open(OpenRequest::doc(id.get_value(), true), app));
        }
    };
    let shows_pause = move || is_current() && player.shows_pause();
    let pct = fraction(&doc) * 100.0;
    view! {
        <section class="hero" aria-label=t!("library-continue")>
            <button class="hero__open" type="button" on:click=move |_| app.nav.go(&format!("/read/{}", id.get_value()))>
                <Cover title=doc.meta.title.clone() size=CoverSize::Large />
                <span class="hero__text">
                    <span class="overline">{t!("library-continue")}</span>
                    <span class="hero__title"><bdi>{doc.meta.title.clone()}</bdi></span>
                    {doc.meta.author.clone().map(|a| view! { <span class="hero__author"><bdi>{a}</bdi></span> })}
                    <span class="hero__meta">{move || time_line(&line_doc, rate.get())}</span>
                </span>
            </button>
            <div class="hero__foot">
                <div class="progress progress--playback" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow=format!("{pct:.0}")>
                    <span class="progress__fill" style:width=format!("{pct:.2}%")></span>
                </div>
                <button class="play-btn play-btn--small" type="button"
                    aria-label=move || if shows_pause() { t!("player-pause") } else { t!("player-play") } on:click=play>
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
