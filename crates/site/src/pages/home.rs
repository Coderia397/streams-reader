//! The landing page (note 15, v2): hero with the film and the interactive voice demo, the glow arc, the classics
//! row, four story chapters, voices, more reasons, small details, available everywhere, the promise with the dated
//! comparison, FAQ, the closing call to action.

use crate::config::{APP_PATH, COMPARE_CHECKED, COMPARE_SOURCE_URL, Page, RELEASES_URL};
use crate::data::{DemoText, curl};
use crate::html::esc;
use crate::icons::{icon, icon_with_class};
use crate::layout::{Ctx, Doc};
use crate::pages::parts;

const FAQ_IDS: [&str; 10] =
    ["free", "voices", "own-voice", "screen-off", "files", "data", "languages", "sources", "iphone", "extension"];

/// The second sentence of Pride and Prejudice (public domain), shown after the timed first one in chapter 02.
const PP_SECOND: &str = "However little known the feelings or views of such a man may be on his first entering a \
                         neighbourhood, this truth is so well fixed in the minds of the surrounding families, that he \
                         is considered as the rightful property of some one or other of their daughters.";
/// Waveform bars in the demo player (the clip envelopes have the same count).
pub const BARS: usize = 48;

pub fn render(ctx: &Ctx) -> Doc {
    let body = [
        hero(ctx),
        "<div class=\"arc\" aria-hidden=\"true\"><span></span></div>\n".to_string(),
        classics(ctx),
        chapters(ctx),
        voices(ctx),
        reasons(ctx),
        details(ctx),
        everywhere(ctx),
        promise(ctx),
        faq(ctx),
        parts::cta_band(ctx),
    ]
    .concat();
    let mut json_ld = vec![
        parts::software_json_ld(ctx),
        parts::faq_json_ld(ctx, "faq", &FAQ_IDS),
        parts::organization_json_ld(),
        parts::website_json_ld(ctx),
    ];
    if !ctx.site.library.books.is_empty() {
        json_ld.push(parts::books_json_ld(ctx));
    }
    Doc {
        title: ctx.cat.text("home-meta-title"),
        description: ctx.cat.text("home-meta-description"),
        body,
        json_ld,
        script: true,
        preload: preload(ctx),
    }
}

/// The hero poster is the largest image in the first view: fetch it before the stylesheet asks for it.
fn preload(ctx: &Ctx) -> String {
    let mut out = String::new();
    for (file, media) in
        [("hero-loop-portrait.poster.avif", "(max-width: 699px)"), ("hero-loop.poster.avif", "(min-width: 700px)")]
    {
        if ctx.site.has_media(file) {
            out.push_str(&format!(
                "<link rel=\"preload\" as=\"image\" type=\"image/avif\" href=\"/assets/media/{file}\" media=\"{media}\" fetchpriority=\"high\">\n"
            ));
        }
    }
    out
}

/// "Hear anything you'd *read*." with the accent word in Literata italic and a hand-drawn ember underline.
fn accent_title(ctx: &Ctx, key: &str) -> String {
    let accent = format!(
        "<em class=\"accent\">{}<svg class=\"scribble\" viewBox=\"0 0 120 12\" preserveAspectRatio=\"none\" aria-hidden=\"true\" \
         focusable=\"false\"><path d=\"M3 8.5C28 3.5 70 2.5 117 6.5\" pathLength=\"1\"/></svg></em>",
        ctx.t(&format!("{key}-accent"))
    );
    ctx.cat.t_html(key, &[("accent", &accent)])
}

fn hero(ctx: &Ctx) -> String {
    // iOS gets no app (note 15 Part E): the button becomes "Add to Home Screen" and leads to the install steps.
    let os_labels: String = ["macos", "windows", "linux", "android", "ios"]
        .iter()
        .map(|os| format!(" data-label-{os}=\"{}\"", ctx.t(&format!("hero-cta-download-{os}"))))
        .chain([format!(" data-href-ios=\"{}#install\"", ctx.href(Page::Download))])
        .collect();
    let facts: String =
        [("28", "fact-voices"), ("5", "fact-expressive"), ("MIT", "fact-open"), ("8", "fact-languages")]
            .iter()
            .map(|(n, key)| format!("<li><b>{n}</b><span>{}</span></li>", ctx.t(key)))
            .collect();
    format!(
        "<section class=\"hero\" aria-labelledby=\"hero-title\">\n<div class=\"hero__glow\" aria-hidden=\"true\"><i></i><i></i></div>\n\
         <div class=\"wrap stage\">\n<div class=\"hero__head\">\n<p class=\"hero__eyebrow\">{spark}{eyebrow}</p>\n\
         <h1 class=\"hero__title\" id=\"hero-title\">{title}</h1>\n<p class=\"hero__lede\">{lede}</p>\n\
         <div class=\"actions\">\n<a class=\"btn btn--primary btn--lg\" href=\"{APP_PATH}\">{open}{arrow}</a>\n\
         <a class=\"btn btn--secondary btn--lg\" href=\"#everywhere\" data-os-download{os_labels}>{dl_icon}<span data-os-label>{dl}</span>\
         <span class=\"badge\">{soon}</span></a>\n</div>\n<p class=\"hero__note\">{note}</p>\n</div>\n\
         {demo}\n{film}\n</div>\n<ul class=\"wrap facts\">{facts}</ul>\n</section>\n",
        spark = icon("sparkling-line"),
        eyebrow = ctx.t("hero-eyebrow"),
        title = accent_title(ctx, "hero-title"),
        lede = ctx.t("hero-lede"),
        open = ctx.t("hero-cta-open"),
        arrow = icon_with_class("arrow-right-line", "ri ri-flip"),
        dl_icon = icon("download-2-line"),
        dl = ctx.t("hero-cta-download"),
        soon = ctx.t("status-soon"),
        note = ctx.t("hero-note"),
        film = film(ctx),
        demo = demo(ctx),
    )
}

