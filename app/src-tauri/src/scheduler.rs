//! Starts sessions when a schedule's window comes round (added Oct 17, 2026).
//!
//! Every few seconds it looks for a window that's on now and hasn't been
//! dealt with. With no session running it starts one with the block list for
//! the rest of the window (joining late if Niwa wasn't running when it began),
//! with the seed last planted if it can grow in that time, or else the rarest
//! that can. A window is dealt with once its session starts, or when it can't:
//! too little of it left, or the start failed. It's never started twice, so a
//! session ended early stays ended. A minute before a window, a notification
//! says it's coming.

use std::collections::HashSet;
use std::time::Duration;

use chrono::{Local, NaiveDateTime, TimeZone};
use niwa_core::schedule::{Schedule, MAX_SCHEDULED_MINUTES};
use niwa_core::species::MIN_SESSION_MINUTES;
use niwa_core::{Species, Store};
use tauri::{AppHandle, Manager};

use crate::{lock, AppState};

const LOOK_EVERY: Duration = Duration::from_secs(15);
/// How long before a window the heads-up comes.
const WARN_AHEAD_SECS: i64 = 60;

fn unix(at: NaiveDateTime) -> i64 {
    Local
        .from_local_datetime(&at)
        .earliest()
        .map(|at| at.timestamp())
        .unwrap_or_else(|| at.and_utc().timestamp())
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut warned: HashSet<(i64, NaiveDateTime)> = HashSet::new();
        loop {
            look(&app, &mut warned);
            std::thread::sleep(LOOK_EVERY);
        }
    });
}

fn look(app: &AppHandle, warned: &mut HashSet<(i64, NaiveDateTime)>) {
    let state = app.state::<AppState>();
    let Ok(schedules) = lock(&state.store).schedules() else {
        return;
    };
    let now = Local::now().naive_local();
    let running = lock(&state.running).is_some();

    for schedule in &schedules {
        let Some(next) = schedule.next_window(now) else { continue };
        let ahead = (next.starts - now).num_seconds();
        if !running && ahead <= WARN_AHEAD_SECS && warned.insert((schedule.id, next.starts)) {
            crate::alerts::focus_time_soon(app, &crate::format_time(next.starts));
        }
    }
    // A session already going carries on; the window can still be joined
    // when it ends.
    if running {
        return;
    }
    for schedule in &schedules {
        let Some(window) = schedule.window_at(now) else { continue };
        let ends = unix(window.ends);
        let handled = lock(&state.store).schedule_handled_until(schedule.id).unwrap_or(0);
        if handled >= ends {
            continue;
        }
        let _ = lock(&state.store).set_schedule_handled_until(schedule.id, ends);
        let minutes = ((window.ends - now).num_seconds() / 60).clamp(0, i64::from(MAX_SCHEDULED_MINUTES)) as u32;
        if minutes < MIN_SESSION_MINUTES {
            continue;
        }
        let species = seed_for(&lock(&state.store), minutes);
        match crate::begin_session(app, minutes, species, true) {
            Ok(_) => crate::reveal_window(app),
            Err(reason) => crate::alerts::focus_time_failed(app, &crate::format_time(window.starts), &reason),
        }
        return;
    }
}

/// The seed for a scheduled session: the last one planted if it's unlocked
/// and can grow in `minutes`, or else the rarest that can.
fn seed_for(store: &Store, minutes: u32) -> Species {
    let focused = store.focused_minutes().unwrap_or(0);
    let usable = |species: &Species| species.is_unlocked(focused) && species.fits_session(minutes);
    store
        .last_seed()
        .ok()
        .flatten()
        .filter(usable)
        .or_else(|| Species::ALL.iter().copied().filter(usable).last())
        .unwrap_or(Species::ALL[0])
}

/// Tells the Strict service the schedules, so it can open Niwa when one
/// comes round with Niwa closed. Nothing happens without the service.
pub fn share_with_service(schedules: Vec<Schedule>) {
    std::thread::spawn(move || {
        if crate::strict::up_to_date() {
            let _ = crate::strict::ask(&niwa_core::ipc::Request::Schedules { schedules });
        }
    });
}
