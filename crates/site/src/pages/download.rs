//! /download: the web app today, the native apps as honest "coming soon" cards, and how to install the web app.

use crate::icons::icon;
use crate::layout::{Ctx, Doc, page_intro};
use crate::pages::parts::{self, PLATFORMS};

pub fn render(ctx: &Ctx) -> Doc {
    let cards: String = PLATFORMS.iter().map(|p| parts::platform_card(ctx, p, true)).collect();
    let install: String = ["android", "desktop", "apple"]
        .iter()
        .map(|k| {
            format!(
                "<li class=\"howto\"><h3 class=\"howto__title\">{}</h3><p>{}</p></li>",
                ctx.t(&format!("install-{k}-title")),
                ctx.t(&format!("install-{k}-text")),
            )
        })
        .collect();
    let coming: String = [("headphone-line", "screen"), ("wifi-off-line", "offline"), ("file-text-line", "files"), ("download-2-line", "updates")]
        .iter()
        .map(|(ic, k)| {
            format!("<li class=\"feature\"><span class=\"feature__icon\">{}</span><div><p class=\"feature__text\">{}</p></div></li>", icon(ic), ctx.t(&format!("apps-{k}")))
        })
        .collect();
    let body = format!(
        "{intro}<section class=\"wrap section section--tight\" aria-label=\"{list_label}\"><ul class=\"platforms platforms--detail\">{cards}</ul></section>\n\
         <section class=\"wrap section\" aria-labelledby=\"install-title\">{install_head}<ol class=\"howtos\">{install}</ol></section>\n\
         <section class=\"wrap section\" aria-labelledby=\"apps-title\">{apps_head}<ul class=\"features features--two\">{coming}</ul></section>\n{cta}",
        intro = page_intro(&ctx.t("download-eyebrow"), &ctx.t("download-title"), &ctx.t("download-lede")),
        list_label = ctx.t("download-list-label"),
        install_head = parts::section_head(ctx, "install", "install-eyebrow", "install-title", Some("install-lede")),
        apps_head = parts::section_head(ctx, "apps", "apps-eyebrow", "apps-title", None),
        cta = parts::cta_band(ctx),
    );
    Doc {
        title: super::title(ctx, "download-title"),
        description: ctx.cat.text("download-meta-description"),
        body,
        json_ld: vec![],
        script: true,
    }
}
