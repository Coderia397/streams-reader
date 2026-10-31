//! Loading placeholders.

use leptos::prelude::*;

#[component]
pub fn RowSkeleton(#[prop(default = 4)] count: usize) -> impl IntoView {
    view! {
        <div class="skeleton-list" aria-hidden="true">
            {(0..count).map(|_| view! {
                <div class="doc-row doc-row--skeleton">
                    <div class="skel skel--cover"></div>
                    <div class="doc-row__body">
                        <div class="skel skel--line"></div>
                        <div class="skel skel--line skel--short"></div>
                    </div>
                </div>
            }).collect_view()}
        </div>
    }
}
