//! The Strict session engine: keeps time, closes blocked programs, and saves
//! the session so it survives the service restarting or the PC rebooting.
//!
//! One thread owns the session. Requests arrive over a channel from the pipe
//! server, so nothing here needs a lock.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use niwa_blocker::clock::awake_time;
use niwa_blocker::installed::Installed;
use niwa_blocker::{close_process, hosts, BlockList, BlockedProcess, Owner, Watcher};
use niwa_core::ipc::{Active, Closing, Finished, Reply, Request, Status, ToClose, PROTOCOL};
use chrono::{Local, NaiveDateTime};
use niwa_core::schedule::Schedule;
use niwa_core::strict::{self, EndedAs, SlipRecord, StrictState};
use niwa_core::Mode;

use crate::launch;

const TICK: Duration = Duration::from_millis(500);
/// How long apps that were already open get to save and quit.
const GRACE: Duration = Duration::from_secs(60);
/// How long the card gets to ask a program to close before it is ended.
const CLOSE_PATIENCE: Duration = Duration::from_secs(2);
/// Closes of the same app within this long of each other are one slip.
const SLIP_WINDOW: Duration = Duration::from_secs(5);
const SAVE_EVERY: Duration = Duration::from_secs(5);
const CARD_CHECK_EVERY: Duration = Duration::from_secs(5);
/// How long the card's report of which programs have taskbar windows holds.
/// Without a report this fresh, no allow-only candidate is closed.
const WINDOWS_FRESH: Duration = Duration::from_secs(3);
/// How often schedules are checked for a window that has come round.
const SCHEDULE_CHECK_EVERY: Duration = Duration::from_secs(20);
pub struct Settings {
    /// Where the session state and its key are kept.
    pub state_dir: PathBuf,
    /// Relaunch the card if it isn't running during a session. Off when the
    /// engine runs in a console for development.
    pub watch_card: bool,
    /// Session seconds per real second. Always 1 outside debug builds.
    pub time_scale: u32,
    /// Run update installers the card hands over (`crate::update`). Off
    /// when the engine runs in a console for development: they're checked
    /// and saved, never run.
    pub run_installers: bool,
}

/// Who sent a request over the pipe.
#[derive(Debug, Clone, Default)]
pub struct Caller {
    /// Their account and login session, if that could be determined.
    pub owner: Option<Owner>,
    /// True when it is the `niwa-app.exe` beside the service.
    pub is_card: bool,
}

pub enum Command {
    /// A request from the pipe, with who sent it.
    Request(Request, Caller, mpsc::Sender<Reply>),
    /// Save and stop: the service is stopping or the PC is shutting down.
    Shutdown(mpsc::Sender<()>),
}

/// What exists only while a session is being enforced.
struct Enforcement {
    list: BlockList,
    watcher: Watcher,
    /// Awake-clock time and session seconds when enforcement (re)started.
    resumed_at: Duration,
    resumed_elapsed: u64,
    /// When the session itself began, for the grace period. `None` after a
    /// resume: nothing gets a second grace period.
    started_at: Option<Duration>,
    open_at_start: HashSet<u32>,
    /// Blocked programs seen and not yet gone, and when each was first seen.
    pending: HashMap<u32, (Instant, BlockedProcess)>,
    counted: HashSet<u32>,
    last_slip: HashMap<String, Instant>,
    closing: Vec<Closing>,
    last_saved: Instant,
    last_card_check: Instant,
    /// Allow-only: the processes the card last said have taskbar windows,
    /// and when it said so.
    windows: Option<(Instant, HashSet<u32>)>,
}

pub struct Engine {
    settings: Settings,
    key: Vec<u8>,
    state: Option<StrictState>,
    enforcement: Option<Enforcement>,
    /// Shared with the service wrapper, which refuses to stop while it's set.
    active: Arc<AtomicBool>,
    /// The card's schedules and the account they belong to.
    schedules: Option<(String, Vec<Schedule>)>,
    /// Windows already seen, so the card is opened at most once for each.
    windows_seen: HashSet<(i64, NaiveDateTime)>,
    last_schedule_check: Instant,
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default()
}

