//! Every command the webview may call. Names, arguments and results are the `streams-native-bridge` contract
//! (`cmd::*`); each command is registered in `lib.rs` (`generate_handler!`), in `build.rs` (ACL, from
//! `cmd::ALL`), and granted in `capabilities/default.json`. Errors are always `BridgeError` (`{ code, message }`).
//!
//! Commands are thin: validate, call the platform facade (`engine`, `feed`, `secrets`, updater), return.

pub mod app;
pub mod auth;
pub mod downloads;
pub mod files;
pub mod player;
pub mod updater;

use streams_native_bridge::BridgeError;

pub type CmdResult<T> = Result<T, BridgeError>;
