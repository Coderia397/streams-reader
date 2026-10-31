//! The reader's More sheet: contents, search, bookmarks, listen without the text, text size and (on desktop) the keyboard
//! shortcuts. Everything applies at once.

use leptos::prelude::*;

use super::bookmarks::Bookmarks;
use super::keys::HELP;
use crate::components::controls::{NavRow, Toggle};
use crate::components::icon::{I, Icon};
use crate::components::sheet::Sheet;
use crate::config::{TEXT_SIZE_MAX, TEXT_SIZE_MIN};
use crate::i18n;
use crate::state::use_app;
use crate::util::dom;

#[component]
pub fn MoreSheet(
    open: RwSignal<bool>,
    hide_text: RwSignal<bool>,
    on_contents: Callback<()>,
    on_search: Callback<()>,
    on_jump: Callback<u32>,
) -> impl IntoView {
    let app = use_app();
    let size = app.settings.text_size;
    let has_chapters = move || app.player.has_chapters();
    let then = move |cb: Callback<()>| {
        Callback::new(move |()| {
            open.set(false);
            cb.run(());
        })
    };
    view! {
        <Sheet open=open title=t!("reader-more")>
            <div class="stack stack--tight">
                <Show when=has_chapters>
                    <NavRow icon=I::Contents label=t!("contents-title") chevron=true on_click=then(on_contents) />
                </Show>
                <NavRow icon=I::Search label=t!("search-doc-title") chevron=true on_click=then(on_search) />
                <Bookmarks open=open on_jump=on_jump />
                <Toggle
                    icon=I::HideText
                    label=t!("reader-hide-text")
                    hint=t!("reader-hide-text-hint")
                    on=hide_text.into()
                    on_change=Callback::new(move |v| hide_text.set(v))
                />
                <div class="row-item row-item--static">
                    <span class="row-item__icon"><Icon icon=I::TextSize /></span>
                    <span class="row-item__text"><span class="row-item__label">{t!("reader-text-size")}</span></span>
                    <div class="stepper">
                        <button class="icon-btn" type="button" aria-label=t!("reader-text-smaller")
                            disabled={move || size.get() <= TEXT_SIZE_MIN} on:click=move |_| size.update(|s| *s = (*s - 1).max(TEXT_SIZE_MIN))>
                            <span class="stepper__a stepper__a--small" aria-hidden="true">"A"</span>
                        </button>
                        <span class="stepper__value">{move || size.get()}</span>
                        <button class="icon-btn" type="button" aria-label=t!("reader-text-larger")
                            disabled={move || size.get() >= TEXT_SIZE_MAX} on:click=move |_| size.update(|s| *s = (*s + 1).min(TEXT_SIZE_MAX))>
                            <span class="stepper__a" aria-hidden="true">"A"</span>
                        </button>
                    </div>
                </div>
            </div>
            {dom::is_desktop().then(|| view! {
                <section class="sheet-section">
                    <h3 class="overline">{t!("keys-title")}</h3>
                    <dl class="keys">
                        {HELP.iter().map(|(keys, msg)| view! {
                            <div class="keys__row"><dt><kbd>{*keys}</kbd></dt><dd>{i18n::tr(msg, &[])}</dd></div>
                        }).collect_view()}
                    </dl>
                </section>
            })}
        </Sheet>
    }
}
