//! Components reused across pages: section headings, FAQ lists, platform cards, the closing call to action and the
//! structured data (JSON-LD) built from the same strings.

use serde_json::{Value, json};

use crate::config::{APP_PATH, GITHUB_URL, ORIGIN, Page};
use crate::icons::{icon, icon_with_class};
use crate::layout::Ctx;

/// A section heading with an optional eyebrow and lede.
pub fn section_head(ctx: &Ctx, id: &str, eyebrow: &str, title: &str, lede: Option<&str>) -> String {
    let lede = lede.map(|k| format!("<p class=\"section__lede\">{}</p>", ctx.t(k))).unwrap_or_default();
    format!(
        "<div class=\"section__head\"><p class=\"eyebrow\">{}</p><h2 class=\"section__title\" id=\"{id}-title\">{}</h2>{lede}</div>",
        ctx.t(eyebrow),
        ctx.t(title),
    )
}

/// Questions and answers as native disclosure widgets (no script). Keys: `{prefix}-q-{id}` and `{prefix}-a-{id}`.
pub fn faq_list(ctx: &Ctx, prefix: &str, ids: &[&str]) -> String {
    let items: String = ids
        .iter()
        .map(|id| {
            format!(
                "<details class=\"faq__item\"><summary><span>{q}</span>{plus}</summary><div class=\"faq__answer\"><p>{a}</p></div></details>\n",
                q = ctx.t(&format!("{prefix}-q-{id}")),
                a = answer(ctx, &format!("{prefix}-a-{id}")),
                plus = icon("add-line"),
            )
        })
        .collect();
    format!("<div class=\"faq\">\n{items}</div>")
}

/// An answer with the links some answers carry (`{ $app }`, `{ $github }`, `{ $email }`, `{ $download }`).
fn answer(ctx: &Ctx, key: &str) -> String {
    let app = format!("<a href=\"{APP_PATH}\">{}</a>", ctx.t("link-app"));
    let github = format!("<a href=\"{GITHUB_URL}\">GitHub</a>");
    let email = format!("<a href=\"mailto:{0}\">{0}</a>", crate::config::CONTACT_EMAIL);
    let download = format!("<a href=\"{}\">{}</a>", ctx.href(Page::Download), ctx.t("link-download"));
    let privacy = format!("<a href=\"{}\">{}</a>", ctx.href(Page::Privacy), ctx.t("footer-privacy"));
    ctx.cat.t_html(
        key,
        &[("app", &app), ("github", &github), ("email", &email), ("download", &download), ("privacy", &privacy)],
    )
}

/// The same answer as plain text, with the link labels in place of the links.
fn answer_plain(ctx: &Ctx, key: &str) -> String {
    let (app, download, privacy) =
        (ctx.cat.text("link-app"), ctx.cat.text("link-download"), ctx.cat.text("footer-privacy"));
    ctx.cat.text_args(
        key,
        &[
            ("app", &app),
            ("github", "GitHub"),
            ("email", crate::config::CONTACT_EMAIL),
            ("download", &download),
            ("privacy", &privacy),
        ],
    )
}

/// FAQPage structured data from the same questions (plain text, no links).
pub fn faq_json_ld(ctx: &Ctx, prefix: &str, ids: &[&str]) -> Value {
    let entities: Vec<Value> = ids
        .iter()
        .map(|id| {
            let a = answer_plain(ctx, &format!("{prefix}-a-{id}"));
            json!({
                "@type": "Question",
                "name": ctx.cat.text(&format!("{prefix}-q-{id}")),
                "acceptedAnswer": { "@type": "Answer", "text": a },
            })
        })
        .collect();
    json!({ "@context": "https://schema.org", "@type": "FAQPage", "inLanguage": ctx.lang().code, "mainEntity": entities })
}

