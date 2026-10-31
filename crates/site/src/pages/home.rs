//! The landing page: hero with the live demo, how it works, voices, features, free/private/open, platforms, FAQ.

use crate::config::{APP_PATH, Page};
use crate::icons::{icon, icon_with_class};
use crate::layout::{Ctx, Doc};
use crate::pages::parts::{self, PLATFORMS};

/// The demo passage (public domain). Must match the words in `assets/demo/*.json` (checked by a test).
pub const PASSAGE: &str = "It is a truth universally acknowledged, that a single man in possession of a good fortune, \
                           must be in want of a wife.";
/// Index of the word lit in the still frame shown before the demo plays (and without script).
const STILL_WORD: usize = 4;

/// Demo voices: file stem and display name (voice names are proper nouns, not translated).
const DEMO_VOICES: [(&str, &str, &str); 2] = [("af_heart", "Heart", "us"), ("bm_george", "George", "uk")];

const FAQ_IDS: [&str; 7] = ["free", "voices", "own-voice", "screen-off", "files", "data", "languages"];

pub fn render(ctx: &Ctx) -> Doc {
    let body =
        [hero(ctx), how(ctx), voices(ctx), features(ctx), trust(ctx), platforms(ctx), faq(ctx), parts::cta_band(ctx)]
            .concat();
    Doc {
        title: ctx.cat.text("home-meta-title"),
        description: ctx.cat.text("home-meta-description"),
        body,
        json_ld: vec![parts::software_json_ld(ctx), parts::faq_json_ld(ctx, "faq", &FAQ_IDS)],
        script: true,
    }
}

fn hero(ctx: &Ctx) -> String {
    let os_labels: String = ["macos", "windows", "linux", "android"]
        .iter()
        .map(|os| format!(" data-label-{os}=\"{}\"", ctx.t(&format!("hero-cta-download-{os}"))))
        .collect();
    format!(
        "<section class=\"hero\" aria-labelledby=\"hero-title\">\n<div class=\"hero__glow\" aria-hidden=\"true\"></div>\n\
         <div class=\"wrap hero__inner\">\n<p class=\"hero__eyebrow\">{spark}{eyebrow}</p>\n\
         <h1 class=\"hero__title\" id=\"hero-title\">{title}</h1>\n<p class=\"hero__lede\">{lede}</p>\n\
         <div class=\"hero__ctas\">\n<a class=\"btn btn--primary btn--lg\" href=\"{APP_PATH}\">{open}{arrow}</a>\n\
         <a class=\"btn btn--secondary btn--lg\" href=\"{download}\" data-os-download{os_labels}>{dl_icon}<span data-os-label>{dl}</span>\
         <span class=\"badge badge--inline\">{soon}</span></a>\n</div>\n<p class=\"hero__note\">{note}</p>\n</div>\n\
         <div class=\"wrap\">{demo}</div>\n</section>\n",
        spark = icon("sparkling-line"),
        eyebrow = ctx.t("hero-eyebrow"),
        title = ctx.t("hero-title"),
        lede = ctx.t("hero-lede"),
        open = ctx.t("hero-cta-open"),
        arrow = icon_with_class("arrow-right-line", "ri ri-flip"),
        download = ctx.href(Page::Download),
        dl_icon = icon("download-2-line"),
        dl = ctx.t("hero-cta-download"),
        soon = ctx.t("status-soon"),
        note = ctx.t("hero-note"),
        demo = demo(ctx),
    )
}