impl Engine {
    /// Loads the key and any saved session from the state folder.
    pub fn open(settings: Settings, active: Arc<AtomicBool>) -> std::io::Result<Engine> {
        fs::create_dir_all(&settings.state_dir)?;
        let key_path = settings.state_dir.join("key.bin");
        let key = match fs::read(&key_path) {
            Ok(key) if key.len() >= 32 => key,
            _ => {
                let mut key = vec![0u8; 32];
                getrandom::fill(&mut key).map_err(|e| std::io::Error::other(e.to_string()))?;
                fs::write(&key_path, &key)?;
                key
            }
        };

        let schedules = crate::schedules::load(&settings.state_dir);
        let mut engine = Engine {
            settings,
            key,
            state: None,
            enforcement: None,
            active,
            schedules,
            windows_seen: HashSet::new(),
            last_schedule_check: Instant::now() - SCHEDULE_CHECK_EVERY,
        };
        engine.load();
        // Earlier versions blocked websites in the hosts file; take out any
        // block one of them left behind.
        if let Err(e) = hosts::clear() {
            crate::log(&format!("couldn't check the hosts file for an old Niwa block: {e}"));
        }
        // Started by an update the card asked for: bring the card back.
        if engine.settings.run_installers {
            crate::update::reopen_card(&engine.settings.state_dir);
        }
        Ok(engine)
    }

    fn state_path(&self) -> PathBuf {
        self.settings.state_dir.join("session.json")
    }

    /// Reads the saved session. A file that fails its seal is ignored, as if
    /// it weren't there: an edited file never becomes the session.
    fn load(&mut self) {
        let Ok(text) = fs::read_to_string(self.state_path()) else {
            return;
        };
        match strict::open(&self.key, &text) {
            Ok(state) => {
                let running = state.is_running();
                self.state = Some(state);
                if running {
                    self.enforce(None);
                }
            }
            Err(reason) => crate::log(&format!("ignoring the saved session: {reason:?}")),
        }
    }

    fn save(&mut self) {
        let Some(state) = &self.state else { return };
        let sealed = strict::seal(&self.key, state);
        // Write beside the file and swap it in, so a crash can't leave half a file.
        let temp = self.settings.state_dir.join("session.json.new");
        let saved = fs::write(&temp, sealed).and_then(|()| fs::rename(&temp, self.state_path()));
        if let Err(e) = saved {
            crate::log(&format!("couldn't save the session: {e}"));
        }
        if let Some(enforcement) = &mut self.enforcement {
            enforcement.last_saved = Instant::now();
        }
    }

    /// Starts enforcing the current session. `started_at` is set for a new
    /// session and `None` when resuming one.
    fn enforce(&mut self, started_at: Option<Duration>) {
        let Some(state) = &self.state else { return };
        let (list, _) = Installed::scan().block_list(&state.plan.config);
        let now = awake_time();
        let mut enforcement = Enforcement {
            list,
            watcher: Watcher::new(),
            resumed_at: now,
            resumed_elapsed: state.elapsed_secs,
            started_at,
            open_at_start: HashSet::new(),
            pending: HashMap::new(),
            counted: HashSet::new(),
            last_slip: HashMap::new(),
            closing: Vec::new(),
            last_saved: Instant::now(),
            // Check for the card on the first tick.
            last_card_check: Instant::now() - CARD_CHECK_EVERY,
            windows: None,
        };
        if started_at.is_some() {
            enforcement.open_at_start = blocked_now(&mut enforcement, &state.user_sid)
                .iter()
                .map(|process| process.pid)
                .collect();
        }
        self.enforcement = Some(enforcement);
        self.active.store(true, Ordering::SeqCst);
    }

    fn stop_enforcing(&mut self) {
        self.enforcement = None;
        self.active.store(false, Ordering::SeqCst);
    }

    /// Runs until the command channel closes or a shutdown is asked for.
    pub fn run(mut self, commands: mpsc::Receiver<Command>) {
        loop {
            match commands.recv_timeout(TICK) {
                Ok(Command::Request(request, caller, reply)) => {
                    let _ = reply.send(self.handle(request, caller));
                }
                Ok(Command::Shutdown(done)) => {
                    self.tick();
                    self.save();
                    let _ = done.send(());
                    return;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    self.save();
                    return;
                }
            }
            self.tick();
            self.check_schedules();
        }
    }

    /// When a schedule's window comes round and the card isn't running, opens
    /// it for the schedule's account: the card starts the session. Once per
    /// window, so a card closed on purpose stays closed.
    fn check_schedules(&mut self) {
        if !self.settings.watch_card || self.last_schedule_check.elapsed() < SCHEDULE_CHECK_EVERY {
            return;
        }
        self.last_schedule_check = Instant::now();
        let Some((user_sid, schedules)) = &self.schedules else {
            return;
        };
        let now = Local::now().naive_local();
        let mut opening = false;
        for schedule in schedules {
            if let Some(window) = schedule.window_at(now) {
                opening |= self.windows_seen.insert((schedule.id, window.starts));
            }
        }
        if opening {
            if let Err(e) = launch::ensure_card_running(user_sid) {
                crate::log(&format!("couldn't open the card for a schedule: {e}"));
            }
        }
    }

