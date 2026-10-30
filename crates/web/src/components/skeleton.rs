//! Loading placeholders and empty states.

use leptos::prelude::*;

use super::icons::{I, Icon};

#[component]
pub fn RowSkeleton(#[prop(default = 4)] count: usize) -> impl IntoView {
    view! {
        <div class="skeleton-list" aria-hidden="true">
            {(0..count).map(|_| view! {
                <div class="row row--skeleton">
                    <div class="skel skel--cover"></div>
                    <div class="row__body">
                        <div class="skel skel--line"></div>
                        <div class="skel skel--line skel--short"></div>
                    </div>
                </div>
            }).collect_view()}
        </div>
    }
}

#[component]
pub fn Empty(
    icon: I,
    #[prop(into)] title: String,
    #[prop(into)] line: String,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="empty">
            <div class="empty__mark"><Icon icon=icon /></div>
            <h2 class="empty__title">{title}</h2>
            <p class="empty__line">{line}</p>
            {children.map(|c| c())}
        </div>
    }
}
