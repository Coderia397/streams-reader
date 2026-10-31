//! Bookmarks in the reader's More sheet: save the current place, list the saved ones (the sentence they start in),
//! jump to one, remove one. Rows live in Supabase (`public.bookmarks`).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::controls::NavRow;
use crate::components::icon::{I, Icon};
use crate::db::bookmarks::{self, Bookmark};
use crate::state::use_app;

/// The first words of the sentence at `offset`, for the list.
pub fn snippet(text: &str, max_chars: usize) -> String {
    let t = text.trim();
    if t.chars().count() <= max_chars {
        return t.to_owned();
    }
    // One char more than allowed, so a word that ends exactly at the limit is kept.
    let cut: String = t.chars().take(max_chars + 1).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head).trim_end_matches([',', ';', ':']);
    format!("{cut}…")
}

/// Whether a saved bookmark already points into segment `seg` (`segment_of` maps a char offset to its segment), so
/// saving the same place twice does not add a second, identical row.
pub fn saved_in_segment(saved: &[Bookmark], seg: u32, segment_of: impl Fn(u32) -> u32) -> bool {
    saved.iter().any(|b| segment_of(u32::try_from(b.char_offset).unwrap_or(u32::MAX)) == seg)
}

#[component]
pub fn Bookmarks(open: RwSignal<bool>, on_jump: Callback<u32>) -> impl IntoView {
    let app = use_app();
    let player = app.player;
    let list = RwSignal::new(Vec::<Bookmark>::new());
    // Whether `list` holds the saved rows yet (they load when the sheet opens).
    let loaded = StoredValue::new(false);
    let doc_id = move || player.doc_id();
    Effect::new(move |_| {
        if !open.get() {
            return;
        }
        let Some(id) = doc_id() else { return };
        spawn_local(async move {
            if let Ok(rows) = bookmarks::list(&id).await {
                let _ = list.try_set(rows);
                let _ = loaded.try_set_value(true);
            }
        });
    });
    // True while a save is in flight, so a second tap cannot add the same place before the first one lands.
    let saving = StoredValue::new(false);
    let add = Callback::new(move |()| {
        let (Some(id), Some(offset)) = (player.doc_id(), player.current_offset()) else { return };
        if saving.get_value() {
            return;
        }
        saving.set_value(true);
        let seg = player.segment.get_untracked();
        spawn_local(async move {
            // A tap before the list arrived must still see the rows already saved.
            // The sheet may close while this runs, so the reactive values are only touched with `try_`.
            if loaded.try_get_value() == Some(false)
                && let Ok(rows) = bookmarks::list(&id).await
            {
                let _ = list.try_set(rows);
                let _ = loaded.try_set_value(true);
            }
            let exists = player.doc.with_untracked(|d| {
                d.as_ref().is_some_and(|d| {
                    list.try_with_untracked(|l| saved_in_segment(l, seg, |o| d.model.segment_for_offset(o)))
                        .unwrap_or(false)
                })
            });
            if exists {
                app.toasts.info(t!("bookmark-exists"));
            } else {
                match bookmarks::add(&id, u64::from(offset)).await {
                    Ok(b) => {
                        let _ = list.try_update(|l| {
                            l.push(b);
                            l.sort_by_key(|b| b.char_offset);
                        });
                        app.toasts.success(t!("bookmark-added"));
                    }
                    Err(e) => app.toasts.api_error(&e),
                }
            }
            let _ = saving.try_set_value(false);
        });
    });
    let remove = move |b: Bookmark| {
        let Some(id) = b.id.clone() else { return };
        list.update(|l| l.retain(|x| x.id.as_deref() != Some(id.as_str())));
        spawn_local(async move {
            if let Err(e) = bookmarks::remove(&id).await {
                app.toasts.api_error(&e);
            }
        });
    };
    let rows = move || {
        let items = list.get();
        player.doc.with(|d| {
            let Some(d) = d.as_ref() else { return Vec::new() };
            items
                .into_iter()
                .map(|b| {
                    let seg = d.model.segment_for_offset(u32::try_from(b.char_offset).unwrap_or(u32::MAX));
                    let text = d.model.segments.get(seg as usize).map(|s| snippet(&s.text, 72)).unwrap_or_default();
                    (b, seg, text)
                })
                .collect::<Vec<_>>()
        })
    };
    view! {
        <NavRow icon=I::Bookmark label=t!("bookmark-add") on_click=add />
        {move || rows().into_iter().map(|(b, seg, text)| {
            let gone = b.clone();
            view! {
                <div class="row-item row-item--static bookmark">
                    <button class="bookmark__jump" type="button" on:click=move |_| { open.set(false); on_jump.run(seg); }>
                        <span class="row-item__icon"><Icon icon=I::Bookmark /></span>
                        <span class="bookmark__text"><bdi>{text}</bdi></span>
                    </button>
                    <button class="icon-btn" type="button" aria-label=t!("bookmark-remove") on:click=move |_| remove(gone.clone())>
                        <Icon icon=I::Close />
                    </button>
                </div>
            }
        }).collect_view()}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets_cut_at_a_word() {
        assert_eq!(snippet("Short one.", 72), "Short one.");
        assert_eq!(snippet("The rain had stopped by the time she reached the station", 20), "The rain had stopped…");
    }

    fn mark(offset: u64) -> Bookmark {
        Bookmark { id: None, document_id: "d".into(), char_offset: offset, note: None, created_at: None }
    }

    #[test]
    fn a_place_is_saved_once_per_sentence() {
        // Segments of 100 chars each.
        let seg_of = |o: u32| o / 100;
        let saved = [mark(40), mark(250)];
        assert!(saved_in_segment(&saved, 0, seg_of));
        assert!(saved_in_segment(&saved, 2, seg_of));
        assert!(!saved_in_segment(&saved, 1, seg_of));
        assert!(!saved_in_segment(&[], 0, seg_of));
    }
}
