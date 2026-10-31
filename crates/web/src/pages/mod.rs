//! One module per route.

pub mod auth_callback;
pub mod library;
pub mod reader;
pub mod record;
pub mod settings;
pub mod sign_in;
pub mod voices;
pub mod welcome;

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::icon::I;
use crate::components::spot::Empty;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="page">
            <Empty icon=I::BookOpen title=t!("not-found-title") line=t!("not-found-line")>
                <A href="/" attr:class="btn btn--secondary">{t!("not-found-back")}</A>
            </Empty>
        </div>
    }
}
