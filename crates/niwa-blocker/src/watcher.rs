//! Polls the process list for programs on the block list.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::blocklist::{BlockList, BlockedKind};
use crate::identity::file_identity;
use crate::process::{self, Owner, ProcessEntry, SnapshotBuffer};

/// A running program that the block list says should be closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedProcess {
    pub pid: u32,
    pub image_path: PathBuf,
    pub label: String,
    pub kind: BlockedKind,
    /// Allow-only: close it only while it has a taskbar window
    /// (`process::app_window_pids`) and belongs to the user.
    pub needs_window: bool,
    /// Who it runs as, looked up once when it was first seen. `None` if
    /// Windows wouldn't say.
    pub owner: Option<Owner>,
}

/// Remembers which processes have already been checked, so each poll only
/// looks up the full path (and owner) of processes that are new since the
/// last one.
#[derive(Default)]
pub struct Watcher {
    /// `None` means checked and allowed (or not inspectable).
    seen: HashMap<ProcessEntry, Option<BlockedProcess>>,
    buffer: SnapshotBuffer,
}

impl Watcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns every blocked program that is running right now. A program
    /// keeps being returned on each poll until it exits.
    pub fn poll(&mut self, list: &BlockList) -> windows::core::Result<Vec<BlockedProcess>> {
        let running = process::snapshot(&mut self.buffer)?;
        let mut seen = HashMap::with_capacity(running.len());
        let mut blocked = Vec::new();

        for entry in running {
            // The entry (pid and start time, with parent and name) is the
            // process's identity, so a recycled pid is checked afresh.
            let verdict = match self.seen.remove(&entry) {
                Some(verdict) => verdict,
                None => process::image_path(entry.pid).and_then(|image_path| {
                    list.check(&image_path, || file_identity(&image_path))
                        .map(|hit| BlockedProcess {
                            pid: entry.pid,
                            image_path,
                            label: hit.label,
                            kind: hit.kind,
                            needs_window: hit.needs_window,
                            owner: process::owner(entry.pid),
                        })
                }),
            };
            blocked.extend(verdict.clone());
            seen.insert(entry, verdict);
        }

        self.seen = seen;
        Ok(blocked)
    }
}
