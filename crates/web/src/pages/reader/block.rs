//! One block of the reading view, split into tappable sentence spans. Text is always inserted as text nodes. The
//! sentence being read gets the wash; the word being spoken, the ember pill.

use std::sync::Arc;

use leptos::prelude::*;
use nolabs_core::proto::api::BlockKind;

use super::ReaderCtx;
use crate::reading::pieces::{Piece, block_pieces, split_word};
use crate::state::player::OpenDoc;
use crate::state::use_app;

#[component]
fn Seg(idx: u32, text: String, current: Selector<u32>) -> impl IntoView {
    let player = use_app().player;
    let ctx = expect_context::<ReaderCtx>();
    let text = Arc::new(text);
    let is_current = {
        let current = current.clone();
        move || current.selected(&idx)
    };
    let content = {
        let is_current = is_current.clone();
        let text = Arc::clone(&text);
        move || {
            if !is_current() {
                return text.to_string().into_any();
            }
            let range = player.word.get().and_then(|w| player.words.with(|ws| ws.get(w).map(|t| (t.2, t.3))));
            match range {
                Some((start, end)) => {
                    let (before, word, after) = split_word(&text, start, end);
                    view! {
                        {before.to_owned()}
                        <span class="word">{word.to_owned()}</span>
                        {after.to_owned()}
                    }
                    .into_any()
                }
                None => text.to_string().into_any(),
            }
        }
    };
    view! {
        <span
            class="seg"
            class:is-current=is_current
            class:is-picked=move || ctx.picked.get().is_some_and(|p| p.segment == idx)
            data-seg=idx
            on:click=move |e| ctx.tap(idx, &e)
        >
            {content}
        </span>
    }
}

#[component]
pub fn BlockView(doc: Arc<OpenDoc>, block: u32, current: Selector<u32>) -> impl IntoView {
    let model = &doc.model;
    let Some(b) = model.blocks.get(block as usize) else { return ().into_any() };
    let offset = model.block_offsets.get(block as usize).copied().unwrap_or(0);
    let pieces = block_pieces(&b.text, offset, model.segments_of_block(block));
    let content = pieces
        .into_iter()
        .map(|p| match p {
            Piece::Gap(t) => t.into_any(),
            Piece::Seg { idx, text } => view! { <Seg idx=idx text=text current=current.clone() /> }.into_any(),
        })
        .collect_view();
    match b.kind {
        BlockKind::Heading => view! { <h2 class="r-h">{content}</h2> }.into_any(),
        BlockKind::Paragraph => view! { <p class="r-p">{content}</p> }.into_any(),
        BlockKind::Quote => view! { <blockquote class="r-q"><p>{content}</p></blockquote> }.into_any(),
        BlockKind::ListItem => view! { <p class="r-li">{content}</p> }.into_any(),
    }
}
