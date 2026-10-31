//! One email as subject, HTML and plain text. The layout is a single card: a dark band with the product name as a
//! wordmark (no remote images, so nothing is blocked or tracked), the title, the code in large spaced digits, one
//! button, the fallback link and a footer. Dark-mode colours via `prefers-color-scheme`; Arabic is laid out right to
//! left while the code and the link stay left to right.

use crate::bundle::{is_rtl, pick_language, text};

/// Which auth email this is; each has `<id>-subject`, `-title`, `-body` and (except reauthentication) `-button`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Signup,
    Magiclink,
    Recovery,
    EmailChange,
    Invite,
    Reauthentication,
}

impl Kind {
    pub const ALL: [Kind; 6] =
        [Kind::Signup, Kind::Magiclink, Kind::Recovery, Kind::EmailChange, Kind::Invite, Kind::Reauthentication];

    fn id(self) -> &'static str {
        match self {
            Kind::Signup => "signup",
            Kind::Magiclink => "magiclink",
            Kind::Recovery => "recovery",
            Kind::EmailChange => "email-change",
            Kind::Invite => "invite",
            Kind::Reauthentication => "reauthentication",
        }
    }
}

/// The product's name and address, from one configuration constant (the product will be renamed).
#[derive(Debug, Clone, Copy)]
pub struct Brand<'a> {
    pub name: &'a str,
    /// `https://…` without a trailing slash.
    pub site_url: &'a str,
}

/// Everything one email needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email {
    pub kind: Kind,
    /// Requested language tag; unknown tags fall back to English.
    pub lang: String,
    pub to: String,
    pub code: Option<String>,
    pub link: Option<String>,
    pub new_email: Option<String>,
    /// How long the code works, for the wording.
    pub minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub subject: String,
    pub html: String,
    pub text: String,
}

/// Text made safe inside HTML text and attribute values.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

const FONT: &str = "-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,'Noto Sans','Noto Sans Arabic','Noto Sans Devanagari','PingFang SC','Microsoft YaHei',Helvetica,Arial,sans-serif";
const INK: &str = "#14161a";
const BODY: &str = "#3d4148";
const MUTED: &str = "#6c717a";
const ACCENT: &str = "#4f46e5";