/// The silent film loop (note 16): poster first, the video is fetched by the script after `load`, only in view,
/// never with reduced motion or Save-Data.
fn film(ctx: &Ctx) -> String {
    let s = ctx.site;
    let m = |f: &str| format!("/assets/media/{f}");
    let source = |file: &str, kind: &str, media: &str| -> String {
        if !s.has_media(file) {
            return String::new();
        }
        let media = if media.is_empty() { String::new() } else { format!(" media=\"{media}\"") };
        format!("<source data-src=\"{}\" type=\"{kind}\"{media}>", m(file))
    };
    let narrow = "(max-width: 699px)";
    let av1 = "video/webm; codecs=&quot;av01.0.05M.08&quot;";
    let h264 = "video/mp4; codecs=&quot;avc1.64001F&quot;";
    let sources = [
        source("hero-loop-portrait.webm", av1, narrow),
        source("hero-loop-portrait.mp4", h264, narrow),
        source("hero-loop.webm", av1, ""),
        source("hero-loop.mp4", h264, ""),
    ]
    .concat();
    let poster_src = |f: &str, media: &str, avif: bool| -> String {
        if !s.has_media(f) {
            return String::new();
        }
        let media = if media.is_empty() { String::new() } else { format!(" media=\"{media}\"") };
        let ty = if avif { " type=\"image/avif\"" } else { "" };
        format!("<source{media}{ty} srcset=\"{}\">", m(f))
    };
    let fallback = if s.has_media("hero-loop.poster.jpg") { m("hero-loop.poster.jpg") } else { String::new() };
    format!(
        "<figure class=\"film\" data-film data-label-play=\"{play}\" data-label-pause=\"{pause}\">\
         <div class=\"film__frame\"><picture>{p1}{p2}{p3}<img class=\"film__poster\" src=\"{fallback}\" width=\"1280\" height=\"720\" \
         alt=\"\" fetchpriority=\"high\" decoding=\"async\"></picture>\
         <video class=\"film__video\" muted playsinline loop preload=\"none\" disablepictureinpicture aria-hidden=\"true\" tabindex=\"-1\">{sources}</video>\
         <button type=\"button\" class=\"btn film__toggle\" data-film-toggle aria-label=\"{play}\" hidden>\
         <span class=\"play__icon play__icon--play\">{play_icon}</span><span class=\"play__icon play__icon--pause\">{pause_icon}</span></button>\
         <span class=\"film__line\" aria-hidden=\"true\"><span data-film-progress></span></span></div>\
         <figcaption class=\"sr-only\">{caption}</figcaption></figure>",
        play = ctx.t("film-play"),
        pause = ctx.t("film-pause"),
        p1 = poster_src("hero-loop-portrait.poster.avif", narrow, true),
        p2 = poster_src("hero-loop-portrait.poster.jpg", narrow, false),
        p3 = poster_src("hero-loop.poster.avif", "", true),
        play_icon = icon("play-fill"),
        pause_icon = icon("pause-fill"),
        caption = ctx.t("film-label"),
    )
}