pub fn software_json_ld(ctx: &Ctx) -> Value {
    json!({
        "@context": "https://schema.org",
        "@type": "SoftwareApplication",
        "name": ctx.cat.text("brand-name"),
        "url": Page::Home.url(&ctx.lang()),
        "description": ctx.cat.text("home-meta-description"),
        "inLanguage": ctx.lang().code,
        "applicationCategory": "MultimediaApplication",
        "operatingSystem": "Web, Android, macOS, Windows, Linux",
        "isAccessibleForFree": true,
        "license": "https://opensource.org/licenses/MIT",
        "codeRepository": GITHUB_URL,
        "image": format!("{ORIGIN}{}", ctx.site.og_image(&ctx.lang())),
        "offers": { "@type": "Offer", "price": "0", "priceCurrency": "USD" },
        "publisher": { "@type": "Organization", "name": "Streams", "url": crate::config::PARENT_URL },
    })
}

/// One platform: icon, translation key stem, link and whether it can be used today.
pub struct Platform {
    pub os: &'static str,
    pub icon: &'static str,
    pub key: &'static str,
    pub available: bool,
}

pub const PLATFORMS: [Platform; 5] = [
    Platform { os: "web", icon: "global-line", key: "platform-web", available: true },
    Platform { os: "android", icon: "android-fill", key: "platform-android", available: false },
    Platform { os: "macos", icon: "apple-fill", key: "platform-macos", available: false },
    Platform { os: "windows", icon: "windows-fill", key: "platform-windows", available: false },
    Platform { os: "linux", icon: "ubuntu-fill", key: "platform-linux", available: false },
];

/// A platform card. `detail` adds the longer note shown on the download page.
pub fn platform_card(ctx: &Ctx, p: &Platform, detail: bool) -> String {
    let status = if p.available {
        format!("<span class=\"badge badge--ok\">{}{}</span>", icon("check-line"), ctx.t("status-available"))
    } else {
        format!("<span class=\"badge\">{}{}</span>", icon("time-line"), ctx.t("status-soon"))
    };
    let note = if detail {
        format!("<p class=\"platform__note\">{}</p>", ctx.t(&format!("{}-note", p.key)))
    } else {
        String::new()
    };
    let action = if p.available {
        format!(
            "<a class=\"btn btn--primary btn--sm\" href=\"{APP_PATH}\">{}{}</a>",
            ctx.t("platform-web-action"),
            icon_with_class("arrow-right-line", "ri ri-flip")
        )
    } else if detail {
        format!(
            "<a class=\"btn btn--secondary btn--sm\" href=\"{GITHUB_URL}/releases\">{}{}</a>",
            ctx.t("platform-watch"),
            icon("github-fill")
        )
    } else {
        String::new()
    };
    format!(
        "<li class=\"platform{avail}\" data-os=\"{os}\"><span class=\"platform__icon\">{ic}</span>\
         <div class=\"platform__body\"><h3 class=\"platform__name\">{name}<span class=\"platform__you\">{you}</span></h3><p class=\"platform__meta\">{meta}</p>{note}</div>\
         <div class=\"platform__side\">{status}{action}</div></li>",
        avail = if p.available { " is-available" } else { "" },
        os = p.os,
        ic = icon(p.icon),
        name = ctx.t(p.key),
        meta = ctx.t(&format!("{}-meta", p.key)),
        you = ctx.t("platform-you"),
    )
}

/// The closing band with the main call to action.
pub fn cta_band(ctx: &Ctx) -> String {
    format!(
        "<section class=\"cta wrap\" aria-labelledby=\"cta-title\"><div class=\"cta__inner\">\
         <h2 class=\"cta__title\" id=\"cta-title\">{}</h2><p class=\"cta__lede\">{}</p>\
         <div class=\"cta__actions\"><a class=\"btn btn--primary btn--lg\" href=\"{APP_PATH}\">{}{}</a>\
         <a class=\"btn btn--ghost btn--lg\" href=\"{GITHUB_URL}\">{}{}</a></div></div></section>\n",
        ctx.t("cta-title"),
        ctx.t("cta-lede"),
        ctx.t("hero-cta-open"),
        icon_with_class("arrow-right-line", "ri ri-flip"),
        icon("github-fill"),
        ctx.t("cta-github"),
    )
}
