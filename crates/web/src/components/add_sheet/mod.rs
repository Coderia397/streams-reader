//! "Add" sheet: every way in, in one place (paste text, a link, a file). Parsing happens in the browser; the result
//! is uploaded and opened in the reader. The forms are in `forms.rs`.

mod forms;

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlInputElement;

use self::forms::{FormCtx, LinkForm, PasteForm};
use super::icon::{I, Icon, Spinner};
use super::sheet::Sheet;
use crate::api::client::js_error_text;
use crate::import_flow::source::ACCEPT;
use crate::import_flow::{ImportSource, Stage, import, worker};
use crate::state::{AppState, use_app};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddMode {
    Paste,
    Link,
    File,
}

impl AddMode {
    pub const ALL: [AddMode; 3] = [AddMode::Paste, AddMode::Link, AddMode::File];

    pub fn icon(self) -> I {
        match self {
            AddMode::Paste => I::Text,
            AddMode::Link => I::Link,
            AddMode::File => I::Upload,
        }
    }

    pub fn label(self) -> String {
        match self {
            AddMode::Paste => t!("add-paste"),
            AddMode::Link => t!("add-link"),
            AddMode::File => t!("add-file"),
        }
    }

    fn hint(self) -> String {
        match self {
            AddMode::Paste => t!("add-paste-hint"),
            AddMode::Link => t!("add-link-hint"),
            AddMode::File => t!("add-file-hint"),
        }
    }
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
                app.toasts.success(t!("add-done"));
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

/// `mode`: `None` shows the list of ways in; a mode shows its form.
#[component]
pub fn AddSheet(open: RwSignal<bool>, mode: RwSignal<Option<AddMode>>) -> impl IntoView {
    let app = use_app();
    let text = RwSignal::new(String::new());
    let title = RwSignal::new(String::new());
    let url = RwSignal::new(String::new());
    let busy = RwSignal::new(None::<Stage>);
    let error = RwSignal::new(None::<String>);
    let done = move || {
        open.set(false);
        mode.set(None);
        text.set(String::new());
        title.set(String::new());
        url.set(String::new());
    };
    // A message about one form's field does not follow to another form or to the next time the sheet opens.
    Effect::new(move |_| {
        mode.track();
        open.track();
        error.set(None);
    });
    // The parsers load in a worker; start it while the person picks what to add.
    Effect::new(move |_| {
        if open.get() {
            worker::warm();
        }
    });
    let ctx =
        FormCtx { error, busy, done: Callback::new(move |()| done()), back: Callback::new(move |()| mode.set(None)) };
    let on_file = move |e: leptos::ev::Event| {
        let Some(input) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) else { return };
        spawn_local(async move {
            match read_file(&input).await {
                Ok(Some((name, bytes))) => start_import(app, ImportSource::File { name, bytes }, busy, done),
                Ok(None) => {}
                Err(e) => app.toasts.error(t!("add-file-error", detail = e)),
            }
            input.set_value("");
        });
    };
    let sheet_title = Signal::derive(move || mode.get().map_or_else(|| t!("add-title"), AddMode::label));

    let chooser = move || {
        view! {
            <ul class="choice-list">
                {AddMode::ALL.into_iter().map(|m| view! {
                    <li>
                        {if m == AddMode::File {
                            view! {
                                <label class="choice">
                                    <span class="choice__icon"><Icon icon=m.icon() /></span>
                                    <span class="choice__text">
                                        <span class="choice__label">{m.label()}</span>
                                        <span class="choice__hint">{m.hint()}</span>
                                    </span>
                                    <input class="visually-hidden" type="file" accept=ACCEPT on:change=on_file />
                                </label>
                            }.into_any()
                        } else {
                            view! {
                                <button class="choice" type="button" on:click=move |_| mode.set(Some(m))>
                                    <span class="choice__icon"><Icon icon=m.icon() /></span>
                                    <span class="choice__text">
                                        <span class="choice__label">{m.label()}</span>
                                        <span class="choice__hint">{m.hint()}</span>
                                    </span>
                                </button>
                            }.into_any()
                        }}
                    </li>
                }).collect_view()}
            </ul>
            <p class="sheet-note">{t!("add-share-tip")}</p>
        }
    };

    view! {
        <Sheet open=open title=sheet_title>
            <Show when=move || busy.get().is_some()>
                <div class="busy" role="status">
                    <Spinner />
                    <span>{move || busy.get().map(Stage::label).unwrap_or_default()}</span>
                </div>
            </Show>
            <Show when=move || busy.get().is_none()>
                {move || match mode.get() {
                    None | Some(AddMode::File) => chooser().into_any(),
                    Some(AddMode::Paste) => view! { <PasteForm ctx=ctx title=title text=text /> }.into_any(),
                    Some(AddMode::Link) => view! { <LinkForm ctx=ctx url=url /> }.into_any(),
                }}
            </Show>
        </Sheet>
    }
}