/// The ElevenLabs-style demo: sample tabs, voice chips, play, waveform and words lighting up in time.
fn demo(ctx: &Ctx) -> String {
    let d = &ctx.site.demo;
    let Some(first) = d.texts.first() else { return String::new() };
    let tab_icon = |id: &str| match id {
        "story" => "book-open-line",
        "article" => "article-line",
        "poem" => "quill-pen-line",
        _ => "moon-line",
    };
    let tabs: String = d
        .texts
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let on = i == 0;
            format!(
                "<button type=\"button\" role=\"tab\" class=\"tab\" id=\"demo-t-{id}\" aria-controls=\"demo-p-{id}\" aria-selected=\"{on}\" \
                 tabindex=\"{tab}\" data-text=\"{id}\" data-clips=\"{clips}\">{ic}{label}</button>",
                id = t.id,
                tab = if on { 0 } else { -1 },
                clips = t.clips.join(" "),
                ic = icon(tab_icon(&t.id)),
                label = ctx.t(&format!("demo-tab-{}", t.id)),
            )
        })
        .collect();
    let panels: String = d
        .texts
        .iter()
        .enumerate()
        .map(|(i, t)| {
            format!(
                "<div class=\"demo__panel\" role=\"tabpanel\" id=\"demo-p-{id}\" aria-labelledby=\"demo-t-{id}\" data-panel=\"{id}\"{hidden}>\
                 <blockquote class=\"demo__text{poem}\" lang=\"en\" dir=\"ltr\"><p>{html}</p></blockquote>\
                 <p class=\"demo__source\" lang=\"en\" dir=\"ltr\">{source}</p></div>",
                id = t.id,
                hidden = if i == 0 { "" } else { " hidden" },
                poem = if t.text.contains('\n') { " demo__text--poem" } else { "" },
                html = t.html(),
                source = esc(&curl(&t.source)),
            )
        })
        .collect();
    let voices: String = d
        .voices
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let on = i == 0;
            let meta = if v.tier == "expressive" {
                format!("<span class=\"voice__tier\">{}{}</span>", icon("sparkling-line"), ctx.t("demo-tier-expressive"))
            } else {
                format!("<span class=\"voice__meta\">{}</span>", ctx.t(&format!("demo-accent-{}", v.accent)))
            };
            let locked = if v.available { "" } else { " aria-disabled=\"true\" data-locked" };
            format!(
                "<button type=\"button\" role=\"radio\" class=\"voice\" aria-checked=\"{on}\" tabindex=\"{tab}\" data-voice=\"{id}\" \
                 data-tier=\"{tier}\"{locked}><span class=\"orb orb--{i}\" aria-hidden=\"true\"></span><span class=\"voice__name\">{name}</span>{meta}</button>",
                tab = if on { 0 } else { -1 },
                id = v.id,
                tier = v.tier,
                name = esc(&v.name),
            )
        })
        .collect();
    let bars = waveform(first);
    format!(
        "<section class=\"demo\" id=\"try\" aria-labelledby=\"demo-title\" data-demo data-base=\"/assets/demo/\" \
         data-label-play=\"{play}\" data-label-pause=\"{pause}\" tabindex=\"-1\">\
         <div class=\"demo__top\"><h2 class=\"demo__title\" id=\"demo-title\">{wave_icon}{title}</h2>\
         <div class=\"tabs\" role=\"tablist\" aria-label=\"{tabs_label}\">{tabs}</div></div>\
         <div class=\"demo__body\"><span class=\"demo__pill\" aria-hidden=\"true\"></span>{panels}</div>\
         <div class=\"demo__foot\"><div class=\"voices\" role=\"radiogroup\" aria-label=\"{voice_label}\">{voices}</div>\
         <div class=\"player\" dir=\"ltr\"><button type=\"button\" class=\"btn play\" data-demo-play aria-label=\"{play}\" aria-pressed=\"false\" disabled>\
         <span class=\"play__icon play__icon--play\">{play_icon}</span><span class=\"play__icon play__icon--pause\">{pause_icon}</span>\
         <span class=\"play__spin\" aria-hidden=\"true\"></span></button>\
         <div class=\"wave\" aria-hidden=\"true\" data-wave>{bars}<b class=\"wave__head\"></b></div>\
         <span class=\"player__time\" data-demo-time>{time}</span>\
         <button type=\"button\" class=\"tab chip--speed\" data-demo-speed aria-label=\"{speed_label}\">1×</button></div></div>\
         <div class=\"demo__note\" aria-hidden=\"true\"><p data-note=\"caption\">{caption}</p><p class=\"off\" data-note=\"switch\">{switch}</p>\
         <p class=\"off is-alert\" data-note=\"locked\">{locked}</p><p class=\"off is-alert\" data-note=\"error\">{error}</p></div>\
         <p class=\"sr-only\" data-demo-status aria-live=\"polite\">{caption}</p></section>",
        play = ctx.t("demo-play"),
        pause = ctx.t("demo-pause"),
        switch = ctx.t("demo-expressive-switch"),
        locked = ctx.t("demo-expressive-locked"),
        error = ctx.t("demo-error"),
        wave_icon = icon("voiceprint-line"),
        title = ctx.t("demo-title"),
        tabs_label = ctx.t("demo-tabs-label"),
        voice_label = ctx.t("demo-voice-label"),
        play_icon = icon("play-fill"),
        pause_icon = icon("pause-fill"),
        time = clock(first.duration),
        speed_label = ctx.t("demo-speed-label"),
        caption = ctx.t("demo-caption"),
    )
}