pub fn render(brand: &Brand<'_>, email: &Email) -> Rendered {
    let lang = pick_language(&email.lang);
    let id = email.kind.id();
    let minutes = email.minutes.to_string();
    let site = brand.site_url.trim_start_matches("https://");
    let vars = [
        ("brand", brand.name),
        ("site", site),
        ("minutes", minutes.as_str()),
        ("code", email.code.as_deref().unwrap_or("")),
        ("new_email", email.new_email.as_deref().unwrap_or("")),
    ];
    let t = |key: &str| text(lang, key, &vars);
    let subject = t(&format!("{id}-subject"));
    let title = t(&format!("{id}-title"));
    let body = t(&format!("{id}-body"));
    let greeting = t("greeting");
    let button = email
        .link
        .as_ref()
        .filter(|_| email.kind != Kind::Reauthentication)
        .map(|link| (t(&format!("{id}-button")), link));
    let code_or_link = (email.code.is_some() && button.is_some()).then(|| t("code-or-link"));
    let ignore = t("ignore");
    let footer = t("footer");

    // Plain text part.
    let mut plain = format!("{title}\n\n{greeting}\n\n{body}\n");
    if let Some(code) = &email.code {
        plain.push_str(&format!("\n{}: {code}\n", t("code-label")));
    }
    if let Some(hint) = &code_or_link {
        plain.push_str(&format!("\n{hint}\n"));
    }
    if let Some((label, link)) = &button {
        plain.push_str(&format!("\n{label}: {link}\n"));
    }
    plain.push_str(&format!("\n{ignore}\n\n--\n{footer}\n"));

    // HTML part.
    let dir = if is_rtl(lang) { "rtl" } else { "ltr" };
    let align = if dir == "rtl" { "right" } else { "left" };
    let p = |class: &str, style: &str, content: &str| format!("<p class=\"{class}\" style=\"{style}\">{content}</p>");
    let mut inner = format!(
        "<h1 class=\"ink\" style=\"margin:0 0 16px;font-size:24px;line-height:1.3;font-weight:700;color:{INK};\">{}</h1>",
        escape(&title)
    );
    inner.push_str(&p(
        "body",
        &format!("margin:0 0 8px;font-size:16px;line-height:1.6;color:{BODY};"),
        &escape(&greeting),
    ));
    inner.push_str(&p(
        "body",
        &format!("margin:0 0 24px;font-size:16px;line-height:1.6;color:{BODY};"),
        &escape(&body),
    ));
    if let Some(code) = &email.code {
        inner.push_str(&format!(
            "<div class=\"codebox\" style=\"margin:0 0 24px;padding:18px 20px;border-radius:12px;background:#f1f2f6;text-align:center;\">\
             <div class=\"muted\" style=\"font-size:12px;letter-spacing:1px;text-transform:uppercase;color:{MUTED};margin-bottom:6px;\">{}</div>\
             <div class=\"ink\" dir=\"ltr\" style=\"font-size:34px;line-height:1.2;font-weight:700;letter-spacing:8px;color:{INK};font-family:'SF Mono',Menlo,Consolas,monospace;\">{}</div></div>",
            escape(&t("code-label")),
            escape(code)
        ));
    }
    if let Some(hint) = &code_or_link {
        inner.push_str(&p(
            "muted",
            &format!("margin:0 0 16px;font-size:14px;line-height:1.5;color:{MUTED};"),
            &escape(hint),
        ));
    }
    if let Some((label, link)) = &button {
        let href = escape(link);
        inner.push_str(&format!(
            "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" style=\"margin:0 0 20px;\"><tr><td style=\"background:{ACCENT};border-radius:10px;\">\
             <a href=\"{href}\" style=\"display:inline-block;padding:14px 26px;font-size:16px;font-weight:600;color:#ffffff;text-decoration:none;font-family:{FONT};\">{}</a></td></tr></table>",
            escape(label)
        ));
        inner.push_str(&p(
            "muted",
            &format!("margin:0 0 8px;font-size:13px;line-height:1.5;color:{MUTED};"),
            &format!(
                "{}<br><a href=\"{href}\" dir=\"ltr\" style=\"color:{MUTED};word-break:break-all;\">{href}</a>",
                escape(&t("button-fallback"))
            ),
        ));
    }
    inner.push_str(&p(
        "muted",
        &format!("margin:20px 0 0;font-size:13px;line-height:1.5;color:{MUTED};"),
        &escape(&ignore),
    ));

    let html = format!(
        "<!doctype html><html lang=\"{lang}\" dir=\"{dir}\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"light dark\">\
         <title>{subject_html}</title><style>\
         @media (prefers-color-scheme: dark){{.page{{background:#0e0f12!important}}.card{{background:#191b20!important}}\
         .ink{{color:#f4f4f6!important}}.body{{color:#d5d7dc!important}}.muted{{color:#a2a6ae!important}}\
         .codebox{{background:#23262d!important}}.rule{{border-color:#2a2d34!important}}}}\
         @media only screen and (max-width:600px){{.pad{{padding-left:22px!important;padding-right:22px!important}}}}</style></head>\
         <body class=\"page\" style=\"margin:0;padding:0;background:#f4f4f6;\">\
         <table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" class=\"page\" style=\"background:#f4f4f6;\"><tr><td align=\"center\" style=\"padding:32px 16px;\">\
         <table role=\"presentation\" width=\"560\" cellpadding=\"0\" cellspacing=\"0\" class=\"card\" style=\"width:100%;max-width:560px;background:#ffffff;border-radius:16px;overflow:hidden;\">\
         <tr><td style=\"background:{INK};padding:22px 32px;\" dir=\"ltr\"><a href=\"{site_href}\" style=\"font-family:{FONT};font-size:20px;font-weight:700;letter-spacing:0.5px;color:#ffffff;text-decoration:none;\">{brand_html}</a></td></tr>\
         <tr><td class=\"pad\" dir=\"{dir}\" style=\"padding:32px 32px 24px;font-family:{FONT};text-align:{align};\">{inner}</td></tr>\
         <tr><td class=\"pad muted rule\" dir=\"{dir}\" style=\"padding:18px 32px 26px;font-family:{FONT};font-size:12px;line-height:1.6;color:{MUTED};border-top:1px solid #e6e7eb;text-align:{align};\">{footer_html}</td></tr>\
         </table></td></tr></table></body></html>",
        subject_html = escape(&subject),
        site_href = escape(brand.site_url),
        brand_html = escape(brand.name),
        footer_html = escape(&footer),
    );
    Rendered { subject, html, text: plain }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::LANGS;

    const BRAND: Brand<'static> = Brand { name: "Streams Reader", site_url: "https://listen.streams.watch" };

    fn email(kind: Kind, lang: &str) -> Email {
        Email {
            kind,
            lang: lang.into(),
            to: "a@b.co".into(),
            code: (kind != Kind::Invite).then(|| "482913".into()),
            link: (kind != Kind::Reauthentication)
                .then(|| "https://listen.streams.watch/auth/confirm?token_hash=abc&type=magiclink".into()),
            new_email: (kind == Kind::EmailChange).then(|| "new@b.co".into()),
            minutes: 10,
        }
    }

    /// Every language and every kind: wording present (no message id leaks), variables filled, parts consistent.
    #[test]
    fn every_language_and_kind_renders_completely() {
        for lang in LANGS {
            for kind in Kind::ALL {
                let e = email(kind, lang);
                let r = render(&BRAND, &e);
                let all = format!("{}\n{}\n{}", r.subject, r.html, r.text);
                assert!(!all.contains("{ $"), "{lang} {kind:?}: unfilled variable");
                for leaked in ["-subject", "-title", "-body", "-button", "code-label", "button-fallback"] {
                    assert!(!r.text.contains(leaked), "{lang} {kind:?}: message id {leaked} leaked");
                }
                assert!(r.subject.contains("Streams Reader"), "{lang} {kind:?}: subject names the brand");
                assert!(r.html.contains(&format!("lang=\"{lang}\"")));
                if let Some(code) = &e.code {
                    assert!(r.html.contains(code) && r.text.contains(code), "{lang} {kind:?}: code shown");
                }
                if kind == Kind::Reauthentication {
                    assert!(!r.html.contains("token_hash"), "reauthentication has no button");
                } else {
                    assert!(r.html.contains("token_hash=abc&amp;type=magiclink"), "{lang} {kind:?}: escaped link");
                    assert!(r.text.contains("token_hash=abc&type=magiclink"), "{lang} {kind:?}: raw link in text");
                }
                if kind == Kind::EmailChange {
                    assert!(r.text.contains("new@b.co"));
                }
                if [Kind::Signup, Kind::Magiclink, Kind::Recovery, Kind::Reauthentication].contains(&kind) {
                    assert!(r.text.contains("10"), "{lang} {kind:?}: minutes shown");
                }
            }
        }
    }

    #[test]
    fn english_wording() {
        let r = render(&BRAND, &email(Kind::Magiclink, "en"));
        assert_eq!(r.subject, "482913 is your Streams Reader sign-in code");
        assert!(r.text.starts_with("Your sign-in code\n\nHello,\n\nEnter this code to sign in."));
        assert!(r.text.contains("Sent by Streams Reader · listen.streams.watch"));
    }

    #[test]
    fn arabic_is_right_to_left_with_a_left_to_right_code() {
        let r = render(&BRAND, &email(Kind::Signup, "ar-EG"));
        assert!(r.html.contains("dir=\"rtl\"") && r.html.contains("text-align:right"));
        assert!(r.html.contains("dir=\"ltr\" style=\"font-size:34px"));
    }

    #[test]
    fn values_are_escaped() {
        let mut e = email(Kind::EmailChange, "en");
        e.new_email = Some("<script>@x.co".into());
        let r = render(&BRAND, &e);
        assert!(r.html.contains("&lt;script&gt;@x.co") && !r.html.contains("<script>"));
        assert_eq!(escape("a&\"'<>"), "a&amp;&quot;&#39;&lt;&gt;");
    }

    #[test]
    fn unknown_language_is_english() {
        assert_eq!(render(&BRAND, &email(Kind::Invite, "xx")).subject, "You are invited to Streams Reader");
    }
}
