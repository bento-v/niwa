//! Milestone 1 prototype: a focus session in the terminal.
//!
//! It runs a timer and closes anything on the block list while the timer
//! runs. There is no plant, garden or storage yet; Ctrl+C gives up.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use clap::{Parser, Subcommand};
use niwa_blocker::clock::awake_time;
use niwa_blocker::{close_process, steam, AppRule, BlockList, BlockedProcess, CloseOutcome, Watcher};

const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// How long an app gets to close itself before it is terminated.
const CLOSE_PATIENCE: Duration = Duration::from_secs(2);
/// Closes of the same app within this long of each other are one slip.
const SLIP_WINDOW: Duration = Duration::from_secs(5);

#[derive(Parser)]
#[command(name = "niwa", about = "Niwa blocker prototype")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the Steam games Niwa can find on this PC.
    Scan,
    /// Run a focus session that closes blocked apps until the timer ends.
    Run {
        /// Session length in minutes.
        #[arg(long, default_value_t = 25.0)]
        minutes: f64,
        /// Block every Steam game.
        #[arg(long)]
        steam: bool,
        /// Block an app by file name (notepad.exe) or full path. Repeatable.
        #[arg(long = "app", value_name = "NAME_OR_PATH")]
        apps: Vec<String>,
        /// Keep an executable or folder allowed. Repeatable.
        #[arg(long = "allow", value_name = "PATH")]
        exceptions: Vec<PathBuf>,
        /// Seconds that apps already open at the start get to save and quit.
        #[arg(long, default_value_t = 60)]
        grace: u64,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Scan => scan(),
        Command::Run {
            minutes,
            steam,
            apps,
            exceptions,
            grace,
        } => run(minutes, steam, apps, exceptions, Duration::from_secs(grace)),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("niwa: {message}");
            ExitCode::FAILURE
        }
    }
}

fn load_steam() -> Result<steam::SteamLibraries, String> {
    let root = steam::find_steam_root().ok_or("couldn't find a Steam install")?;
    steam::scan(&root).map_err(|e| format!("couldn't read Steam's library files: {e}"))
}

fn scan() -> Result<(), String> {
    let libraries = load_steam()?;
    println!("Steam libraries:");
    for dir in &libraries.common_dirs {
        println!("  {}", dir.display());
    }
    println!("{} games:", libraries.games.len());
    for game in &libraries.games {
        println!("  {} ({})", game.name, game.install_dir.display());
    }
    Ok(())
}

fn run(
    minutes: f64,
    block_steam: bool,
    apps: Vec<String>,
    exceptions: Vec<PathBuf>,
    grace: Duration,
) -> Result<(), String> {
    if !(minutes > 0.0 && minutes.is_finite()) {
        return Err("--minutes must be more than 0".into());
    }
    if !block_steam && apps.is_empty() {
        return Err("nothing to block: pass --steam, --app, or both".into());
    }

    let libraries = match block_steam {
        true => vec![load_steam()?.into_library()],
        false => Vec::new(),
    };
    let list = BlockList {
        libraries,
        apps: apps.into_iter().map(app_rule).collect(),
        exceptions,
        ..BlockList::default()
    };
    for library in &list.libraries {
        println!("Blocking {} {} games.", library.games.len(), library.source);
    }
    for rule in &list.apps {
        match rule {
            AppRule::Path(path) => println!("Blocking {}.", path.display()),
            AppRule::FileName(name) | AppRule::Picked { name, .. } => {
                println!("Blocking {name}.")
            }
            AppRule::Known(known) => println!("Blocking {}.", known.name),
        }
    }

    let total = Duration::from_secs_f64(minutes * 60.0);
    let start = awake_time();
    let mut watcher = Watcher::new();
    let mut session = Session::default();

    // Apps that are already open get the grace period; anything launched
    // after this point is closed at once.
    let already_open = poll(&mut watcher, &list)?;
    let grace_pids: HashSet<u32> = already_open.iter().map(|p| p.pid).collect();
    if !already_open.is_empty() {
        println!(
            "Already open, closing in {} s — save and quit now:",
            grace.as_secs()
        );
        for process in &already_open {
            println!("  {} ({})", process.label, process.image_path.display());
        }
        println!("To keep one open, give up now and add --allow <its folder>.");
    }
    println!("Session started. Press Ctrl+C to give up.");

    loop {
        let elapsed = awake_time().saturating_sub(start);
        if elapsed >= total {
            break;
        }

        for process in poll(&mut watcher, &list)? {
            let in_grace = grace_pids.contains(&process.pid) && elapsed < grace;
            if !in_grace {
                session.close(&process, !grace_pids.contains(&process.pid), elapsed);
            }
        }

        status(total.saturating_sub(elapsed), session.slip_count());
        thread::sleep(POLL_INTERVAL.min(total.saturating_sub(elapsed)));
    }

    clear_status();
    println!("Session finished: {} focused.", minutes_text(total));
    if session.slips.is_empty() {
        println!("No slips.");
    } else {
        println!("Slips:");
        for (label, count) in &session.slips {
            println!("  {label}: {count}");
        }
    }
    Ok(())
}