fn waveform(t: &DemoText) -> String {
    (0..BARS).map(|i| format!("<i class=\"l{}\"></i>", t.levels.get(i).copied().unwrap_or(3).clamp(1, 10))).collect()
}

/// `m:ss` from milliseconds.
pub fn clock(ms: u32) -> String {
    let s = ms.div_ceil(1000);
    format!("{}:{:02}", s / 60, s % 60)
}

/// The section header pattern: a round chip with an icon, a grey eyebrow, then the statement.
fn head(ctx: &Ctx, id: &str, ic: &str, eyebrow: &str, title: &str, lede: Option<&str>) -> String {
    let lede = lede.map(|k| format!("<p class=\"section__lede\">{}</p>", ctx.t(k))).unwrap_or_default();
    format!(
        "<div class=\"section__head reveal\"><p class=\"eyebrow\"><span class=\"eyebrow__chip\">{}</span>{}</p>\
         <h2 class=\"section__title\" id=\"{id}-title\">{}</h2>{lede}</div>",
        icon(ic),
        ctx.t(eyebrow),
        ctx.t(title),
    )
}

fn classics(ctx: &Ctx) -> String {
    let books = &ctx.site.library.books;
    if books.is_empty() {
        return String::new();
    }
    let item = |i: usize, b: &crate::data::Book| -> String {
        let wide = if i >= 9 { " book--wide" } else { "" };
        let cover =
            |fmt: &str| format!("/assets/covers/{}-200.{fmt} 200w, /assets/covers/{}-300.{fmt} 300w", b.slug, b.slug);
        let pic = format!(
            "<picture><source type=\"image/avif\" srcset=\"{a}\" sizes=\"(min-width: 900px) 132px, 104px\">\
             <img class=\"book__cover\" src=\"/assets/covers/{slug}-200.webp\" srcset=\"{w}\" sizes=\"(min-width: 900px) 132px, 104px\" \
             width=\"200\" height=\"300\" alt=\"\" loading=\"lazy\" decoding=\"async\"></picture>",
            a = cover("avif"),
            w = cover("webp"),
            slug = b.slug,
        );
        let label = ctx.cat.t_args("classics-play", &[("title", &b.title), ("author", &b.author)]);
        let inner = format!(
            "<span class=\"book__rank\" aria-hidden=\"true\">{n}</span><span class=\"book__art\">{pic}<span class=\"book__play\" aria-hidden=\"true\">\
             {play}{pause}</span></span>",
            n = i + 1,
            play = icon_with_class("play-fill", "ri book__i-play"),
            pause = icon_with_class("pause-fill", "ri book__i-pause"),
        );
        if b.preview_voice.is_some() {
            format!(
                "<li class=\"book{wide}\"><button type=\"button\" class=\"book__btn\" data-preview=\"/assets/previews/{}.webm\" aria-label=\"{label}\" \
                 aria-pressed=\"false\">{inner}</button></li>",
                b.slug
            )
        } else {
            format!(
                "<li class=\"book{wide}\"><div class=\"book__btn\" role=\"img\" aria-label=\"{}\">{inner}</div></li>",
                esc(&format!("{}, {}", b.title, b.author))
            )
        }
    };
    let list: String = books.iter().enumerate().map(|(i, b)| item(i, b)).collect();
    format!(
        "<section class=\"section classics\" id=\"library\" aria-labelledby=\"classics-title\">\
         <div class=\"wrap\">{head}</div>\
         <div class=\"marquee\" data-marquee><div class=\"marquee__viewport\" data-marquee-viewport tabindex=\"-1\"><div class=\"marquee__rail\">\
         <ol class=\"marquee__list\">{list}</ol><ol class=\"marquee__list marquee__list--copy\" aria-hidden=\"true\" inert>{list}</ol></div></div>\
         <button type=\"button\" class=\"btn marquee__arrow marquee__arrow--prev\" data-marquee-step=\"-1\" aria-label=\"{prev}\">{prev_icon}</button>\
         <button type=\"button\" class=\"btn marquee__arrow marquee__arrow--next\" data-marquee-step=\"1\" aria-label=\"{next}\">{next_icon}</button></div>\
         <p class=\"wrap classics__licence\">{licence} <a href=\"{credits}#library\">{about}</a></p></section>\n",
        head = head(ctx, "classics", "book-shelf-line", "classics-eyebrow", "classics-title", Some("classics-lede")),
        prev = ctx.t("classics-prev"),
        next = ctx.t("classics-next"),
        prev_icon = icon_with_class("arrow-left-s-line", "ri ri-flip"),
        next_icon = icon_with_class("arrow-right-s-line", "ri ri-flip"),
        licence = ctx.t("classics-licence"),
        credits = ctx.href(Page::Credits),
        about = ctx.t("classics-credits"),
    )
}