    fn handle(&mut self, request: Request, caller: Caller) -> Reply {
        let Caller { owner: caller, is_card } = caller;
        match request {
            Request::Status => {}
            Request::Windows { pids } => {
                // Only the card, signed in as the session's account, can
                // say which windows are open; anyone else is just answered.
                let ours = self.state.as_ref().is_some_and(|state| {
                    caller.as_ref().is_some_and(|caller| caller.session != 0 && caller.sid == state.user_sid)
                });
                if let (true, true, Some(enforcement)) = (is_card, ours, self.enforcement.as_mut()) {
                    enforcement.windows = Some((Instant::now(), pids.into_iter().collect()));
                }
            }
            Request::Schedules { schedules } => {
                let Some(caller) = caller.filter(|caller| is_card && caller.session != 0) else {
                    return Reply::Error("Only Niwa can set its schedules.".into());
                };
                crate::schedules::save(&self.settings.state_dir, &caller.sid, &schedules);
                self.schedules = Some((caller.sid, schedules));
            }
            Request::InstallUpdate { installer, signature, version } => {
                // Only the card, from a login session, and never mid-session:
                // the installer would stop the service and with it the session.
                let Some(caller) = caller.filter(|caller| is_card && caller.session != 0) else {
                    return Reply::Error("Only Niwa can install its updates.".into());
                };
                if self.state.as_ref().is_some_and(StrictState::is_running) {
                    return Reply::Error("Niwa can update once this session ends.".into());
                }
                let state_dir = &self.settings.state_dir;
                let run = self.settings.run_installers;
                if let Err(reason) = crate::update::install(state_dir, &installer, &signature, &version, &caller.sid, run) {
                    return Reply::Error(reason);
                }
            }
            Request::Start { plan } => {
                if self.state.as_ref().is_some_and(StrictState::is_running) {
                    return Reply::Error("A session is already running.".into());
                }
                if plan.config.mode != Mode::Strict {
                    return Reply::Error("Only Strict sessions run in the service.".into());
                }
                if let Err(reason) = plan.validate() {
                    return Reply::Error(reason);
                }
                // The session belongs to whoever asked for it. Without knowing
                // who that is, there is no safe set of programs to close.
                let Some(caller) = caller.filter(|caller| caller.session != 0) else {
                    return Reply::Error("Couldn't tell which account is starting the session.".into());
                };
                self.state = Some(StrictState::new(plan, caller.sid));
                self.enforce(Some(awake_time()));
                self.save();
            }
            Request::RequestExit | Request::CancelExit | Request::DevAbort => {
                let Some(state) = self.state.as_mut().filter(|state| state.is_running()) else {
                    return Reply::Error("No session is running.".into());
                };
                if caller.is_none_or(|caller| caller.sid != state.user_sid) {
                    return Reply::Error("This session belongs to another account.".into());
                }
                match request {
                    Request::RequestExit => state.request_exit(),
                    Request::CancelExit => state.cancel_exit(),
                    _ if cfg!(debug_assertions) => state.outcome = Some(EndedAs::Withered),
                    _ => return Reply::Error("Not available in this build.".into()),
                }
                if !state.is_running() {
                    self.stop_enforcing();
                }
                self.save();
            }
        }
        Reply::Ok(self.status())
    }

