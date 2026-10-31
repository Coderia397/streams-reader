//! /credits: the models, libraries, fonts and icons Streams Reader is built on, with their licences.

use crate::html::esc;
use crate::layout::{Ctx, Doc, page_intro};

/// One credited work. `role` is a translation key; names and licences are not translated.
pub struct Credit {
    pub name: &'static str,
    pub by: &'static str,
    pub licence: &'static str,
    pub url: &'static str,
    pub role: &'static str,
}

const fn c(
    name: &'static str,
    by: &'static str,
    licence: &'static str,
    url: &'static str,
    role: &'static str,
) -> Credit {
    Credit { name, by, licence, url, role }
}

/// (group key, credits).
pub const GROUPS: [(&str, &[Credit]); 3] = [
    (
        "models",
        &[
            c("Kokoro-82M", "hexgrad", "Apache-2.0", "https://huggingface.co/hexgrad/Kokoro-82M", "credit-role-kokoro"),
            c(
                "Ming-3.0-tts-0.5B",
                "inclusionAI",
                "MIT",
                "https://huggingface.co/inclusionAI/Ming-3.0-tts-0.5B",
                "credit-role-ming",
            ),
            c("misaki", "hexgrad", "Apache-2.0", "https://github.com/hexgrad/misaki", "credit-role-misaki"),
            c(
                "eSpeak NG",
                "eSpeak NG contributors",
                "GPL-3.0",
                "https://github.com/espeak-ng/espeak-ng",
                "credit-role-espeak",
            ),
        ],
    ),
    (
        "software",
        &[
            c("Rust", "The Rust Project", "MIT / Apache-2.0", "https://www.rust-lang.org", "credit-role-rust"),
            c("Leptos", "Leptos contributors", "MIT", "https://github.com/leptos-rs/leptos", "credit-role-leptos"),
            c(
                "workers-rs",
                "Cloudflare",
                "Apache-2.0",
                "https://github.com/cloudflare/workers-rs",
                "credit-role-workers",
            ),
            c("PDF.js", "Mozilla", "Apache-2.0", "https://github.com/mozilla/pdf.js", "credit-role-pdfjs"),
            c(
                "fluent-rs",
                "Project Fluent",
                "Apache-2.0 / MIT",
                "https://github.com/projectfluent/fluent-rs",
                "credit-role-fluent",
            ),
            c("grass", "Connor Skees", "MIT", "https://github.com/connorskees/grass", "credit-role-grass"),
            c("PyTorch", "PyTorch Foundation", "BSD-3-Clause", "https://pytorch.org", "credit-role-torch"),
            c("Gradio", "Hugging Face", "Apache-2.0", "https://github.com/gradio-app/gradio", "credit-role-gradio"),
        ],
    ),
    (
        "design",
        &[
            c("Inter", "Rasmus Andersson", "OFL-1.1", "https://rsms.me/inter/", "credit-role-inter"),
            c("Literata", "TypeTogether", "OFL-1.1", "https://github.com/googlefonts/literata", "credit-role-literata"),
            c("Remix Icon", "Remix Design", "Remix Icon License v1.0", "https://remixicon.com", "credit-role-remix"),
            c(
                "Pride and Prejudice",
                "Jane Austen",
                "Public domain",
                "https://www.gutenberg.org/ebooks/1342",
                "credit-role-austen",
            ),
        ],
    ),
];

pub fn render(ctx: &Ctx) -> Doc {
    let groups: String = GROUPS
        .iter()
        .map(|(g, credits)| {
            let rows: String = credits
                .iter()
                .map(|cr| {
                    format!(
                        "<li class=\"credit\"><div class=\"credit__main\"><a class=\"credit__name\" href=\"{}\">{}</a>\
                         <span class=\"credit__by\">{}</span></div><p class=\"credit__role\">{}</p><span class=\"credit__licence\">{}</span></li>",
                        cr.url,
                        esc(cr.name),
                        esc(cr.by),
                        ctx.t(cr.role),
                        esc(cr.licence),
                    )
                })
                .collect();
            format!(
                "<section class=\"credits-group\" aria-labelledby=\"{g}-title\"><h2 class=\"help-group__title\" id=\"{g}-title\">{}</h2><ul class=\"credits\">{rows}</ul></section>\n",
                ctx.t(&format!("credits-group-{g}"))
            )
        })
        .collect();
    let body = format!(
        "{intro}<div class=\"wrap section section--tight section--narrow\">{groups}<p class=\"credits__note\">{note}</p></div>\n",
        intro = page_intro(&ctx.t("legal-eyebrow"), &ctx.t("credits-title"), &ctx.t("credits-lede")),
        note = ctx.t("credits-note"),
    );
    Doc {
        title: super::title(ctx, "credits-title"),
        description: ctx.cat.text("credits-meta-description"),
        body,
        json_ld: vec![],
        script: false,
    }
}