fn chapters(ctx: &Ctx) -> String {
    let chip = |ic: &str, key: &str| format!("<li class=\"tag\">{}{}</li>", icon(ic), ctx.t(key));
    let chapter = |n: usize, title: String, chips: String, media: String, extra: String| {
        format!(
            "<article class=\"chapter\" aria-labelledby=\"ch{n}-title\"><div class=\"chapter__media\" aria-hidden=\"true\">{media}</div>\
             <div class=\"chapter__text reveal\"><p class=\"chapter__num\">{eyebrow}</p><h2 class=\"chapter__title\" id=\"ch{n}-title\">{title}</h2>\
             <p class=\"chapter__body\">{body}</p><ul class=\"tags\">{chips}</ul>{extra}</div></article>\n",
            eyebrow = ctx.t(&format!("ch{n}-eyebrow")),
            body = ctx.t(&format!("ch{n}-text")),
        )
    };
    let story = ctx.site.demo.texts.iter().find(|t| t.id == "story");
    let article = ctx.site.demo.texts.iter().find(|t| t.id == "article");
    let article_text = article.map(|t| esc(&t.text)).unwrap_or_default();
    // 01: a cluttered page collapses into clean text.
    let m1 = format!(
        "<div class=\"mock mock--clean\" data-inview><div class=\"mock__bar\"><i></i><i></i><i></i><span>example.com/why-is-the-sky-blue</span></div>\
         <div class=\"clean\"><span class=\"junk junk--nav\"></span><span class=\"junk junk--ad\"></span>\
         <span class=\"junk junk--pop\"></span><div class=\"clean__text\" lang=\"en\" dir=\"ltr\"><p class=\"clean__h\">Why is the sky blue?</p>\
         <p>{article_text}</p></div><span class=\"clean__listen\">{play}</span></div></div>",
        play = icon("headphone-line"),
    );
    // 02: the reader follows along, silently, with the real timings of the story clip.
    let follow = story
        .and_then(|t| t.clips.first().map(|c| format!(" data-follow=\"/assets/demo/{}-{c}.json\"", t.id)))
        .unwrap_or_default();
    let m2 = format!(
        "<div class=\"mock mock--reader\" data-inview{follow}>\
         <div class=\"reader\" lang=\"en\" dir=\"ltr\"><p class=\"reader__chapter\">Chapter 1</p><p class=\"reader__text\"><span class=\"demo__pill\"></span>\
         <span class=\"reader__sentence\">{sentence}</span> {second}</p></div>\
</div>",
        sentence = story.map(DemoText::html).unwrap_or_default(),
        second = esc(PP_SECOND),
    );
    // 03: the recording screen with a level meter that moves only while in view.
    let meter: String = (0..24).map(|_| "<i></i>").collect();
    let m3 = format!(
        "<div class=\"mock mock--rec\" data-inview><p class=\"rec__step\">{step}</p>\
         <p class=\"rec__line\" lang=\"en\" dir=\"ltr\">“The morning light fell softly across the open pages.”</p>\
         <p class=\"meter\">{meter}</p><span class=\"rec__btn\">{rec}</span><p class=\"rec__hint\">{hint}</p></div>",
        step = ctx.t("ch3-mock-step"),
        rec = icon("record-circle-fill"),
        hint = ctx.t("ch3-mock-hint"),
    );
    // 04: the lock screen keeps playing (the same phone as the Android tile, scaled by its container).
    let m4 = format!("<div class=\"mock mock--lock\" data-inview>{}</div>", mini_phone());
    let try_link = format!(
        "<p class=\"chapter__more\"><a class=\"link-arrow\" href=\"#try\" data-try=\"story\">{}{}</a></p>",
        ctx.t("ch2-try"),
        icon("volume-up-line")
    );
    format!(
        "<section class=\"section chapters\" id=\"how\" aria-label=\"{label}\"><div class=\"wrap\">{c1}{c2}{c3}{c4}</div></section>\n",
        label = ctx.t("nav-how"),
        c1 = chapter(
            1,
            ctx.t("ch1-title"),
            [
                chip("links-line", "ch1-chip-links"),
                chip("file-pdf-2-line", "ch1-chip-pdf"),
                chip("book-open-line", "ch1-chip-epub"),
                chip("file-text-line", "ch1-chip-text")
            ]
            .concat(),
            m1,
            String::new()
        ),
        c2 = chapter(
            2,
            ctx.t("ch2-title"),
            [
                chip("voiceprint-line", "ch2-chip-word"),
                chip("eye-off-line", "ch2-chip-follow"),
                chip("speed-up-line", "ch2-chip-speed")
            ]
            .concat(),
            m2,
            try_link
        ),
        c3 = chapter(
            3,
            accent_title(ctx, "ch3-title"),
            [
                chip("mic-line", "ch3-chip-lines"),
                chip("volume-up-line", "ch3-chip-level"),
                chip("lock-line", "ch3-chip-private")
            ]
            .concat(),
            m3,
            String::new()
        ),
        c4 = chapter(
            4,
            ctx.t("ch4-title"),
            [
                chip("lock-line", "ch4-chip-lock"),
                chip("bookmark-line", "ch4-chip-resume"),
                chip("timer-line", "ch4-chip-sleep")
            ]
            .concat(),
            m4,
            String::new()
        ),
    )
}

