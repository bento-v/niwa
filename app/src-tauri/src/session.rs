//! Runs a focus session on a background thread: keeps time, closes blocked
//! apps, and tells the card and the tray what's happening.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use niwa_blocker::clock::awake_time;
use niwa_blocker::process::current_owner;
use niwa_blocker::{app_window_pids, close_process, BlockList, BlockedProcess, CloseOutcome, Watcher};
use niwa_core::species::stage;
use niwa_core::store::Outcome;
use niwa_core::{Mode, Species};
use tauri::{AppHandle, Emitter, Manager};

use crate::views::{ClosedView, ClosingView, EndedView, SessionView, SlipView};
use crate::{lock, totals, tray, unix_now, AppState};

const POLL_INTERVAL: Duration = Duration::from_millis(500);
/// How long apps that were already open get to save and quit.
pub const GRACE: Duration = Duration::from_secs(60);
/// How long an app gets to close itself before it is terminated.
const CLOSE_PATIENCE: Duration = Duration::from_secs(2);
/// Closes of the same app within this long of each other are one slip: one
/// launch can start several blocked processes.
const SLIP_WINDOW: Duration = Duration::from_secs(5);
pub struct Plan {
    pub session_id: i64,
    pub species: Species,
    pub planned_minutes: u32,
    pub mode: Mode,
    pub list_name: String,
    pub list: BlockList,
    pub blocking: Vec<String>,
    /// Started by a schedule.
    pub scheduled: bool,
}

/// The part of a running session other threads can reach.
pub struct Running {
    /// Strict sessions run in the service; the card only follows them.
    pub strict: bool,
    give_up: AtomicBool,
    /// Gentle: when ending early was asked for, on the awake clock. The
    /// session withers once the wait (`Mode::exit_wait_secs`) has passed.
    exit_requested_at: Mutex<Option<Duration>>,
    view: Mutex<SessionView>,
}

impl Running {
    pub fn new(view: SessionView, strict: bool) -> Running {
        Running {
            strict,
            give_up: AtomicBool::new(false),
            exit_requested_at: Mutex::new(None),
            view: Mutex::new(view),
        }
    }

    /// Changes the view and returns the new one.
    pub fn update(&self, change: impl FnOnce(&mut SessionView)) -> SessionView {
        let mut view = lock(&self.view);
        change(&mut view);
        view.clone()
    }

    /// Ends the session now. Only for quitting Niwa, which can't wait.
    pub fn give_up(&self) {
        self.give_up.store(true, Ordering::SeqCst);
    }

    /// Gentle: asks to end early. The wait starts now; asking again doesn't
    /// restart it.
    pub fn request_exit(&self) {
        lock(&self.exit_requested_at).get_or_insert_with(awake_time);
    }

    /// Gentle: calls off a request to end early.
    pub fn cancel_exit(&self) {
        *lock(&self.exit_requested_at) = None;
    }

    pub fn view(&self) -> SessionView {
        lock(&self.view).clone()
    }
}

/// Debug builds can run sessions faster than real time for testing:
/// `NIWA_TIME_SCALE=60` makes a minute pass every second.
fn time_scale() -> u32 {
    if !cfg!(debug_assertions) {
        return 1;
    }
    std::env::var("NIWA_TIME_SCALE")
        .ok()
        .and_then(|scale| scale.parse().ok())
        .filter(|&scale| scale >= 1)
        .unwrap_or(1)
}

pub fn start(app: AppHandle, plan: Plan) -> (Arc<Running>, JoinHandle<()>) {
    let total = Duration::from_secs(u64::from(plan.planned_minutes) * 60);
    let running = Arc::new(Running::new(
        SessionView {
            species: plan.species.id(),
            species_name: plan.species.name(),
            planned_minutes: plan.planned_minutes,
            mode: plan.mode.id(),
            list_name: plan.list_name.clone(),
            remaining_secs: total.as_secs() as u32,
            total_secs: total.as_secs() as u32,
            stage: 1,
            slips: 0,
            blocking: plan.blocking.clone(),
            scheduled: plan.scheduled,
            closing: Vec::new(),
            exit_secs_left: None,
        },
        false,
    ));
    let handle = thread::spawn({
        let running = running.clone();
        move || run(app, running, plan, total)
    });
    (running, handle)
}

