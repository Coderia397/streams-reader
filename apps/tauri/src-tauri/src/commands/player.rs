//! Player commands. The UI sends intent; the native engine owns the playlist, buffering, rate, sleep timer and the
//! OS media session, and reports back through `streams:player` events (`PlayerEvent`).
//!
//! | Command | Effect |
//! |---|---|
//! | `player_load { args }` | open a document: feeder starts at `args.start_segment`, engine plays when audio lands |
//! | `player_control { command }` | `play`, `pause`, `toggle`, `seekBy`, `seekTo`, `next`, `previous`, `setRate`, `setSleep`, `setSkip`, `unload` |
//! | `player_snapshot` | current `Snapshot` (for a UI that mounts while audio plays) |

use streams_native_bridge::{BridgeError, ErrorCode, LoadArgs, PlayerCommand, Snapshot};
use tauri::State;

use super::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub fn player_load(state: State<'_, AppState>, args: LoadArgs) -> CmdResult<()> {
    if args.items.is_empty() {
        return Err(BridgeError::new(ErrorCode::Invalid, "document has no segments"));
    }
    if usize::try_from(args.start_segment).map_or(true, |s| s >= args.items.len()) {
        return Err(BridgeError::new(ErrorCode::Invalid, "start segment out of range"));
    }
    state.engine.load(args)
}

#[tauri::command]
pub fn player_control(state: State<'_, AppState>, command: PlayerCommand) -> CmdResult<()> {
    if let PlayerCommand::SetRate { rate } = command
        && !(0.5..=3.0).contains(&rate)
    {
        return Err(BridgeError::new(ErrorCode::Invalid, "rate must be between 0.5 and 3"));
    }
    state.engine.control(command)
}

#[tauri::command]
pub fn player_snapshot(state: State<'_, AppState>) -> CmdResult<Snapshot> {
    Ok(state.engine.snapshot())
}