fn voices(ctx: &Ctx) -> String {
    let d = &ctx.site.demo;
    let card = |i: usize, v: &crate::data::DemoVoice| -> String {
        let text = if v.tier == "expressive" { "night" } else { "story" };
        let has = d.texts.iter().any(|t| t.id == text && t.clips.iter().any(|c| c == &v.id));
        let tier = if v.tier == "expressive" {
            format!(
                "<span class=\"vcard__tag vcard__tag--x\">{}{}</span>",
                icon("sparkling-line"),
                ctx.t("voice-tier-expressive")
            )
        } else {
            format!(
                "<span class=\"vcard__tag\">{} · {}</span>",
                ctx.t("voice-tier-standard"),
                ctx.t(&format!("demo-accent-{}", v.accent))
            )
        };
        let orb = if has {
            format!(
                "<button type=\"button\" class=\"vcard__orb orb orb--{i}\" data-preview=\"/assets/demo/{text}-{id}.webm\" aria-pressed=\"false\" \
                 aria-label=\"{label}\">{play}{pause}</button>",
                id = v.id,
                label = ctx.cat.t_args("voice-preview", &[("name", &v.name)]),
                play = icon_with_class("play-fill", "ri book__i-play"),
                pause = icon_with_class("pause-fill", "ri book__i-pause"),
            )
        } else {
            format!("<span class=\"vcard__orb orb orb--{i}\" aria-hidden=\"true\"></span>")
        };
        format!(
            "<li class=\"vcard\">{orb}<div><h3 class=\"vcard__name\">{name}</h3><p class=\"vcard__desc\">{desc}</p></div>{tier}</li>",
            name = esc(&v.name),
            desc = ctx.t(&format!("voice-desc-{}", v.id)),
        )
    };
    let cards: String = d.voices.iter().enumerate().map(|(i, v)| card(i, v)).collect();
    let own = format!(
        "<li class=\"vcard vcard--own\"><span class=\"vcard__orb orb orb--you\" aria-hidden=\"true\">{mic}</span><div><h3 class=\"vcard__name\">{name}</h3>\
         <p class=\"vcard__desc\">{desc}</p></div><a class=\"vcard__tag vcard__tag--link\" href=\"{APP_PATH}\">{action}{arrow}</a></li>",
        mic = icon("mic-line"),
        name = ctx.t("voice-own-name"),
        desc = ctx.t("voice-own-desc"),
        action = ctx.t("voice-own-action"),
        arrow = icon_with_class("arrow-right-line", "ri ri-flip"),
    );
    let tier = |key: &str, n: &str| {
        format!(
            "<li class=\"tier\"><p class=\"tier__n\">{n}</p><div><h3 class=\"tier__title\">{}</h3><p class=\"tier__text\">{}</p>\
             <p class=\"tier__meta\">{}{}</p></div></li>",
            ctx.t(&format!("{key}-title")),
            ctx.t(&format!("{key}-text")),
            icon("check-line"),
            ctx.t(&format!("{key}-meta")),
        )
    };
    format!(
        "<section class=\"section wrap\" id=\"voices\" aria-labelledby=\"voices-title\">{head}<ul class=\"vgrid stagger\">{cards}{own}</ul>\
         <ul class=\"tiers\">{a}{b}{c}</ul></section>\n",
        head = head(ctx, "voices", "user-voice-line", "voices-eyebrow", "voices-title", Some("voices-lede")),
        a = tier("voices-standard", "28"),
        b = tier("voices-expressive", "5"),
        c = tier("voices-own", "1"),
    )
}

/// Four gradient cards, each with a large Remix glyph in a soft disc in the bottom corner (D37: no spots here). Below
/// 600 px the list is a scroll row, so it is a named tab stop (Safari does not focus scrollers on its own); the script
/// drops the stop where the row does not scroll.
fn reasons(ctx: &Ctx) -> String {
    let items = [
        ("open", "code-s-slash-line"),
        ("classics", "book-shelf-line"),
        ("offline", "wifi-off-line"),
        ("devices", "smartphone-line"),
    ];
    let cards: String = items
        .iter()
        .map(|(k, ic)| {
            format!(
                "<li class=\"reason\"><h3 class=\"reason__title\">{}</h3><p class=\"reason__text\">{}</p>\
                 <span class=\"reason__icon\" aria-hidden=\"true\">{}</span></li>",
                ctx.t(&format!("reason-{k}-title")),
                ctx.t(&format!("reason-{k}-text")),
                icon(ic),
            )
        })
        .collect();
    format!(
        "<section class=\"section wrap reasons-section\" aria-labelledby=\"reasons-title\">{}<ul class=\"reasons stagger\" tabindex=\"0\" aria-labelledby=\"reasons-title\" data-scroller>{cards}</ul></section>\n",
        head(ctx, "reasons", "star-line", "reasons-eyebrow", "reasons-title", None)
    )
}

