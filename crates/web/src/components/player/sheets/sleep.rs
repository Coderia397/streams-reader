//! Sleep timer: minutes, the end of the chapter, or off; the player fades out over the last 10 s. Applies at once
//! with Undo.

use leptos::prelude::*;

use crate::components::icon::{I, Icon};
use crate::components::sheet::Sheet;
use crate::config::SLEEP_CHOICES_MIN;
use crate::state::player::SleepChoice;
use crate::state::use_app;
use crate::util::format;

pub fn label(choice: SleepChoice) -> String {
    match choice {
        SleepChoice::Off => t!("sleep-off"),
        SleepChoice::Minutes(m) => t!("sleep-minutes", minutes = m),
        SleepChoice::EndOfChapter => t!("sleep-end-chapter"),
    }
}

#[component]
pub fn SleepSheet(open: RwSignal<bool>) -> impl IntoView {
    let app = use_app();
    let player = app.player;
    let pick = move |choice: SleepChoice| {
        let before = player.sleep.get_untracked();
        player.set_sleep(choice);
        open.set(false);
        if choice != before {
            let text =
                if choice == SleepChoice::Off { t!("sleep-cleared") } else { t!("sleep-set", choice = label(choice)) };
            app.toasts.undoable(text, move || player.set_sleep(before));
        }
    };
    let option = move |choice: SleepChoice| {
        view! {
            <button class="chip chip--lg" type="button"
                aria-pressed=move || if player.sleep.get() == choice { "true" } else { "false" }
                on:click=move |_| pick(choice)>
                {label(choice)}
            </button>
        }
    };
    view! {
        <Sheet open=open title=t!("sleep-title")>
            {move || player.sleep_left_ms.get().map(|ms| view! {
                <p class="sleep__left"><Icon icon=I::Timer />{t!("sleep-stops-in", time = format::clock(ms))}</p>
            })}
            <div class="chip-row chip-row--wrap">
                {SLEEP_CHOICES_MIN.iter().map(|&m| option(SleepChoice::Minutes(m))).collect_view()}
                {option(SleepChoice::EndOfChapter)}
                {option(SleepChoice::Off)}
            </div>
            <p class="sheet-note">{t!("sleep-hint")}</p>
        </Sheet>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(label(SleepChoice::Minutes(45)), "45 min");
        assert_eq!(label(SleepChoice::EndOfChapter), "End of chapter");
    }
}
