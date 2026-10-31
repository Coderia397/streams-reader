//! `/settings`: account, listening defaults, appearance and language, storage, engine status, about.

mod about;
mod account;
mod appearance;
mod listening;

use leptos::prelude::*;

use about::{AboutGroup, EngineGroup, StorageGroup};
use account::AccountGroup;
use appearance::AppearanceGroup;
use listening::ListeningGroup;

#[component]
pub fn SettingsPage() -> impl IntoView {
    view! {
        <div class="page">
            <header class="page-head"><h1 class="page-title">{t!("settings-title")}</h1></header>
            <AccountGroup />
            <ListeningGroup />
            <AppearanceGroup />
            <StorageGroup />
            <EngineGroup />
            <AboutGroup />
        </div>
    }
}
