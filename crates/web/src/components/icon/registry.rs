//! Which Remix icon each [`I`] variant uses. One line per icon: `Variant => "file-name"` (without `.svg`).

macro_rules! icons {
    ($($variant:ident => $file:literal $(mirror $m:tt)?,)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum I { $($variant,)* }

        impl I {
            #[cfg(test)]
            pub const ALL: &'static [(I, &'static str)] = &[$((I::$variant, $file),)*];

            pub fn svg(self) -> &'static str {
                match self {
                    $(I::$variant => include_str!(concat!("../../../assets/icons/remix/", $file, ".svg")),)*
                }
            }

            /// Flipped horizontally in right-to-left layouts.
            pub fn mirrors(self) -> bool {
                match self {
                    $($(I::$variant => { let _ = stringify!($m); true })?)*
                    #[allow(unreachable_patterns)]
                    _ => false,
                }
            }
        }
    };
}

icons! {
    Play => "play-large-fill",
    Pause => "pause-large-fill",
    Replay5 => "replay-5-line",
    Replay10 => "replay-10-line",
    Replay15 => "replay-15-line",
    Replay30 => "replay-30-line",
    Forward5 => "forward-5-line",
    Forward10 => "forward-10-line",
    Forward15 => "forward-15-line",
    Forward30 => "forward-30-line",
    Plus => "add-line",
    Close => "close-line",
    Check => "check-line",
    CheckCircle => "checkbox-circle-fill",
    Back => "arrow-left-s-line" mirror x,
    Forward => "arrow-right-s-line" mirror x,
    ChevronDown => "arrow-down-s-line",
    Trash => "delete-bin-line",
    Library => "book-shelf-line",
    Voices => "user-voice-line",
    Settings => "settings-4-line",
    Mic => "mic-line",
    Stop => "stop-fill",
    Link => "link-m",
    Unlink => "link-unlink",
    File => "file-text-line",
    Pdf => "file-pdf-2-line",
    Book => "book-2-line",
    BookOpen => "book-open-line",
    Article => "article-line",
    Text => "quill-pen-line",
    Upload => "upload-2-line",
    Download => "download-2-line",
    Sleep => "moon-clear-line",
    Timer => "timer-line",
    Speed => "speed-up-line",
    Contents => "play-list-2-line",
    List => "list-unordered",
    More => "more-fill",
    MoreVertical => "more-2-fill",
    Sparkle => "sparkling-2-line",
    Loader => "loader-4-line",
    Alert => "error-warning-line",
    Info => "information-line",
    SignOut => "logout-box-r-line" mirror x,
    SignOutAll => "logout-circle-r-line" mirror x,
    Phone => "smartphone-line",
    Volume => "volume-up-line",
    Refresh => "refresh-line",
    Eye => "eye-line",
    HideText => "eye-close-line",
    Storage => "database-2-line",
    Headphones => "headphone-line",
    Search => "search-line",
    Heart => "heart-3-line",
    HeartFill => "heart-3-fill",
    Mail => "mail-line",
    MailSent => "mail-send-line",
    Google => "google-fill",
    Microsoft => "microsoft-fill",
    Apple => "apple-fill",
    Language => "translate-2",
    Theme => "contrast-2-line",
    Follow => "focus-3-line",
    TextSize => "font-size",
    Account => "account-circle-line",
    Shield => "shield-check-line",
    GitHub => "github-line",
    Equalizer => "equalizer-line",
    Bookmark => "bookmark-line",
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::I;
    use crate::components::icon::path_data;

    #[test]
    fn every_icon_has_one_path() {
        for (icon, file) in I::ALL {
            let svg = icon.svg();
            assert_eq!(svg.matches("<path").count(), 1, "{file}");
            assert!(path_data(svg).starts_with('M'), "{file}");
            assert!(svg.contains("viewBox=\"0 0 24 24\""), "{file}");
        }
    }

    /// Only icons in use are vendored: every SVG in the folder is registered, and each file once.
    #[test]
    fn folder_matches_registry() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons/remix");
        let on_disk: BTreeSet<String> = std::fs::read_dir(dir)
            .map(|d| {
                d.flatten().filter_map(|e| e.file_name().to_str()?.strip_suffix(".svg").map(str::to_owned)).collect()
            })
            .unwrap_or_default();
        let registered: BTreeSet<String> = I::ALL.iter().map(|(_, f)| (*f).to_owned()).collect();
        assert_eq!(registered.len(), I::ALL.len(), "a file is registered twice");
        assert_eq!(on_disk, registered);
    }

    #[test]
    fn directional_icons_mirror() {
        assert!(I::Back.mirrors());
        assert!(!I::Play.mirrors());
    }
}
