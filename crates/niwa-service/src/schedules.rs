//! The card's schedules, kept in the state folder so the service still knows
//! them after a restart: when a window comes round with the card closed, the
//! service opens it (`Engine::check_schedules`).

use std::fs;
use std::path::Path;

use niwa_core::schedule::Schedule;

const FILE: &str = "schedules.json";

/// The schedules and the account (SID) that set them, if any were saved.
pub fn load(state_dir: &Path) -> Option<(String, Vec<Schedule>)> {
    let text = fs::read_to_string(state_dir.join(FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save(state_dir: &Path, user_sid: &str, schedules: &[Schedule]) {
    let saved = serde_json::to_string(&(user_sid, schedules))
        .map_err(std::io::Error::other)
        .and_then(|text| fs::write(state_dir.join(FILE), text));
    if let Err(e) = saved {
        crate::log(&format!("couldn't save the schedules: {e}"));
    }
}
