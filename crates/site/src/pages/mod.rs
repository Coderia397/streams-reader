//! One module per page. Each turns the translated strings into a [`Doc`] for the shared shell.

mod credits;
mod download;
mod help;
mod home;
mod legal;
mod not_found;
pub mod parts;

use crate::config::Page;
use crate::layout::{Ctx, Doc};

pub fn render(ctx: &Ctx) -> Doc {
    match ctx.page {
        Page::Home => home::render(ctx),
        Page::Download => download::render(ctx),
        Page::Help => help::render(ctx),
        Page::Privacy => legal::privacy(ctx),
        Page::Terms => legal::terms(ctx),
        Page::Credits => credits::render(ctx),
        Page::NotFound => not_found::render(ctx),
    }
}

/// `<title>`: "Page · Streams Reader", or the full landing title for the home page.
pub fn title(ctx: &Ctx, key: &str) -> String {
    format!("{} · {}", ctx.cat.text(key), ctx.cat.text("brand-name"))
}
