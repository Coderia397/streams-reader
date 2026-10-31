//! Skip back / forward buttons; the length comes from Settings and the icon shows it.

use leptos::prelude::*;

use crate::components::icon::{I, Icon};
use crate::state::use_app;

pub fn back_icon(seconds: u32) -> I {
    match seconds {
        5 => I::Replay5,
        10 => I::Replay10,
        30 => I::Replay30,
        _ => I::Replay15,
    }
}

pub fn forward_icon(seconds: u32) -> I {
    match seconds {
        5 => I::Forward5,
        10 => I::Forward10,
        15 => I::Forward15,
        _ => I::Forward30,
    }
}

#[component]
pub fn SkipButton(forward: bool, #[prop(optional)] class: &'static str) -> impl IntoView {
    let app = use_app();
    let seconds = move || if forward { app.settings.skip_forward_s.get() } else { app.settings.skip_back_s.get() };
    let label = move || {
        if forward {
            t!("player-skip-forward", seconds = seconds())
        } else {
            t!("player-skip-back", seconds = seconds())
        }
    };
    let class =
        if class.is_empty() { "icon-btn icon-btn--lg".to_owned() } else { format!("icon-btn icon-btn--lg {class}") };
    view! {
        <button class=class type="button" aria-label=label on:click=move |_| {
            let s = seconds() as i32;
            app.player.skip(if forward { s } else { -s });
        }>
            {move || view! { <Icon icon=if forward { forward_icon(seconds()) } else { back_icon(seconds()) } /> }}
        </button>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SKIP_CHOICES_S;

    #[test]
    fn every_skip_choice_has_its_own_icon() {
        let backs: Vec<I> = SKIP_CHOICES_S.iter().map(|&s| back_icon(s)).collect();
        let fwds: Vec<I> = SKIP_CHOICES_S.iter().map(|&s| forward_icon(s)).collect();
        for (i, a) in backs.iter().enumerate() {
            assert!(backs[i + 1..].iter().all(|b| b != a));
        }
        for (i, a) in fwds.iter().enumerate() {
            assert!(fwds[i + 1..].iter().all(|b| b != a));
        }
    }
}