fn demo(ctx: &Ctx) -> String {
    let words: String = PASSAGE
        .split(' ')
        .enumerate()
        .map(|(i, w)| {
            let still = if i == STILL_WORD { " is-still" } else { "" };
            format!("<span class=\"w{still}\">{}</span>", crate::html::esc(w))
        })
        .collect::<Vec<_>>()
        .join(" ");
    let chips: String = DEMO_VOICES
        .iter()
        .enumerate()
        .map(|(i, (id, name, accent))| {
            let on = i == 0;
            format!(
                "<button type=\"button\" class=\"chip{sel}\" aria-pressed=\"{on}\" data-voice=\"{id}\">\
                 <span class=\"orb orb--{i}\" aria-hidden=\"true\"></span>{name}<span class=\"chip__meta\">{acc}</span></button>",
                sel = if on { " is-selected" } else { "" },
                acc = ctx.t(&format!("demo-accent-{accent}")),
            )
        })
        .collect();
    format!(
        "<figure class=\"demo\" data-demo data-base=\"/assets/demo/\" data-label-play=\"{play}\" data-label-pause=\"{pause}\">\n\
         <div class=\"demo__bar\"><p class=\"demo__book\">{book_icon}<cite>Pride and Prejudice</cite><span class=\"demo__author\">Jane Austen</span></p>\
         <div class=\"demo__voices\" role=\"group\" aria-label=\"{voice_label}\">{chips}</div></div>\n\
         <blockquote class=\"demo__text\" lang=\"en\" dir=\"ltr\"><span class=\"demo__pill\" aria-hidden=\"true\"></span>\
         <p><span class=\"demo__sentence\">{words}</span></p></blockquote>\n\
         <div class=\"demo__controls\" dir=\"ltr\"><button type=\"button\" class=\"play\" data-demo-play aria-label=\"{play}\" disabled>\
         <span class=\"play__icon play__icon--play\">{play_icon}</span><span class=\"play__icon play__icon--pause\">{pause_icon}</span></button>\
         <div class=\"demo__track\" aria-hidden=\"true\"><span class=\"demo__progress\" data-demo-progress></span></div>\
         <button type=\"button\" class=\"chip chip--speed\" data-demo-speed aria-label=\"{speed_label}\">1×</button></div>\n\
         <figcaption class=\"demo__caption\">{caption}</figcaption>\n</figure>",
        play = ctx.t("demo-play"),
        pause = ctx.t("demo-pause"),
        book_icon = icon("book-open-line"),
        voice_label = ctx.t("demo-voice-label"),
        play_icon = icon("play-fill"),
        pause_icon = icon("pause-fill"),
        speed_label = ctx.t("demo-speed-label"),
        caption = ctx.t("demo-caption"),
    )
}

fn how(ctx: &Ctx) -> String {
    let step = |n: usize, art: String| {
        format!(
            "<li class=\"step\"><div class=\"step__art\" aria-hidden=\"true\">{art}</div><p class=\"step__num\">{n}</p>\
             <h3 class=\"step__title\">{}</h3><p class=\"step__text\">{}</p></li>",
            ctx.t(&format!("how-{n}-title")),
            ctx.t(&format!("how-{n}-text")),
        )
    };
    let art = |name: &str| ctx.site.spot(name, "spot--step");
    format!(
        "<section class=\"section wrap\" id=\"how\" aria-labelledby=\"how-title\">{head}\
         <ol class=\"steps\">{s1}{s2}{s3}</ol></section>\n",
        head = parts::section_head(ctx, "how", "how-eyebrow", "how-title", None),
        s1 = step(1, art("import-link")),
        s2 = step(2, art("empty-voices")),
        s3 = step(3, art("background-play")),
    )
}

fn voices(ctx: &Ctx) -> String {
    let card = |key: &str, spot: &str, featured: bool| {
        format!(
            "<li class=\"tier{f}\">{}<h3 class=\"tier__title\">{}</h3>\
             <p class=\"tier__text\">{}</p><p class=\"tier__meta\">{}</p></li>",
            ctx.site.spot(spot, "spot--tier"),
            ctx.t(&format!("{key}-title")),
            ctx.t(&format!("{key}-text")),
            ctx.t(&format!("{key}-meta")),
            f = if featured { " tier--featured" } else { "" },
        )
    };
    format!(
        "<section class=\"section wrap\" id=\"voices\" aria-labelledby=\"voices-title\">{head}<ul class=\"tiers\">{a}{b}{c}</ul></section>\n",
        head = parts::section_head(ctx, "voices", "voices-eyebrow", "voices-title", Some("voices-lede")),
        a = card("voices-standard", "welcome", false),
        b = card("voices-expressive", "voice-design", false),
        c = card("voices-own", "voice-clone", true),
    )
}