fn run(app: AppHandle, running: Arc<Running>, plan: Plan, total: Duration) {
    let scale = time_scale();
    let start = awake_time();
    let mut watcher = Watcher::new();

    // Apps already open get the grace period; anything launched after this
    // point is closed at once and counts as a slip.
    let open_at_start: HashSet<u32> = watcher
        .poll(&plan.list)
        .unwrap_or_default()
        .iter()
        .map(|process| process.pid)
        .collect();
    let being_closed: Arc<Mutex<HashSet<u32>>> = Arc::default();
    let mut counted: HashSet<u32> = HashSet::new();
    let mut last_slip: HashMap<String, Duration> = HashMap::new();
    let mut slips = 0u32;
    let mut last_sent: Option<(u32, Vec<ClosingView>, Option<u32>)> = None;
    let exit_wait = Duration::from_secs(plan.mode.exit_wait_secs(plan.planned_minutes));
    let mut warned_closing = false;

    // Allow-only lists close only the user's own programs, and only while
    // they show a taskbar window.
    let me = current_owner();
    let elapsed = loop {
        let awake = awake_time().saturating_sub(start);
        let elapsed = (awake * scale).min(total);
        // How long a requested early end still has to wait, if one was asked for.
        let exit_left = lock(&running.exit_requested_at)
            .map(|asked| exit_wait.saturating_sub(awake_time().saturating_sub(asked) * scale));
        // Finishing wins over a pending early end, as in Strict.
        if running.give_up.load(Ordering::SeqCst) || elapsed >= total || exit_left == Some(Duration::ZERO) {
            break elapsed;
        }
        let exit_secs_left = exit_left.map(|left| left.as_secs_f64().ceil() as u32);

        let mut closing: Vec<ClosingView> = Vec::new();
        let found = watcher.poll(&plan.list).unwrap_or_default();
        let windowed = match found.iter().any(|process| process.needs_window) {
            true => app_window_pids(),
            false => HashSet::new(),
        };
        let closable = found.into_iter().filter(|process| {
            !process.needs_window
                || (windowed.contains(&process.pid) && me.is_some() && process.owner == me)
        });
        for process in closable {
            let was_open = open_at_start.contains(&process.pid);
            if was_open && awake < GRACE {
                if !closing.iter().any(|c| c.label == process.label) {
                    closing.push(ClosingView {
                        label: process.label.clone(),
                        secs_left: (GRACE - awake).as_secs_f64().ceil() as u32,
                    });
                }
                continue;
            }
            if !was_open && counted.insert(process.pid) {
                let repeat = last_slip
                    .insert(process.label.clone(), awake)
                    .is_some_and(|last| awake.saturating_sub(last) < SLIP_WINDOW);
                if !repeat {
                    slips += 1;
                    record_slip(&app, &plan, &process.label, slips);
                }
            }
            close_in_background(&app, process, &being_closed);
        }

        let remaining = total - elapsed;
        let remaining_secs = remaining.as_secs_f64().ceil() as u32;
        let view = {
            let mut view = lock(&running.view);
            view.remaining_secs = remaining_secs;
            view.stage = stage(elapsed, total);
            view.slips = slips;
            view.closing = closing.clone();
            view.exit_secs_left = exit_secs_left;
            view.clone()
        };
        if !warned_closing && !closing.is_empty() {
            warned_closing = true;
            let labels: Vec<String> = closing.iter().map(|c| c.label.clone()).collect();
            crate::alerts::closing_soon(&app, &labels, closing[0].secs_left);
        }
        // The card only needs telling when something it shows has changed.
        let key = (remaining_secs, closing, exit_secs_left);
        if last_sent.as_ref() != Some(&key) {
            let _ = app.emit("session-tick", &view);
            tray::show_session(&app, &view);
            last_sent = Some(key);
        }

        thread::sleep(POLL_INTERVAL.min(remaining / scale));
    };

    let outcome = match elapsed >= total {
        true => Outcome::Grown,
        false => Outcome::Withered,
    };
    finish(&app, &plan, outcome, elapsed, total, slips);
}

fn record_slip(app: &AppHandle, plan: &Plan, label: &str, slips: u32) {
    let state = app.state::<AppState>();
    let _ = lock(&state.store).record_slip(plan.session_id, unix_now(), label);
    crate::alerts::slip(app, label, plan.species.name());
    let _ = app.emit(
        "session-slip",
        SlipView {
            label: label.to_owned(),
            slips,
        },
    );
}

/// Closing waits for the app to quit, so it runs off the timer thread. A
/// process that is still being closed isn't started on twice.
fn close_in_background(app: &AppHandle, process: BlockedProcess, being_closed: &Arc<Mutex<HashSet<u32>>>) {
    if !lock(being_closed).insert(process.pid) {
        return;
    }
    let being_closed = being_closed.clone();
    let app = app.clone();
    thread::spawn(move || {
        let outcome = close_process(process.pid, &process.image_path, CLOSE_PATIENCE);
        lock(&being_closed).remove(&process.pid);
        // Only what Niwa itself closed: an app that quit on its own isn't news.
        if matches!(outcome, CloseOutcome::ClosedWhenAsked | CloseOutcome::Terminated) {
            closed(&app, &process.label);
        }
    });
}

/// Tells the card and the mini timer that an app was just closed.
pub fn closed(app: &AppHandle, label: &str) {
    let _ = app.emit(
        "app-closed",
        ClosedView {
            label: label.to_owned(),
        },
    );
}

fn finish(
    app: &AppHandle,
    plan: &Plan,
    outcome: Outcome,
    elapsed: Duration,
    total: Duration,
    slips: u32,
) {
    let state = app.state::<AppState>();
    let (_, streak_days, today_minutes) = {
        let mut store = lock(&state.store);
        let _ = store.finish_session(
            plan.session_id,
            unix_now(),
            outcome,
            elapsed.as_secs() as u32,
        );
        totals(&store)
    };
    *lock(&state.running) = None;
    tray::show_idle(app);
    if outcome == Outcome::Grown {
        crate::alerts::grown(app, plan.species.name(), plan.planned_minutes);
    }

    // A finished plant is worth seeing: bring the card back if it was hidden.
    crate::reveal_window(app);
    let _ = app.emit(
        "session-ended",
        EndedView {
            outcome: outcome.id(),
            species: plan.species.id(),
            species_name: plan.species.name(),
            planned_minutes: plan.planned_minutes,
            remaining_secs: (total - elapsed).as_secs_f64().ceil() as u32,
            slips,
            today_minutes,
            streak_days,
        },
    );
}
