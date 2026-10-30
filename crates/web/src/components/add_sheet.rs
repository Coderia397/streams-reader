//! "Add" sheet: paste text, a link, or a file. Parsing happens in the browser; the result is uploaded and opened.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlInputElement;

use super::icons::{I, Icon, Spinner};
use super::sheet::Sheet;
use crate::api::client::js_error_text;
use crate::import_flow::source::{ACCEPT, normalise_url};
use crate::import_flow::{ImportSource, Stage, import};
use crate::state::{AppState, use_app};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Paste,
    Link,
    File,
}

/// Runs an import with progress in `busy`; on success the document opens in the reader.
pub fn start_import(app: AppState, src: ImportSource, busy: RwSignal<Option<Stage>>, on_done: impl Fn() + 'static) {
    if busy.get_untracked().is_some() {
        return;
    }
    busy.set(Some(Stage::Reading));
    spawn_local(async move {
        let voice = app.settings.voice.get_untracked();
        let result = import(src, voice, &|stage| busy.set(Some(stage))).await;
        busy.set(None);
        match result {
            Ok(doc) => {
                let id = doc.id.clone();
                app.library.upsert(doc);
                on_done();
                app.toasts.success("Added to your library.");
                app.nav.go(&format!("/read/{id}"));
            }
            Err(e) => app.toasts.error(e.friendly()),
        }
    });
}

async fn read_file(input: &HtmlInputElement) -> Result<Option<(String, Vec<u8>)>, String> {
    let Some(file) = input.files().and_then(|f| f.get(0)) else { return Ok(None) };
    let buf = JsFuture::from(file.array_buffer()).await.map_err(js_error_text)?;
    Ok(Some((file.name(), js_sys::Uint8Array::new(&buf).to_vec())))
}

#[component]
pub fn AddSheet(open: RwSignal<bool>) -> impl IntoView {
    let app = use_app();
    let tab = RwSignal::new(Tab::Paste);
    let text = RwSignal::new(String::new());
    let title = RwSignal::new(String::new());
    let url = RwSignal::new(String::new());
    let busy = RwSignal::new(None::<Stage>);
    let done = move || {
        open.set(false);
        text.set(String::new());
        title.set(String::new());
        url.set(String::new());
    };
    let add_paste = move |_| {
        let body = text.get_untracked();
        if body.trim().is_empty() {
            app.toasts.info("Paste some text first.");
            return;
        }
        let t = title.get_untracked();
        let title = (!t.trim().is_empty()).then(|| t.trim().to_owned());
        start_import(app, ImportSource::Paste { text: body, title }, busy, done);
    };
    let add_link = move |_| match normalise_url(&url.get_untracked()) {
        Some(u) => start_import(app, ImportSource::Link { url: u }, busy, done),
        None => app.toasts.info("That doesn't look like a web address."),
    };
    let on_file = move |e: leptos::ev::Event| {
        let Some(input) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) else { return };
        spawn_local(async move {
            match read_file(&input).await {
                Ok(Some((name, bytes))) => start_import(app, ImportSource::File { name, bytes }, busy, done),
                Ok(None) => {}
                Err(e) => app.toasts.error(format!("Couldn't open that file: {e}")),
            }
            input.set_value("");
        });
    };
    let tab_btn = move |t: Tab, icon: I, label: &'static str| {
        view! {
            <button
                class="segmented__btn"
                class:is-on=move || tab.get() == t
                type="button"
                role="tab"
                aria-selected=move || if tab.get() == t { "true" } else { "false" }
                on:click=move |_| tab.set(t)
            >
                <Icon icon=icon />
                <span>{label}</span>
            </button>
        }
    };
    view! {
        <Sheet open=open title="Add to library">
            <div class="segmented" role="tablist">
                {tab_btn(Tab::Paste, I::Text, "Text")}
                {tab_btn(Tab::Link, I::Link, "Link")}
                {tab_btn(Tab::File, I::File, "File")}
            </div>
            <Show when=move || busy.get().is_some()>
                <div class="busy" role="status">
                    <Spinner />
                    <span>{move || busy.get().map(Stage::label).unwrap_or_default()}</span>
                </div>
            </Show>
            <Show when=move || busy.get().is_none()>
                {move || match tab.get() {
                    Tab::Paste => view! {
                        <div class="form">
                            <input class="field" type="text" placeholder="Title (optional)" maxlength="200"
                                prop:value=move || title.get()
                                on:input=move |e| title.set(event_target_value(&e)) />
                            <textarea class="field field--area" placeholder="Paste an article, a chapter, anything." rows="8"
                                prop:value=move || text.get()
                                on:input=move |e| text.set(event_target_value(&e))></textarea>
                            <button class="btn btn--primary" type="button" on:click=add_paste>"Add text"</button>
                        </div>
                    }.into_any(),
                    Tab::Link => view! {
                        <div class="form">
                            <input class="field" type="url" inputmode="url" autocomplete="off" placeholder="https://"
                                prop:value=move || url.get()
                                on:input=move |e| url.set(event_target_value(&e))
                                on:keydown=move |e| if e.key() == "Enter" { add_link(()) } />
                            <p class="hint">"Articles and blog posts work best. The page is cleaned up so only the story is read."</p>
                            <button class="btn btn--primary" type="button" on:click=move |_| add_link(())>"Add link"</button>
                        </div>
                    }.into_any(),
                    Tab::File => view! {
                        <div class="form">
                            <label class="drop">
                                <Icon icon=I::Upload />
                                <span class="drop__title">"Choose a file"</span>
                                <span class="drop__hint">"EPUB, PDF, TXT, Markdown or HTML"</span>
                                <input class="visually-hidden" type="file" accept=ACCEPT on:change=on_file />
                            </label>
                        </div>
                    }.into_any(),
                }}
            </Show>
        </Sheet>
    }
}
