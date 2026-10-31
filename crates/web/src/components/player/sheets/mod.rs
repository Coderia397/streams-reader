//! Sheets opened from the dock and the reader. Every choice applies at once; voice and sleep changes offer Undo.

mod contents;
mod search;
mod sleep;
mod speed;
mod voice;

pub use contents::ContentsSheet;
pub use search::SearchSheet;
pub use sleep::SleepSheet;
pub use speed::SpeedSheet;
pub use voice::VoiceSheet;
