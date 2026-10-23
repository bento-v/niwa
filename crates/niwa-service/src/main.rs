//! niwa-service: the background service behind Strict mode.
//!
//!   niwa-service install     register and start the service (needs admin)
//!   niwa-service uninstall   remove it (needs admin; refused during a session)
//!   niwa-service status      show what the service is doing
//!   niwa-service run         what Windows runs; not for use by hand
//!
//! Debug builds add two commands for development:
//!
//!   niwa-service console     run the engine in this terminal, not as a service
//!   niwa-service dev-abort   end the running session at once

mod engine;
mod install;
mod launch;
mod pipe;
mod schedules;
mod service;
mod update;

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::thread;

use engine::{Command, Engine, Settings};
use niwa_core::ipc::{self, Request, Status};

static LOG_FILE: OnceLock<PathBuf> = OnceLock::new();

/// Appends a line to the service's log. A service has no console, so this is
/// the only place its troubles show up.
pub fn log(message: &str) {
    eprintln!("niwa-service: {message}");
    if let Some(path) = LOG_FILE.get() {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{message}");
        }
    }
}

/// Starts the engine and the pipe server on their own threads. Returns the
/// engine's command channel, or `None` if the engine couldn't start.
pub fn start(settings: Settings, active: Arc<AtomicBool>) -> Option<mpsc::Sender<Command>> {
    let _ = LOG_FILE.set(settings.state_dir.join("service.log"));
    let engine = match Engine::open(settings, active) {
        Ok(engine) => engine,
        Err(e) => {
            log(&format!("couldn't open the state folder: {e}"));
            return None;
        }
    };
    let (commands, inbox) = mpsc::channel();
    thread::spawn(move || engine.run(inbox));

    let for_pipe = commands.clone();
    thread::spawn(move || {
        if let Err(e) = pipe::serve(&ipc::pipe_path(), for_pipe) {
            log(&format!("the pipe server stopped: {e}"));
        }
    });
    Some(commands)
}

fn print_status(status: &Status) {
    match (&status.active, &status.finished) {
        (Some(active), _) => {
            println!(
                "Strict session running: {} for {} min, {}:{:02} left, {} slip(s).",
                active.plan.species,
                active.plan.planned_minutes,
                active.remaining_secs / 60,
                active.remaining_secs % 60,
                active.slips.len()
            );
            if let Some(left) = active.exit_secs_left {
                println!("An early end was requested: {}:{:02} until it takes effect.", left / 60, left % 60);
            }
        }
        (None, Some(finished)) => println!(
            "No session running. The last one ended {:?} after {} min.",
            finished.outcome,
            finished.focused_secs / 60
        ),
        (None, None) => println!("No session running."),
    }
}

fn ask(request: Request) -> ExitCode {
    match ipc::request(&request) {
        Ok(Ok(status)) => {
            print_status(&status);
            ExitCode::SUCCESS
        }
        Ok(Err(refusal)) => {
            eprintln!("{refusal}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("The Niwa service isn't running ({e}).");
            ExitCode::FAILURE
        }
    }
}

/// Exit code for "refused: a Strict session is running", which the installer
/// tells apart from other failures.
const EXIT_SESSION_RUNNING: u8 = 3;

fn report(done: Result<(), String>, success: &str) -> ExitCode {
    match done {
        Ok(()) => {
            println!("{success}");
            ExitCode::SUCCESS
        }
        Err(reason) => {
            eprintln!("{reason}");
            match reason == install::SESSION_RUNNING {
                true => ExitCode::from(EXIT_SESSION_RUNNING),
                false => ExitCode::FAILURE,
            }
        }
    }
}

/// Runs the engine in the foreground as the current user, for development.
/// It can only close programs the current user could close anyway.
fn console() -> ExitCode {
    let state_dir = std::env::var_os("NIWA_SERVICE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("niwa-service-dev"));
    let time_scale = std::env::var("NIWA_TIME_SCALE")
        .ok()
        .and_then(|scale| scale.parse().ok())
        .filter(|&scale| scale >= 1)
        .unwrap_or(1);
    println!("Niwa service engine on {} (state in {})", ipc::pipe_path(), state_dir.display());
    let settings = Settings {
        state_dir,
        watch_card: false,
        time_scale,
        run_installers: false,
    };
    match start(settings, Arc::new(AtomicBool::new(false))) {
        Some(_commands) => loop {
            thread::park();
        },
        None => ExitCode::FAILURE,
    }
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();
    match command.as_str() {
        "run" => match service::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("This command is for Windows to run, as a service ({e}).");
                ExitCode::FAILURE
            }
        },
        "install" => report(install::install(), "The Niwa service is installed and running."),
        "uninstall" => report(install::uninstall(), "The Niwa service has been removed."),
        "status" => ask(Request::Status),
        "console" if cfg!(debug_assertions) => console(),
        "dev-abort" if cfg!(debug_assertions) => ask(Request::DevAbort),
        _ => {
            eprintln!("Usage: niwa-service install | uninstall | status");
            ExitCode::FAILURE
        }
    }
}