    fn status(&self) -> Status {
        let Some(state) = &self.state else {
            return Status {
                version: PROTOCOL,
                ..Status::default()
            };
        };
        if let Some(outcome) = state.outcome {
            return Status {
                version: PROTOCOL,
                active: None,
                finished: Some(Finished {
                    session_id: state.plan.session_id,
                    outcome,
                    focused_secs: state.elapsed_secs,
                    remaining_secs: state.remaining_secs(),
                    slips: state.slips.clone(),
                }),
            };
        }
        let enforcement = self.enforcement.as_ref();
        Status {
            version: PROTOCOL,
            active: Some(Active {
                plan: state.plan.clone(),
                remaining_secs: state.remaining_secs(),
                exit_secs_left: state.exit_secs_left(),
                slips: state.slips.clone(),
                closing: enforcement.map(|e| e.closing.clone()).unwrap_or_default(),
                to_close: enforcement
                    .map(|e| {
                        e.pending
                            .values()
                            .map(|(_, process)| ToClose {
                                pid: process.pid,
                                label: process.label.clone(),
                                image_path: process.image_path.to_string_lossy().into_owned(),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }),
            finished: None,
        }
    }

    fn tick(&mut self) {
        if self.enforce_once() {
            self.save();
        }
        if self.state.as_ref().is_some_and(|state| !state.is_running()) && self.enforcement.is_some() {
            self.stop_enforcing();
        }
    }

    /// One pass over the session: moves time on and deals with blocked
    /// programs. Returns whether the session should be saved now.
    fn enforce_once(&mut self) -> bool {
        let (Some(state), Some(enforcement)) = (self.state.as_mut(), self.enforcement.as_mut()) else {
            return false;
        };

        let now = awake_time();
        let since_resume = now.saturating_sub(enforcement.resumed_at) * self.settings.time_scale;
        state.advance(enforcement.resumed_elapsed + since_resume.as_secs());
        if !state.is_running() {
            return true;
        }

        let in_grace = enforcement
            .started_at
            .is_some_and(|started| now.saturating_sub(started) < GRACE);
        let grace_left = enforcement
            .started_at
            .map(|started| GRACE.saturating_sub(now.saturating_sub(started)))
            .unwrap_or_default();

        let mut dirty = false;
        let mut closing: Vec<Closing> = Vec::new();
        let mut still_blocked = HashSet::new();
        // Allow-only candidates are closed only while the card reports a
        // taskbar window for them.
        let windowed = enforcement
            .windows
            .as_ref()
            .filter(|(at, _)| at.elapsed() < WINDOWS_FRESH)
            .map(|(_, pids)| pids.clone())
            .unwrap_or_default();
        let found: Vec<BlockedProcess> = blocked_now(enforcement, &state.user_sid)
            .into_iter()
            .filter(|process| !process.needs_window || windowed.contains(&process.pid))
            .collect();
        for process in found {
            let was_open = enforcement.open_at_start.contains(&process.pid);
            if was_open && in_grace {
                if !closing.iter().any(|c| c.label == process.label) {
                    closing.push(Closing {
                        label: process.label.clone(),
                        secs_left: grace_left.as_secs_f64().ceil() as u32,
                    });
                }
                continue;
            }
            if !was_open && enforcement.counted.insert(process.pid) {
                let repeat = enforcement
                    .last_slip
                    .insert(process.label.clone(), Instant::now())
                    .is_some_and(|last| last.elapsed() < SLIP_WINDOW);
                if !repeat {
                    state.slips.push(SlipRecord {
                        at: unix_now(),
                        label: process.label.clone(),
                    });
                    dirty = true;
                }
            }

            still_blocked.insert(process.pid);
            let (first_seen, _) = enforcement
                .pending
                .entry(process.pid)
                .or_insert_with(|| (Instant::now(), process.clone()));
            // The card gets a moment to ask the program's windows to close.
            // After that it is ended, and ended again each tick until it's gone.
            if first_seen.elapsed() >= CLOSE_PATIENCE {
                close_process(process.pid, &process.image_path, Duration::ZERO);
            }
        }
        enforcement.pending.retain(|pid, _| still_blocked.contains(pid));
        enforcement.closing = closing;

        let watch_card = self.settings.watch_card && enforcement.last_card_check.elapsed() >= CARD_CHECK_EVERY;
        if watch_card {
            enforcement.last_card_check = Instant::now();
            if let Err(e) = launch::ensure_card_running(&state.user_sid) {
                crate::log(&format!("couldn't start the card: {e}"));
            }
        }

        dirty || enforcement.last_saved.elapsed() >= SAVE_EVERY
    }
}

/// The blocked programs running now that belong to the session's account.
///
/// The service runs as SYSTEM and could end anything. It only ever ends
/// programs owned by the account that started the session, running in a
/// normal login session: never services, never another account's programs.
fn blocked_now(enforcement: &mut Enforcement, user_sid: &str) -> Vec<BlockedProcess> {
    enforcement
        .watcher
        .poll(&enforcement.list)
        .unwrap_or_default()
        .into_iter()
        .filter(|process| {
            process
                .owner
                .as_ref()
                .is_some_and(|owner| owner.session != 0 && owner.sid == user_sid)
        })
        .collect()
}