fn features(ctx: &Ctx) -> String {
    let items = [
        ("book-open-line", "feature-highlight"),
        ("speed-up-line", "feature-speed"),
        ("timer-line", "feature-sleep"),
        ("bookmark-line", "feature-resume"),
        ("headphone-line", "feature-screen-off"),
        ("translate-2", "feature-languages"),
    ];
    let lis: String = items
        .iter()
        .map(|(ic, key)| {
            format!(
                "<li class=\"feature\"><span class=\"feature__icon\">{}</span><div><h3 class=\"feature__title\">{}</h3><p class=\"feature__text\">{}</p></div></li>",
                icon(ic),
                ctx.t(&format!("{key}-title")),
                ctx.t(&format!("{key}-text")),
            )
        })
        .collect();
    format!(
        "<section class=\"section wrap\" aria-labelledby=\"features-title\">{}<ul class=\"features\">{lis}</ul></section>\n",
        parts::section_head(ctx, "features", "features-eyebrow", "features-title", None)
    )
}

fn trust(ctx: &Ctx) -> String {
    let col = |spot: &str, key: &str| {
        format!(
            "<li class=\"pillar\">{}<h3 class=\"pillar__title\">{}</h3><p class=\"pillar__text\">{}</p></li>",
            ctx.site.spot(spot, "spot--pillar"),
            ctx.t(&format!("{key}-title")),
            ctx.t(&format!("{key}-text")),
        )
    };
    format!(
        "<section class=\"section section--band\" aria-labelledby=\"trust-title\"><div class=\"wrap\">{}<ul class=\"pillars\">{}{}{}</ul></div></section>\n",
        parts::section_head(ctx, "trust", "trust-eyebrow", "trust-title", None),
        col("invite", "trust-free"),
        col("privacy", "trust-private"),
        col("open-source", "trust-open"),
    )
}

fn platforms(ctx: &Ctx) -> String {
    let cards: String = PLATFORMS.iter().map(|p| parts::platform_card(ctx, p, false)).collect();
    format!(
        "<section class=\"section wrap\" id=\"platforms\" aria-labelledby=\"platforms-title\">{}<ul class=\"platforms\">{cards}</ul>\
         <p class=\"section__more\"><a class=\"link-arrow\" href=\"{}\">{}{}</a></p></section>\n",
        parts::section_head(ctx, "platforms", "platforms-eyebrow", "platforms-title", Some("platforms-lede")),
        ctx.href(Page::Download),
        ctx.t("platforms-more"),
        icon_with_class("arrow-right-line", "ri ri-flip"),
    )
}

fn faq(ctx: &Ctx) -> String {
    format!(
        "<section class=\"section wrap split\" id=\"faq\" aria-labelledby=\"faq-title\">{}<div>{}\
         <p class=\"section__more\"><a class=\"link-arrow\" href=\"{}\">{}{}</a></p></div></section>\n",
        parts::section_head(ctx, "faq", "faq-eyebrow", "faq-title", None),
        parts::faq_list(ctx, "faq", &FAQ_IDS),
        ctx.href(Page::Help),
        ctx.t("faq-more"),
        icon_with_class("arrow-right-line", "ri ri-flip"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passage_matches_every_demo_timing_file() {
        let words: Vec<&str> = PASSAGE.split(' ').collect();
        for raw in [include_str!("../../assets/demo/af_heart.json"), include_str!("../../assets/demo/bm_george.json")] {
            let doc: serde_json::Value = serde_json::from_str(raw).unwrap();
            let timed: Vec<&str> = doc["words"].as_array().unwrap().iter().map(|w| w["w"].as_str().unwrap()).collect();
            assert_eq!(timed, words);
            let mut last = 0.0;
            for w in doc["words"].as_array().unwrap() {
                let (s, e) = (w["s"].as_f64().unwrap(), w["e"].as_f64().unwrap());
                assert!(s >= last && e > s, "timings must increase");
                last = e;
            }
            assert!(last <= doc["duration"].as_f64().unwrap());
        }
        assert!(STILL_WORD < words.len());
    }
}
