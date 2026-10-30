//! One module per route.

pub mod library;
pub mod login;
pub mod reader;
pub mod record;
pub mod settings;
pub mod voices;
pub mod welcome;

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::icons::I;
use crate::components::skeleton::Empty;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="page">
            <Empty icon=I::Book title="Nothing here" line="That page doesn't exist.">
                <A href="/" attr:class="btn">"Back to the library"</A>
            </Empty>
        </div>
    }
}
