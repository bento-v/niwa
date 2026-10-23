//! The card's side of a Strict session.
//!
//! The session itself runs in the service. The card follows it: shows its
//! progress, asks blocked programs' windows to close (which the service can't
//! do from its own session), records slips, and writes the outcome to the
//! database when it ends. If the service has been killed, the card starts it
//! again.

use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use niwa_blocker::process::image_path;
use niwa_blocker::{app_window_pids, ask_to_close};
use niwa_core::ipc::{self, Active, Finished, Request, PROTOCOL, SERVICE_NAME};
use niwa_core::species::stage;
use niwa_core::store::Outcome;
use niwa_core::strict::SlipRecord;
use niwa_core::Species;
use tauri::{AppHandle, Emitter, Manager};

use crate::session::Running;
use crate::views::{ClosingView, EndedView, SessionView, SlipView};
use crate::{lock, totals, tray, unix_now, AppState};

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const RESTART_SERVICE_EVERY: Duration = Duration::from_secs(5);
/// Stops a console window flashing up when a command-line tool is run.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// What the card knows about the session it is following.
pub struct Followed {
    pub session_id: i64,
    pub species: Species,
    pub planned_minutes: u32,
    pub list_name: String,
    pub blocking: Vec<String>,
    /// Slips already written to the database, so none is recorded twice.
    pub slips_recorded: usize,
    /// Started by a schedule.
    pub scheduled: bool,
}

/// Whether the service is installed and answering.
pub fn available() -> bool {
    ipc::request(&Request::Status).is_ok()
}

/// Whether the running service understands everything this card can ask of
/// it: app categories and allow-only lists came with `PROTOCOL` 2.
pub fn up_to_date() -> bool {
    matches!(ipc::request(&Request::Status), Ok(Ok(status)) if status.version >= PROTOCOL)
}

/// Sends a request and turns "can't reach the service" into words.
pub fn ask(request: &Request) -> Result<(), String> {
    match ipc::request(request) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(refusal)) => Err(refusal),
        Err(_) => Err("Strict mode needs the Niwa service, which isn't running.".into()),
    }
}

/// Starts following a session: one that was just started, or one found still
/// running in the service when the card launched.
pub fn follow(app: AppHandle, followed: Followed) -> (Arc<Running>, JoinHandle<()>) {
    let total_secs = followed.planned_minutes * 60;
    let running = Arc::new(Running::new(
        SessionView {
            species: followed.species.id(),
            species_name: followed.species.name(),
            planned_minutes: followed.planned_minutes,
            mode: "strict",
            list_name: followed.list_name.clone(),
            remaining_secs: total_secs,
            total_secs,
            stage: 1,
            slips: followed.slips_recorded as u32,
            blocking: followed.blocking.clone(),
            scheduled: followed.scheduled,
            closing: Vec::new(),
            exit_secs_left: None,
        },
        true,
    ));
    let handle = thread::spawn({
        let running = running.clone();
        move || run(app, running, followed)
    });
    (running, handle)
}

