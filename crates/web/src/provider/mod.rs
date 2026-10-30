//! Audio for the player: the per-document render session (the `nolabs_player::Provider`), its pure bookkeeping, and
//! one-off renders (previews, pre-render on import).

pub mod oneshot;
pub mod session;
pub mod table;

pub use session::RenderSession;