fn app_rule(app: String) -> AppRule {
    if app.contains(['\\', '/']) {
        AppRule::Path(PathBuf::from(app))
    } else {
        AppRule::FileName(app)
    }
}

fn poll(watcher: &mut Watcher, list: &BlockList) -> Result<Vec<BlockedProcess>, String> {
    watcher
        .poll(list)
        .map_err(|e| format!("couldn't list running processes: {e}"))
}

#[derive(Default)]
struct Session {
    /// Launches of blocked apps during the session, by app.
    slips: BTreeMap<String, u32>,
    /// When each app was last closed as a slip, in session time.
    last_slip: HashMap<String, Duration>,
    /// Processes already counted or reported, so a process that takes more
    /// than one poll to close isn't counted twice.
    handled: HashSet<u32>,
}

impl Session {
    fn slip_count(&self) -> u32 {
        self.slips.values().sum()
    }

    fn close(&mut self, process: &BlockedProcess, is_slip: bool, elapsed: Duration) {
        let first_attempt = self.handled.insert(process.pid);
        if first_attempt && is_slip {
            // One launch can start several blocked processes (a bootstrapper
            // and the game itself). Count those as one slip.
            let repeat = self
                .last_slip
                .insert(process.label.clone(), elapsed)
                .is_some_and(|last| elapsed.saturating_sub(last) < SLIP_WINDOW);
            if !repeat {
                *self.slips.entry(process.label.clone()).or_default() += 1;
            }
        }

        let outcome = close_process(process.pid, &process.image_path, CLOSE_PATIENCE);
        if !first_attempt && !outcome.is_closed() {
            // Already reported; keep retrying quietly.
            return;
        }
        clear_status();
        match outcome {
            CloseOutcome::ClosedWhenAsked | CloseOutcome::Terminated => {
                println!("Closed {}.", process.label)
            }
            CloseOutcome::AlreadyGone => println!("{} quit on its own.", process.label),
            CloseOutcome::NotTheSameProgram => {}
            CloseOutcome::Failed(reason) => {
                println!("Couldn't close {}: {}", process.label, reason.trim())
            }
        }
    }
}

fn status(left: Duration, slips: u32) {
    let seconds = left.as_secs_f64().ceil() as u64;
    print!(
        "\r{:02}:{:02} left · {} slip{}   ",
        seconds / 60,
        seconds % 60,
        slips,
        if slips == 1 { "" } else { "s" }
    );
    let _ = std::io::stdout().flush();
}

fn clear_status() {
    print!("\r{:40}\r", "");
}

fn minutes_text(duration: Duration) -> String {
    let seconds = duration.as_secs();
    match (seconds / 60, seconds % 60) {
        (0, s) => format!("{s} s"),
        (m, 0) => format!("{m} min"),
        (m, s) => format!("{m} min {s} s"),
    }
}
