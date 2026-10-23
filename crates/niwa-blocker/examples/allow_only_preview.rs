//! Prints what an allow-only list with the suggested work apps would close
//! right now. Read-only: nothing is closed.

use std::collections::BTreeSet;

use niwa_blocker::catalog::InstalledApps;
use niwa_blocker::installed::{suggested_allowed, Installed};
use niwa_blocker::process::{current_owner, owner};
use niwa_blocker::{app_window_pids, Watcher};
use niwa_core::{BlockListConfig, ListKind};

fn main() {
    let suggested = suggested_allowed(&InstalledApps::scan());
    println!("Suggested allowed: {:?}", suggested.iter().map(|e| &e.name).collect::<Vec<_>>());
    let config = BlockListConfig {
        kind: ListKind::AllowOnly,
        allowed: suggested,
        ..BlockListConfig::default()
    };
    let (list, labels) = Installed::scan().block_list(&config);
    println!("Labels: {labels:?}");
    let me = current_owner().expect("own owner");
    let windowed = app_window_pids();
    let candidates = Watcher::new().poll(&list).expect("process list");
    let mut would_close = BTreeSet::new();
    let mut background = BTreeSet::new();
    for process in candidates {
        let mine = owner(process.pid).is_some_and(|o| o == me);
        if mine && windowed.contains(&process.pid) {
            would_close.insert(format!("{}  ({})", process.label, process.image_path.display()));
        } else {
            background.insert(process.label);
        }
    }
    println!("\nWould close (taskbar windows):");
    for line in &would_close {
        println!("  {line}");
    }
    println!("\nLeft alone (no taskbar window, or not yours): {background:?}");
}
