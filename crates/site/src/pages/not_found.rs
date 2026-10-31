//! 404.html: served by the Worker for unknown paths (with status 404).

use crate::config::{APP_PATH, Page};
use crate::icons::{icon, icon_with_class};
use crate::layout::{Ctx, Doc};

pub fn render(ctx: &Ctx) -> Doc {
    let body = format!(
        "<section class=\"lost wrap\" aria-labelledby=\"lost-title\"><p class=\"lost__code\" aria-hidden=\"true\">404</p>\
         <h1 class=\"lost__title\" id=\"lost-title\">{title}</h1><p class=\"lost__lede\">{lede}</p>\
         <div class=\"hero__ctas\"><a class=\"btn btn--primary btn--lg\" href=\"{home}\">{back_icon}{back}</a>\
         <a class=\"btn btn--secondary btn--lg\" href=\"{APP_PATH}\">{open}</a><a class=\"btn btn--ghost btn--lg\" href=\"{help}\">{help_icon}{help_label}</a></div></section>\n",
        title = ctx.t("lost-title"),
        lede = ctx.t("lost-lede"),
        home = ctx.href(Page::Home),
        back_icon = icon_with_class("arrow-left-line", "ri ri-flip"),
        back = ctx.t("lost-home"),
        open = ctx.t("nav-open-app"),
        help = ctx.href(Page::Help),
        help_icon = icon("question-line"),
        help_label = ctx.t("nav-help"),
    );
    Doc {
        title: super::title(ctx, "lost-title"),
        description: ctx.cat.text("lost-lede"),
        body,
        json_ld: vec![],
        script: false,
    }
}
