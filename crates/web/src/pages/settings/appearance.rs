//! Theme and interface language.

use leptos::prelude::*;

use crate::components::controls::{Group, NavRow, Segmented};
use crate::components::icon::{I, Icon};
use crate::components::sheet::Sheet;
use crate::i18n::{self, Lang};
use crate::state::profile::push_locale;
use crate::state::settings::Theme;
use crate::state::use_app;

#[component]
pub fn AppearanceGroup() -> impl IntoView {
    let app = use_app();
    let lang_open = RwSignal::new(false);
    let follows = i18n::follows_browser();
    let current = i18n::current();
    let current_name = if follows {
        t!("language-auto-named", language = current.native_name())
    } else {
        current.native_name().to_owned()
    };
    let row = move |choice: Option<Lang>, label: String, selected: bool| {
        view! {
            <button class="row-item" type="button" aria-pressed=selected.to_string() on:click=move |_| {
                let code = choice.unwrap_or_else(i18n::browser_lang).code();
                leptos::task::spawn_local(async move {
                    push_locale(code).await;
                    i18n::choose(choice);
                });
            }>
                <span class="row-item__text"><span class="row-item__label">{label}</span></span>
                {selected.then(|| view! { <Icon icon=I::Check class="row-item__check" /> })}
            </button>
        }
    };
    view! {
        <Group title=t!("settings-appearance")>
            <div class="row-item row-item--stack">
                <span class="row-item__head">
                    <span class="row-item__icon"><Icon icon=I::Theme /></span>
                    <span class="row-item__label">{t!("settings-theme")}</span>
                </span>
                <Segmented
                    label=t!("settings-theme")
                    options=Theme::ALL.iter().map(|&t| (t, t.label())).collect()
                    value=app.settings.theme.into()
                    on_pick=Callback::new(move |t| app.settings.theme.set(t))
                />
            </div>
            <NavRow icon=I::Language label=t!("settings-language") value=Signal::stored(current_name) chevron=true
                on_click=Callback::new(move |()| lang_open.set(true)) />
        </Group>
        <Sheet open=lang_open title=t!("settings-language") tall=true>
            <div class="stack stack--tight">
                {row(None, t!("language-auto"), follows)}
                {Lang::ALL.into_iter().map(|l| row(Some(l), l.native_name().to_owned(), !follows && l == current)).collect_view()}
            </div>
        </Sheet>
    }
}
