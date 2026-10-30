//! Sleep timer: 15/30/45/60 minutes or the end of the chapter; the player fades out over the last 10 s.

use leptos::prelude::*;

use crate::components::icons::{I, Icon};
use crate::components::sheet::Sheet;
use crate::config::SLEEP_CHOICES_MIN;
use crate::state::player::SleepChoice;
use crate::state::use_app;
use crate::util::format;

fn label(choice: SleepChoice) -> String {
    match choice {
        SleepChoice::Off => "Off".to_owned(),
        SleepChoice::Minutes(m) => format!("{m} minutes"),
        SleepChoice::EndOfChapter => "End of chapter".to_owned(),
    }
}

fn choices() -> Vec<SleepChoice> {
    std::iter::once(SleepChoice::Off)
        .chain(SLEEP_CHOICES_MIN.iter().map(|&m| SleepChoice::Minutes(m)))
        .chain(std::iter::once(SleepChoice::EndOfChapter))
        .collect()
}

#[component]
pub fn SleepSheet(open: RwSignal<bool>) -> impl IntoView {
    let player = use_app().player;
    view! {
        <Sheet open=open title="Sleep timer">
            {move || player.sleep_left_ms.get().map(|ms| view! {
                <p class="sleep__left">{format!("Stops in {}", format::clock(ms))}</p>
            })}
            <ul class="options">
                {choices().into_iter().map(|choice| view! {
                    <li>
                        <button
                            class="option"
                            class:is-on=move || player.sleep.get() == choice
                            type="button"
                            on:click=move |_| {
                                player.set_sleep(choice);
                                open.set(false);
                            }
                        >
                            <span>{label(choice)}</span>
                            <Show when=move || player.sleep.get() == choice><Icon icon=I::Check /></Show>
                        </button>
                    </li>
                }).collect_view()}
            </ul>
        </Sheet>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(label(SleepChoice::Minutes(45)), "45 minutes");
        assert_eq!(label(SleepChoice::EndOfChapter), "End of chapter");
    }
}
