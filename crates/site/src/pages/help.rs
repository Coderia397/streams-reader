//! /help: a short help centre, grouped questions with answers, and how to reach a person.

use crate::config::CONTACT_EMAIL;
use crate::icons::icon;
use crate::layout::{Ctx, Doc, page_intro};
use crate::pages::parts;

/// Groups of questions: (group key, icon, question ids). Keys: `help-group-{group}`, `help-q-{id}`, `help-a-{id}`.
pub const GROUPS: [(&str, &str, &[&str]); 4] = [
    ("start", "book-open-line", &["add", "files", "play", "install"]),
    ("voices", "user-voice-line", &["change-voice", "expressive", "clone", "speed"]),
    ("account", "lock-line", &["account", "data", "delete"]),
    ("trouble", "question-line", &["stops", "no-audio", "wrong-text"]),
];

pub fn all_ids() -> Vec<&'static str> {
    GROUPS.iter().flat_map(|(_, _, ids)| ids.iter().copied()).collect()
}

pub fn render(ctx: &Ctx) -> Doc {
    let toc: String = GROUPS
        .iter()
        .map(|(g, ic, _)| {
            format!(
                "<li><a class=\"toc__link\" href=\"#{g}\">{}{}</a></li>",
                icon(ic),
                ctx.t(&format!("help-group-{g}"))
            )
        })
        .collect();
    let groups: String = GROUPS
        .iter()
        .map(|(g, _, ids)| {
            format!(
                "<section class=\"help-group\" id=\"{g}\" aria-labelledby=\"{g}-title\"><h2 class=\"help-group__title\" id=\"{g}-title\">{}</h2>{}</section>\n",
                ctx.t(&format!("help-group-{g}")),
                parts::faq_list(ctx, "help", ids),
            )
        })
        .collect();
    let contact = format!(
        "<aside class=\"contact\" aria-labelledby=\"contact-title\"><span class=\"contact__icon\">{}</span><div><h2 class=\"contact__title\" id=\"contact-title\">{}</h2>\
         <p>{}</p></div><a class=\"btn btn--secondary\" href=\"mailto:{CONTACT_EMAIL}\">{CONTACT_EMAIL}</a></aside>",
        icon("mail-line"),
        ctx.t("help-contact-title"),
        ctx.t("help-contact-text"),
    );
    let body = format!(
        "{intro}<div class=\"wrap section section--tight section--narrow\"><nav aria-label=\"{toc_label}\"><ul class=\"toc\">{toc}</ul></nav>\n{groups}{contact}</div>\n",
        intro = page_intro(&ctx.t("help-eyebrow"), &ctx.t("help-title"), &ctx.t("help-lede")),
        toc_label = ctx.t("help-toc-label"),
    );
    Doc {
        title: super::title(ctx, "help-title"),
        description: ctx.cat.text("help-meta-description"),
        body,
        json_ld: vec![parts::faq_json_ld(ctx, "help", &all_ids())],
        script: false,
    }
}
