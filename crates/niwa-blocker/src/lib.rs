//! Niwa's blocking engine: finds installed games, matches running programs
//! against a block list and closes the ones that match.

pub mod blocklist;
pub mod catalog;
pub mod clock;
pub mod epic;
pub mod hosts;
pub mod identity;
pub mod installed;
pub mod process;
pub mod riot;
pub mod steam;
pub mod vdf;
pub mod watcher;

pub use blocklist::{
    AllowOnly, AppRule, BlockList, Blocked, BlockedKind, FileIdentity, Game, GameLibrary, Launcher,
};
pub use process::{app_window_pids, ask_to_close, close_process, CloseOutcome, Owner};
pub use watcher::{BlockedProcess, Watcher};