fn details(ctx: &Ctx) -> String {
    // Only what the chapters don't already show (their chips cover highlight, speed, sleep, resume, screen off, files).
    let items = [
        ("keyboard-line", "feature-keyboard"),
        ("translate-2", "feature-languages"),
        ("moon-line", "feature-theme"),
        ("download-2-line", "feature-install"),
    ];
    let lis: String = items
        .iter()
        .map(|(ic, key)| {
            format!(
                "<li class=\"feature\"><span class=\"feature__icon\">{}</span><h3 class=\"feature__title\">{}</h3><p class=\"feature__text\">{}</p></li>",
                icon(ic),
                ctx.t(&format!("{key}-title")),
                ctx.t(&format!("{key}-text")),
            )
        })
        .collect();
    format!(
        "<section class=\"section wrap\" aria-labelledby=\"features-title\">{}<ul class=\"features stagger\">{lis}</ul></section>\n",
        head(ctx, "features", "sparkling-line", "features-eyebrow", "features-title", None)
    )
}

fn everywhere(ctx: &Ctx) -> String {
    let soon = format!("<span class=\"badge\">{}{}</span>", icon("time-line"), ctx.t("status-soon"));
    let ok = format!("<span class=\"badge badge--ok\">{}{}</span>", icon("check-line"), ctx.t("status-available"));
    let os_labels: String = ["macos", "windows", "linux"]
        .iter()
        .map(|os| format!(" data-label-{os}=\"{}\"", ctx.t(&format!("hero-cta-download-{os}"))))
        .collect();
    let download = ctx.href(Page::Download);
    let tile = |os: &str, ic: &str, ui: String, state: &str, actions: String| {
        format!(
            "<li class=\"tile\" data-os=\"{os}\"><div class=\"tile__ui\" aria-hidden=\"true\">{ui}</div><div class=\"tile__body\">\
             <h3 class=\"tile__name\">{ic}<span>{name} <span class=\"tile__you\">{you}</span></span></h3><p class=\"tile__meta\">{meta}</p>\
             <div class=\"tile__actions\">{state}{actions}</div></div></li>",
            ic = icon(ic),
            name = ctx.t(&format!("tile-{os}")),
            you = ctx.t("platform-you"),
            meta = ctx.t(&format!("tile-{os}-meta")),
        )
    };
    let disabled = |label: String, extra: &str| {
        format!(
            "<span class=\"btn btn--secondary btn--sm\" aria-disabled=\"true\"{extra}><span data-os-label>{label}</span></span>"
        )
    };
    let web_ui = "<div class=\"mini mini--web\"><p class=\"mini__dots\"><i></i><i></i><i></i></p><p class=\"mini__page\" lang=\"en\">It is a truth \
                  <mark>universally</mark> acknowledged, that a single man in possession of a good fortune…</p></div>"
        .to_string();
    let android_ui = mini_phone();
    let desktop_ui = format!(
        "<div class=\"mini mini--desk\"><p class=\"mini__dots\"><i></i><i></i><i></i></p><div class=\"mini__panel\" dir=\"ltr\"><span class=\"mini__play\">{}</span>\
         <span class=\"mini__wave\">{}</span><span class=\"mini__rate\">1.25× · 190 wpm</span></div></div>",
        icon("play-fill"),
        (0..14).map(|_| "<i></i>").collect::<String>()
    );
    let ext_ui = format!(
        "<div class=\"mini mini--ext\"><p class=\"mini__toolbar\"><span class=\"mini__url\"></span><span class=\"mini__btn\">{mark}</span></p>\
         <p class=\"mini__toast\">{wave}{reading}</p><span class=\"mini__lines mini__lines--page\"><i></i><i></i><i></i></span></div>",
        mark = icon("volume-up-line"),
        wave = icon("voiceprint-line"),
        reading = ctx.t("tile-mock-reading"),
    );
    let tiles = [
        tile(
            "web",
            "global-line",
            web_ui,
            &ok,
            format!(
                "<a class=\"btn btn--primary btn--sm\" href=\"{APP_PATH}\">{}{}</a>",
                ctx.t("tile-web-action"),
                icon_with_class("arrow-right-line", "ri ri-flip")
            ),
        ),
        tile(
            "android",
            "android-fill",
            android_ui,
            &soon,
            format!(
                "{}<a class=\"tile__link\" href=\"{download}#install\">{}</a>",
                disabled(ctx.t("tile-android-action"), ""),
                ctx.t("tile-android-alt")
            ),
        ),
        tile(
            "desktop",
            "macbook-line",
            desktop_ui,
            &soon,
            format!(
                "{}<a class=\"tile__link\" href=\"{RELEASES_URL}\">{}</a>",
                disabled(ctx.t("tile-desktop-action"), &format!(" data-os-download{os_labels}")),
                ctx.t("tile-desktop-alt")
            ),
        ),
        tile("extension", "chrome-line", ext_ui, &soon, disabled(ctx.t("tile-extension-action"), "")),
    ]
    .concat();
    format!(
        "<section class=\"section wrap\" id=\"everywhere\" aria-labelledby=\"everywhere-title\">{head}<ul class=\"tiles stagger\" data-tiles>{tiles}</ul>\
         <p class=\"tiles__ios\">{apple}{ios} <a href=\"{download}\">{all}</a></p></section>\n",
        head = head(ctx, "everywhere", "smartphone-line", "everywhere-eyebrow", "everywhere-title", None),
        apple = icon("apple-fill"),
        ios = ctx.t("everywhere-ios"),
        all = ctx.t("footer-all-downloads"),
    )
}