fn run(app: AppHandle, running: Arc<Running>, mut followed: Followed) {
    // Programs the service is closing, by pid: once one is gone, it was closed.
    let mut asked: HashMap<u32, String> = HashMap::new();
    let mut last_sent: Option<(u32, Vec<ClosingView>, Option<u32>)> = None;
    let mut last_restart = Instant::now() - RESTART_SERVICE_EVERY;
    let mut warned_closing = false;
    // Allow-only: the service can't see the user's desktop, so each poll
    // tells it which programs have taskbar windows.
    let mut report_windows = false;

    loop {
        let request = match report_windows {
            true => Request::Windows {
                pids: app_window_pids().into_iter().collect(),
            },
            false => Request::Status,
        };
        match ipc::request(&request) {
            // The service isn't there. It restarts itself if it was killed,
            // and the card may start it too; keep asking either way.
            Err(_) => {
                if last_restart.elapsed() >= RESTART_SERVICE_EVERY {
                    last_restart = Instant::now();
                    start_service();
                }
            }
            Ok(Err(_)) => {}
            Ok(Ok(status)) => {
                let ours = |id: i64| id == followed.session_id;
                match status.active.filter(|active| ours(active.plan.session_id)) {
                    Some(active) => {
                        report_windows = active.plan.config.kind == niwa_core::ListKind::AllowOnly;
                        record_slips(&app, &mut followed, &active.slips);
                        // Ask each blocked program's windows to close, once.
                        // The service ends it shortly after if it's still there.
                        for program in &active.to_close {
                            if !asked.contains_key(&program.pid) {
                                asked.insert(program.pid, program.label.clone());
                                ask_to_close(program.pid, Path::new(&program.image_path));
                            }
                        }
                        asked.retain(|pid, label| {
                            let pending = active.to_close.iter().any(|p| p.pid == *pid);
                            if !pending && image_path(*pid).is_none() {
                                crate::session::closed(&app, label);
                            }
                            pending
                        });

                        let view = show(&running, &followed, &active);
                        if !warned_closing && !view.closing.is_empty() {
                            warned_closing = true;
                            let labels: Vec<String> = view.closing.iter().map(|c| c.label.clone()).collect();
                            crate::alerts::closing_soon(&app, &labels, view.closing[0].secs_left);
                        }
                        let key = (view.remaining_secs, view.closing.clone(), view.exit_secs_left);
                        if last_sent.as_ref() != Some(&key) {
                            let _ = app.emit("session-tick", &view);
                            tray::show_session(&app, &view);
                            last_sent = Some(key);
                        }
                    }
                    None => {
                        let finished = status.finished.filter(|finished| ours(finished.session_id));
                        finish(&app, &mut followed, finished);
                        return;
                    }
                }
            }
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn show(running: &Running, followed: &Followed, active: &Active) -> SessionView {
    let total = Duration::from_secs(u64::from(followed.planned_minutes) * 60);
    let remaining = Duration::from_secs(active.remaining_secs);
    running.update(|view| {
        view.remaining_secs = active.remaining_secs as u32;
        view.stage = stage(total.saturating_sub(remaining), total);
        view.slips = followed.slips_recorded as u32;
        view.closing = active
            .closing
            .iter()
            .map(|closing| ClosingView {
                label: closing.label.clone(),
                secs_left: closing.secs_left,
            })
            .collect();
        view.exit_secs_left = active.exit_secs_left.map(|secs| secs as u32);
        view.scheduled = active.plan.scheduled;
    })
}

/// Writes any slips the service has seen since the card last looked.
fn record_slips(app: &AppHandle, followed: &mut Followed, slips: &[SlipRecord]) {
    let state = app.state::<AppState>();
    for slip in slips.iter().skip(followed.slips_recorded) {
        let _ = lock(&state.store).record_slip(followed.session_id, slip.at, &slip.label);
        followed.slips_recorded += 1;
        crate::alerts::slip(app, &slip.label, followed.species.name());
        let _ = app.emit(
            "session-slip",
            SlipView {
                label: slip.label.clone(),
                slips: followed.slips_recorded as u32,
            },
        );
    }
}

/// Records how the session ended. If the service no longer knows the session
/// at all, it didn't finish, so the plant withers.
fn finish(app: &AppHandle, followed: &mut Followed, finished: Option<Finished>) {
    let state = app.state::<AppState>();
    let (outcome, focused_secs, remaining_secs) = match &finished {
        Some(finished) => (
            Outcome::from(finished.outcome),
            finished.focused_secs as u32,
            finished.remaining_secs as u32,
        ),
        None => (Outcome::Withered, 0, followed.planned_minutes * 60),
    };
    if let Some(finished) = &finished {
        record_slips(app, followed, &finished.slips);
    }

    let (_, streak_days, today_minutes) = {
        let mut store = lock(&state.store);
        let _ = store.finish_session(followed.session_id, unix_now(), outcome, focused_secs);
        totals(&store)
    };
    *lock(&state.running) = None;
    tray::show_idle(app);
    if outcome == Outcome::Grown {
        crate::alerts::grown(app, followed.species.name(), followed.planned_minutes);
    }
    crate::reveal_window(app);
    let _ = app.emit(
        "session-ended",
        EndedView {
            outcome: outcome.id(),
            species: followed.species.id(),
            species_name: followed.species.name(),
            planned_minutes: followed.planned_minutes,
            remaining_secs,
            slips: followed.slips_recorded as u32,
            today_minutes,
            streak_days,
        },
    );
}

/// Asks Windows to start the service. Ordinary users are allowed to start it
/// (though not to stop it), which is what lets the card bring it back.
fn start_service() {
    let _ = Command::new("sc.exe")
        .args(["start", SERVICE_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}
