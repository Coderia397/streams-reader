//! /privacy and /terms: plain-language policies built from numbered sections in the locale files.
//! Keys: `{doc}-{section}-title` and `{doc}-{section}-{n}` for each paragraph.

use crate::config::{CONTACT_EMAIL, GITHUB_URL};
use crate::layout::{Ctx, Doc, page_intro};

/// (section id, paragraph count).
pub const PRIVACY: [(&str, usize); 8] = [
    ("summary", 1),
    ("store", 4),
    ("not", 2),
    ("processors", 2),
    ("keep", 2),
    ("cookies", 1),
    ("rights", 2),
    ("changes", 1),
];

pub const TERMS: [(&str, usize); 7] =
    [("use", 2), ("content", 2), ("voices", 2), ("fair", 1), ("open", 1), ("warranty", 2), ("changes", 1)];

pub fn privacy(ctx: &Ctx) -> Doc {
    policy(ctx, "privacy", &PRIVACY)
}

pub fn terms(ctx: &Ctx) -> Doc {
    policy(ctx, "terms", &TERMS)
}

fn policy(ctx: &Ctx, doc: &str, sections: &[(&str, usize)]) -> Doc {
    let email = format!("<a href=\"mailto:{CONTACT_EMAIL}\">{CONTACT_EMAIL}</a>");
    let github = format!("<a href=\"{GITHUB_URL}\">GitHub</a>");
    let body_sections: String = sections
        .iter()
        .map(|(id, n)| {
            let paras: String = (1..=*n)
                .map(|i| {
                    format!(
                        "<p>{}</p>",
                        ctx.cat.t_html(&format!("{doc}-{id}-{i}"), &[("email", &email), ("github", &github)])
                    )
                })
                .collect();
            format!("<section><h2 id=\"{id}\">{}</h2>{paras}</section>\n", ctx.t(&format!("{doc}-{id}-title")))
        })
        .collect();
    let body = format!(
        "{intro}<article class=\"wrap prose\">\n<p class=\"prose__updated\">{updated}</p>\n{body_sections}</article>\n",
        intro = page_intro(&ctx.t("legal-eyebrow"), &ctx.t(&format!("{doc}-title")), &ctx.t(&format!("{doc}-lede"))),
        updated = ctx.t("legal-updated"),
    );
    Doc {
        title: super::title(ctx, &format!("{doc}-title")),
        description: ctx.cat.text(&format!("{doc}-meta-description")),
        body,
        json_ld: vec![],
        script: false,
    }
}