/// A phone lock screen with the now-playing card (the Android tile and chapter 04).
fn mini_phone() -> String {
    format!(
        "<div class=\"mini mini--phone\"><p class=\"mini__clock\">22:41</p><div class=\"mini__player\"><span class=\"mini__cover\"></span>\
         <span class=\"mini__lines\"><i></i><i></i></span><span class=\"mini__bar\" dir=\"ltr\"><span></span></span><span class=\"mini__ctl\" dir=\"ltr\">{}{}{}</span></div></div>",
        icon("skip-back-fill"),
        icon("pause-fill"),
        icon("skip-forward-fill")
    )
}

fn promise(ctx: &Ctx) -> String {
    let col = |ic: &str, key: &str| {
        format!(
            "<li class=\"pillar\"><span class=\"eyebrow__chip\" aria-hidden=\"true\">{}</span><h3 class=\"pillar__title\">{}</h3>\
             <p class=\"pillar__text\">{}</p></li>",
            icon(ic),
            ctx.t(&format!("{key}-title")),
            ctx.t(&format!("{key}-text")),
        )
    };
    let rows: String = ["price", "hours", "voice", "apps", "library"]
        .iter()
        .map(|r| {
            format!(
                "<tr><th scope=\"row\">{}</th><td>{}</td><td>{}</td></tr>",
                ctx.t(&format!("compare-{r}")),
                ctx.t(&format!("compare-{r}-us")),
                ctx.t(&format!("compare-{r}-them")),
            )
        })
        .collect();
    let source = format!("<a href=\"{COMPARE_SOURCE_URL}\">{}</a>", ctx.t("compare-source"));
    let date = format!("<time datetime=\"{COMPARE_CHECKED}\">{}</time>", ctx.t("compare-date"));
    format!(
        "<section class=\"section section--band\" id=\"promise\" aria-labelledby=\"trust-title\"><div class=\"wrap\">{head}\
         <ul class=\"pillars stagger\">{a}{b}{c}</ul><p class=\"promise__note reveal\">{note}</p>\
         <div class=\"compare reveal\"><h3 class=\"compare__title\">{ctitle}</h3><div class=\"compare__scroll\" tabindex=\"0\" role=\"region\" aria-labelledby=\"compare-cap\">\
         <table class=\"compare__table\"><caption id=\"compare-cap\" class=\"sr-only\">{ctitle}</caption><thead><tr><td></td>\
         <th scope=\"col\" class=\"compare__us\">Streams Reader</th><th scope=\"col\">ElevenReader</th></tr></thead><tbody>{rows}</tbody></table></div>\
         <p class=\"compare__asof\">{asof}</p></div></div></section>\n",
        head = head(ctx, "trust", "gift-line", "trust-eyebrow", "trust-title", None),
        a = col("check-line", "trust-free"),
        b = col("shield-check-line", "trust-private"),
        c = col("code-s-slash-line", "trust-open"),
        note = ctx.t("trust-note"),
        ctitle = ctx.t("compare-title"),
        asof = ctx.cat.t_html("compare-asof", &[("date", &date), ("source", &source)]),
    )
}

fn faq(ctx: &Ctx) -> String {
    format!(
        "<section class=\"section wrap faq-section\" id=\"faq\" aria-labelledby=\"faq-title\">{}<div class=\"reveal\">{}\
         <p class=\"section__more\"><a class=\"link-arrow\" href=\"{}\">{}{}</a></p></div></section>\n",
        head(ctx, "faq", "question-line", "faq-eyebrow", "faq-title", None),
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
    fn clock_rounds_up_to_whole_seconds() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(8_200), "0:09");
        assert_eq!(clock(61_000), "1:01");
    }
}
