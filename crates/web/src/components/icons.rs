//! Line icons in the Lucide style (24 px grid, 1.75 px round strokes), drawn as path data so one component renders
//! them all. Filled variants (play, pause) use the `icon--filled` class.

use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I {
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Back15,
    Forward15,
    Plus,
    X,
    Check,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    Trash,
    Library,
    Voices,
    Settings,
    Mic,
    Stop,
    Link,
    File,
    Text,
    Moon,
    Gauge,
    List,
    More,
    Sparkles,
    ArrowDown,
    Loader,
    Alert,
    LogOut,
    Download,
    Upload,
    Battery,
    Phone,
    Volume,
    Refresh,
    Book,
    Lock,
    Eye,
    EyeOff,
    Storage,
    Headphones,
}

fn paths(icon: I) -> &'static [&'static str] {
    match icon {
        I::Play => &["M6 4.8v14.4a1 1 0 0 0 1.53.85l11.5-7.2a1 1 0 0 0 0-1.7L7.53 3.95A1 1 0 0 0 6 4.8z"],
        I::Pause => &[
            "M7 4h2a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z",
            "M15 4h2a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1h-2a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z",
        ],
        I::SkipBack => &["M19 20 9 12l10-8z", "M5 19V5"],
        I::SkipForward => &["M5 4l10 8-10 8z", "M19 5v14"],
        I::Back15 => &["M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8", "M3 3v5h5"],
        I::Forward15 => &["M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8", "M21 3v5h-5"],
        I::Plus => &["M5 12h14", "M12 5v14"],
        I::X => &["M18 6 6 18", "m6 6 12 12"],
        I::Check => &["M20 6 9 17l-5-5"],
        I::ChevronLeft => &["m15 18-6-6 6-6"],
        I::ChevronRight => &["m9 18 6-6-6-6"],
        I::ChevronDown => &["m6 9 6 6 6-6"],
        I::Trash => &["M3 6h18", "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6", "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"],
        I::Library => &["m16 6 4 14", "M12 6v14", "M8 8v12", "M4 4v16"],
        I::Voices => &["M2 10v3", "M6 6v11", "M10 3v18", "M14 8v7", "M18 5v13", "M22 10v3"],
        I::Settings => {
            &["M20 7h-9", "M14 17H5", "M14 17a3 3 0 1 0 6 0a3 3 0 1 0-6 0", "M4 7a3 3 0 1 0 6 0a3 3 0 1 0-6 0"]
        }
        I::Mic => &["M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3z", "M19 10v2a7 7 0 0 1-14 0v-2", "M12 19v3"],
        I::Stop => &["M7 5h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z"],
        I::Link => &[
            "M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71",
            "M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71",
        ],
        I::File => &[
            "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7z",
            "M14 2v4a2 2 0 0 0 2 2h4",
            "M10 9H8",
            "M16 13H8",
            "M16 17H8",
        ],
        I::Text => &["M4 7V4h16v3", "M9 20h6", "M12 4v16"],
        I::Moon => &["M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9z"],
        I::Gauge => &["m12 14 4-4", "M3.34 19a10 10 0 1 1 17.32 0"],
        I::List => &["M3 6h.01", "M3 12h.01", "M3 18h.01", "M8 6h13", "M8 12h13", "M8 18h13"],
        I::More => &[
            "M11 5a1 1 0 1 0 2 0a1 1 0 1 0-2 0",
            "M11 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0",
            "M11 19a1 1 0 1 0 2 0a1 1 0 1 0-2 0",
        ],
        I::Sparkles => &["M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9z", "M19 16v4", "M17 18h4"],
        I::ArrowDown => &["M12 5v14", "m19 12-7 7-7-7"],
        I::Loader => &["M21 12a9 9 0 1 1-6.22-8.56"],
        I::Alert => &["M2 12a10 10 0 1 0 20 0a10 10 0 1 0-20 0", "M12 8v4", "M12 16h.01"],
        I::LogOut => &["M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4", "m16 17 5-5-5-5", "M21 12H9"],
        I::Download => &["M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", "m7 10 5 5 5-5", "M12 15V3"],
        I::Upload => &["M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", "m17 8-5-5-5 5", "M12 3v12"],
        I::Battery => &["M4 7h12a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2z", "M22 11v2"],
        I::Phone => &["M7 2h10a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z", "M12 18h.01"],
        I::Volume => &["M11 5 6 9H2v6h4l5 4z", "M15.54 8.46a5 5 0 0 1 0 7.07", "M19.07 4.93a10 10 0 0 1 0 14.14"],
        I::Refresh => &[
            "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8",
            "M21 3v5h-5",
            "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16",
            "M8 16H3v5",
        ],
        I::Book => &["M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z", "M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"],
        I::Lock => {
            &["M5 11h14a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2z", "M7 11V7a5 5 0 0 1 10 0v4"]
        }
        I::Eye => &["M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7z", "M9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0"],
        I::EyeOff => &[
            "M9.88 9.88a3 3 0 1 0 4.24 4.24",
            "M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68",
            "M6.61 6.61A13.53 13.53 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61",
            "M2 2l20 20",
        ],
        I::Storage => &[
            "M22 12H2",
            "M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z",
            "M6 16h.01",
            "M10 16h.01",
        ],
        I::Headphones => &[
            "M3 14h3a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a9 9 0 0 1 18 0v7a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3",
        ],
    }
}

fn filled(icon: I) -> bool {
    matches!(icon, I::Play | I::Pause)
}

/// An inline SVG icon, decorative by default (the button around it carries the label).
#[component]
pub fn Icon(icon: I, #[prop(optional)] class: &'static str) -> impl IntoView {
    let class = format!(
        "icon{}{}{}",
        if filled(icon) { " icon--filled" } else { "" },
        if class.is_empty() { "" } else { " " },
        class
    );
    view! {
        <svg class=class viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            {paths(icon).iter().map(|d| view! { <path d=*d /> }).collect_view()}
        </svg>
    }
}

/// A spinning loader.
#[component]
pub fn Spinner(#[prop(optional)] class: &'static str) -> impl IntoView {
    view! { <Icon icon=I::Loader class=if class.is_empty() { "spin" } else { class } /> }
}
