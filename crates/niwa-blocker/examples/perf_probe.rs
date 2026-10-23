//! Times the blocker's hot paths on this PC: what a session's poll costs
//! twice a second. Read-only: nothing is closed.
//!
//!   cargo run --release -p niwa-blocker --example perf_probe

use std::time::Instant;

use niwa_blocker::catalog::InstalledApps;
use niwa_blocker::installed::{suggested_allowed, Installed};
use niwa_blocker::process::{self, SnapshotBuffer};
use niwa_blocker::{app_window_pids, Watcher};
use niwa_core::{BlockListConfig, ListKind};

fn time<T>(label: &str, runs: u32, mut f: impl FnMut() -> T) -> T {
    let start = Instant::now();
    let mut out = f();
    for _ in 1..runs {
        out = f();
    }
    println!("{label:32} {:>8.3} ms", start.elapsed().as_secs_f64() * 1000.0 / f64::from(runs));
    out
}

fn main() {
    time("Installed::scan", 5, Installed::scan);
    let apps = time("InstalledApps::scan", 5, InstalledApps::scan);

    let mut buffer = SnapshotBuffer::default();
    let processes = time("process snapshot", 50, || process::snapshot(&mut buffer).unwrap());
    println!("  {} processes", processes.len());

    let gentle = Installed::scan().block_list(&Installed::scan().default_config()).0;
    let mut watcher = Watcher::new();
    time("default list, first poll", 1, || watcher.poll(&gentle).unwrap());
    time("default list, each poll after", 50, || watcher.poll(&gentle).unwrap());

    let config = BlockListConfig {
        kind: ListKind::AllowOnly,
        allowed: suggested_allowed(&apps),
        ..BlockListConfig::default()
    };
    let allow_only = Installed::scan().block_list(&config).0;
    let mut watcher = Watcher::new();
    time("allow-only, first poll", 1, || watcher.poll(&allow_only).unwrap());
    let found = time("allow-only, each poll after", 50, || watcher.poll(&allow_only).unwrap());
    println!("  {} allow-only candidates", found.len());
    time("app_window_pids", 50, app_window_pids);
}
