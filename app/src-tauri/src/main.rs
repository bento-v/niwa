//! The Niwa desktop app: a Tauri shell around the blocker and the core rules.

// A windowed program in every build: a console program gets a terminal window
// whenever something other than a terminal starts it, such as the service.
#![windows_subsystem = "windows"]

mod alerts;
mod blocks;
mod mini;
mod session;
mod strict;
mod scheduler;
mod tray;
mod update;
mod views;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{Local, TimeZone};
use niwa_core::schedule::Schedule;
use niwa_core::species::{next_unlock, MAX_SESSION_MINUTES, MIN_SESSION_MINUTES};
use niwa_blocker::installed::Installed;
use niwa_core::ipc::Request;
use niwa_core::store::{NewSession, Outcome};
use niwa_core::strict::StrictPlan;
use niwa_core::stats::{self, EndedSession, SlipOn};
use niwa_core::streak::{local_day, streak};
use niwa_core::{AppEntry, BlockListConfig, ExceptionEntry, Mode, Species, Store};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow, WindowEvent,
};
use tauri_plugin_dialog::DialogExt;

use session::{Plan, Running};
use strict::Followed;
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use views::{
    BlockListView, GardenView, NextUnlock, Overview, PlantView, SessionView, SettingsView, SpeciesView,
    StatsView, TemptingView, UpdateView,
};

/// The card's size, as "width,height" in logical pixels, once it's been resized.
const CARD_SIZE: &str = "card_size";
/// The smallest card (the size it was designed at, plus the room around it
/// for its shadow) and the largest.
const CARD_MIN: (f64, f64) = (416.0, 666.0);
const CARD_MAX: (f64, f64) = (1000.0, 1300.0);
/// Where a resize left the card, waiting to be saved.
static UNSAVED_CARD_SIZE: Mutex<Option<(f64, f64)>> = Mutex::new(None);
/// Where the card last was, and how big, while it was a whole card on screen.
static CARD_PLACE: Mutex<Option<(PhysicalPosition<i32>, PhysicalSize<u32>)>> = Mutex::new(None);
/// Whether the chime plays when a plant is fully grown. On unless turned off.
const SOUND: &str = "sound";

/// A yes/no setting, or `default` if it has never been set.
fn flag(store: &Store, key: &str, default: bool) -> bool {
    match store.setting(key).ok().flatten().as_deref() {
        Some("true") => true,
        Some("false") => false,
        _ => default,
    }
}

/// Unix time at the start of a local calendar day.
fn local_midnight(day: chrono::NaiveDate) -> i64 {
    day.and_hms_opt(0, 0, 0)
        .and_then(|midnight| midnight.and_local_timezone(Local).earliest())
        .map(|midnight| midnight.timestamp())
        .unwrap_or_default()
}

pub struct AppState {
    store: Mutex<Store>,
    running: Mutex<Option<Arc<Running>>>,
    runner: Mutex<Option<JoinHandle<()>>>,
    tray: Mutex<Option<tray::TrayItems>>,
}

/// Locks a mutex, carrying on with the data if another thread panicked while
/// holding it: nothing here is left half-updated across a lock.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default()
}

/// Total focused minutes, the streak in days, and minutes focused today.
pub fn totals(store: &Store) -> (u32, u32, u32) {
    let today = Local::now().date_naive();
    let days: Vec<_> = store
        .finished_times()
        .unwrap_or_default()
        .into_iter()
        .map(local_day)
        .collect();
    let midnight = local_midnight(today);
    (
        store.focused_minutes().unwrap_or_default(),
        streak(&days, today),
        store.focused_minutes_since(midnight).unwrap_or_default(),
    )
}

/// The user's block list. There is one for now; switching lists comes later.
fn block_list(store: &Store) -> Result<niwa_core::StoredBlockList, String> {
    store
        .block_lists()
        .map_err(|e| e.to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "No block list found.".to_owned())
}

#[tauri::command]
fn get_overview(state: State<AppState>) -> Result<Overview, String> {
    let session = lock(&state.running).as_ref().map(|running| running.view());
    let store = lock(&state.store);
    let (focused_minutes, streak_days, _) = totals(&store);
    let list = block_list(&store)?;
    Ok(Overview {
        session,
        focused_minutes,
        streak_days,
        species: Species::ALL
            .into_iter()
            .map(|species| SpeciesView::new(species, focused_minutes))
            .collect(),
        list: blocks::view(&list.name, &list.config, &Installed::scan()),
        strict_available: strict::available(),
        min_minutes: MIN_SESSION_MINUTES,
        max_minutes: MAX_SESSION_MINUTES,
    })
}

#[tauri::command]
fn get_garden(state: State<AppState>) -> Result<GardenView, String> {
    let store = lock(&state.store);
    let plants: Vec<PlantView> = store
        .garden()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(PlantView::from)
        .collect();
    let withered = plants.iter().filter(|plant| plant.withered).count();
    Ok(GardenView {
        grown: plants.len() - withered,
        withered,
        plants,
        next_unlock: next_unlock(store.focused_minutes().unwrap_or_default()).map(|species| {
            NextUnlock {
                name: species.name(),
                hours: species.unlock_hours(),
            }
        }),
    })
}

#[tauri::command]
fn start_session(app: AppHandle, minutes: u32, species: String) -> Result<SessionView, String> {
    let species = Species::from_id(&species).ok_or("Unknown seed.")?;
    begin_session(&app, minutes, species, false)
}

/// Starts a session with the block list: from the card, or from a schedule
/// (`scheduled`), which may run up to `MAX_SCHEDULED_MINUTES`.
pub fn begin_session(app: &AppHandle, minutes: u32, species: Species, scheduled: bool) -> Result<SessionView, String> {
    niwa_core::strict::check_length(minutes, scheduled)?;
    let state = app.state::<AppState>();
    let mut running = lock(&state.running);
    if running.is_some() {
        return Err("A session is already running.".into());
    }

    let stored = {
        let store = lock(&state.store);
        let focused_minutes = store.focused_minutes().map_err(|e| e.to_string())?;
        if !species.is_unlocked(focused_minutes) {
            return Err(format!(
                "{} unlocks at {} hours of focus.",
                species.name(),
                species.unlock_hours()
            ));
        }
        if !species.fits_session(minutes) {
            return Err(format!(
                "{} needs a session of at least {} minutes.",
                species.name(),
                species.min_minutes()
            ));
        }
        block_list(&store)?
    };
    let strict = stored.config.mode == Mode::Strict;
    if strict && !strict::available() {
        return Err("Strict mode needs the Niwa service, which isn't running.".into());
    }
    // An older service would start the session but ignore what it doesn't
    // know, leaving the list half enforced.
    if strict && !strict::up_to_date() {
        return Err(
            "This list needs the updated Niwa service. Install the latest Niwa (or restart the \
             NiwaStrict service from an admin terminal), then try again."
                .into(),
        );
    }

    let (list, blocking) = Installed::scan().block_list(&stored.config);
    let session_id = lock(&state.store)
        .start_session(&NewSession {
            started_at: unix_now(),
            planned_minutes: minutes,
            mode: stored.config.mode,
            species,
            block_list: &stored.name,
        })
        .map_err(|e| e.to_string())?;

    let (session, handle) = if strict {
        // The service runs the session; the card follows it.
        let plan = StrictPlan {
            session_id,
            species: species.id().to_owned(),
            planned_minutes: minutes,
            list_name: stored.name.clone(),
            config: stored.config,
            scheduled,
        };
        if let Err(refusal) = strict::ask(&Request::Start { plan }) {
            let _ = lock(&state.store).discard_session(session_id);
            return Err(refusal);
        }
        strict::follow(
            app.clone(),
            Followed {
                session_id,
                species,
                planned_minutes: minutes,
                list_name: stored.name,
                blocking,
                slips_recorded: 0,
                scheduled,
            },
        )
    } else {
        session::start(
            app.clone(),
            Plan {
                session_id,
                species,
                planned_minutes: minutes,
                mode: stored.config.mode,
                list_name: stored.name,
                list,
                blocking,
                scheduled,
            },
        )
    };
    let view = session.view();
    *running = Some(session);
    *lock(&state.runner) = Some(handle);
    tray::show_session(app, &view);
    Ok(view)
}


/// Starts the wait that ends the session early: five minutes in Gentle, a
/// third of the session in Strict (`Mode::exit_wait_secs`).
#[tauri::command]
fn request_exit(state: State<AppState>) -> Result<(), String> {
    match lock(&state.running).as_ref() {
        Some(running) if running.strict => strict::ask(&Request::RequestExit),
        Some(running) => {
            running.request_exit();
            Ok(())
        }
        None => Err("No session is running.".into()),
    }
}

/// Calls off a requested early end.
#[tauri::command]
fn cancel_exit(state: State<AppState>) -> Result<(), String> {
    match lock(&state.running).as_ref() {
        Some(running) if running.strict => strict::ask(&Request::CancelExit),
        Some(running) => {
            running.cancel_exit();
            Ok(())
        }
        None => Err("No session is running.".into()),
    }
}

#[tauri::command]
fn save_block_list(
    state: State<AppState>,
    config: BlockListConfig,
) -> Result<BlockListView, String> {
    // Editing the list mid-session would be a way around it.
    if lock(&state.running).is_some() {
        return Err("The block list is locked while a session is running.".into());
    }
    if config.mode == Mode::Strict && !strict::available() {
        return Err("Strict mode needs the Niwa service, which isn't installed.".into());
    }
    let store = lock(&state.store);
    let stored = block_list(&store)?;
    store
        .update_block_list(stored.id, &config)
        .map_err(|e| e.to_string())?;
    Ok(blocks::view(&stored.name, &config, &Installed::scan()))
}

/// What the Blocks screen's preview last looked with. It asks every few
/// seconds while the screen is open; keeping the watcher means only programs
/// started since the last look are inspected again.
struct Preview {
    config: BlockListConfig,
    /// When the installed games were last scanned for it.
    scanned: Instant,
    list: niwa_blocker::BlockList,
    watcher: niwa_blocker::Watcher,
}

static PREVIEW: Mutex<Option<Preview>> = Mutex::new(None);
/// How long a preview's scan of installed games is trusted.
const PREVIEW_RESCAN: Duration = Duration::from_secs(60);

/// The open apps a list would close if a session started now, by name. Only
/// looks: nothing is closed.
#[tauri::command]
async fn preview_closing(config: BlockListConfig) -> Vec<String> {
    let mut preview = lock(&PREVIEW);
    let stale = |preview: &Preview| preview.config != config || preview.scanned.elapsed() >= PREVIEW_RESCAN;
    if preview.as_ref().is_none_or(stale) {
        let (list, _) = Installed::scan().block_list(&config);
        *preview = Some(Preview {
            config,
            scanned: Instant::now(),
            list,
            watcher: niwa_blocker::Watcher::new(),
        });
    }
    let Some(Preview { list, watcher, .. }) = preview.as_mut() else {
        return Vec::new();
    };
    let found = watcher.poll(list).unwrap_or_default();
    let windowed = match found.iter().any(|process| process.needs_window) {
        true => niwa_blocker::app_window_pids(),
        false => Default::default(),
    };
    let me = niwa_blocker::process::current_owner();
    let mut labels: Vec<String> = found
        .into_iter()
        .filter(|process| {
            !process.needs_window
                || (windowed.contains(&process.pid) && me.is_some() && process.owner == me)
        })
        .map(|process| process.label)
        .collect();
    labels.sort_by_key(|label| label.to_lowercase());
    labels.dedup();
    labels
}

#[tauri::command]
async fn pick_app(window: WebviewWindow, allow: bool) -> Option<AppEntry> {
    let file = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title(if allow { "Choose an app to allow" } else { "Choose an app to block" })
        .add_filter("Programs", &["exe"])
        .blocking_pick_file()?;
    Some(blocks::app_entry(&file.into_path().ok()?))
}

#[tauri::command]
async fn pick_exception(window: WebviewWindow, folder: bool) -> Option<ExceptionEntry> {
    let dialog = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Choose what to always allow");
    let picked = match folder {
        true => dialog.blocking_pick_folder()?,
        false => dialog.add_filter("Programs", &["exe"]).blocking_pick_file()?,
    };
    Some(blocks::exception_entry(
        &picked.into_path().ok()?,
        &Installed::scan(),
    ))
}

/// Minimises the card: it hides and the mini timer takes its place.
#[tauri::command]
fn hide_window(app: AppHandle, window: WebviewWindow) {
    set_shown(&window, false);
    save_card_size(&app);
    mini::show(&app);
}

/// From the mini timer: bring the full card back.
#[tauri::command]
fn open_card(app: AppHandle) {
    show_window(&app);
}

/// From the mini timer: the user has started dragging it somewhere else.
#[tauri::command]
fn mini_drag_started() {
    mini::drag_started();
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    quit(&app);
}

/// Debug builds can run with no window at all (`NIWA_HEADLESS=1`), so
/// automated tests don't take focus from whatever is on screen.
pub fn headless() -> bool {
    cfg!(debug_assertions) && std::env::var_os("NIWA_HEADLESS").is_some()
}

/// Brings the card up without taking the keyboard: used when a session ends.
pub fn reveal_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        set_shown(&window, true);
    }
    mini::hide(app);
}

/// Shows or hides a window, and tells its page.
///
/// Hiding a window doesn't tell WebView2 on its own. The page would go on
/// drawing every frame at the monitor's refresh rate, swaying plant and all,
/// for a whole session in a window nobody can see. A page told it's hidden
/// stops drawing, pauses its animations and slows its timers.
///
/// Headless test runs keep windows off the screen, but their pages behave
/// as if shown.
pub fn set_shown(window: &WebviewWindow, shown: bool) {
    match window.label() {
        "main" => {
            CARD_SHOWN.store(shown, Ordering::SeqCst);
            // Leaving only counts once the user has been in the card again.
            CARD_USED.store(false, Ordering::SeqCst);
            if shown {
                *lock(&CARD_SHOWN_AT) = Some(Instant::now());
                restore_card(window);
            }
        }
        mini::LABEL => MINI_SHOWN.store(shown, Ordering::SeqCst),
        _ => {}
    }
    if shown {
        if !headless() {
            let _ = window.show();
        }
        sync_page(window);
        // And again once the window is surely up: a card shown with its page
        // still told to stop drawing is an empty, see-through window.
        let window = window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            sync_page(&window);
        });
    } else {
        let _ = window.hide();
        sync_page(window);
    }
}

/// Whether the mini timer is on screen (the card's is `CARD_SHOWN`).
static MINI_SHOWN: AtomicBool = AtomicBool::new(false);

/// Tells a window's page whether to draw: whether the window is meant to be
/// shown when this runs, not when it was asked for. Hiding and showing are
/// asked for from different threads (the card is tucked away from the
/// watcher's), and the requests reach WebView2 later on the main thread; a
/// stale "stop drawing" landing after a show left the card an empty,
/// see-through window (Oct 9, 2026). Drawing again, it's also told where its
/// window is, so it paints at once.
fn sync_page(window: &WebviewWindow) {
    let label = window.label().to_owned();
    let _ = window.with_webview(move |webview| {
        let wanted = match label.as_str() {
            "main" => CARD_SHOWN.load(Ordering::SeqCst),
            mini::LABEL => MINI_SHOWN.load(Ordering::SeqCst),
            _ => true,
        };
        // SAFETY: called on the main thread with the window's own live
        // controller, as WebView2 requires.
        unsafe {
            let controller = webview.controller();
            let _ = controller.SetIsVisible(wanted);
            if wanted {
                let _ = controller.NotifyParentWindowPositionChanged();
            }
        }
    });
}

/// Puts the card back whole before it's shown.
///
/// Windows can minimise the card (clicking its taskbar button while it's in
/// front, Win+D). Niwa then tucks it away, and a minimised window that is
/// hidden and shown again comes back at the size and spot Windows gives a
/// minimised one: a 160 by 28 sliver in a corner, so the card seemed never to
/// open (Oct 9, 2026). It's restored, then returned to where it last was.
fn restore_card(card: &WebviewWindow) {
    if card.is_minimized().unwrap_or(false) {
        let _ = card.unminimize();
    }
    let Some((position, size)) = *lock(&CARD_PLACE) else {
        // Not seen whole yet: at least make it full size, in the middle.
        let scale = card.scale_factor().unwrap_or(1.0);
        let small = card
            .outer_size()
            .is_ok_and(|size| size.to_logical::<f64>(scale).width < CARD_MIN.0 - 1.0);
        if small {
            let saved = card.app_handle().try_state::<AppState>().and_then(|state| saved_card_size(&lock(&state.store)));
            let _ = card.set_size(saved.unwrap_or(LogicalSize::new(CARD_MIN.0, CARD_MIN.1)));
            let _ = card.center();
        }
        return;
    };
    if card.outer_size().ok() != Some(size) {
        let _ = card.set_size(size);
    }
    if card.outer_position().ok() != Some(position) {
        let _ = card.set_position(position);
    }
}

/// Notes where the card is, when it's a whole card on screen.
fn remember_card_place(card: &tauri::Window) {
    let whole = card.is_visible().unwrap_or(false) && !card.is_minimized().unwrap_or(true);
    let scale = card.scale_factor().unwrap_or(1.0);
    if let (true, Ok(position), Ok(size)) = (whole, card.outer_position(), card.outer_size()) {
        let logical = size.to_logical::<f64>(scale);
        // A sliver isn't a card: the smallest card is CARD_MIN.
        if logical.width >= CARD_MIN.0 - 1.0 && logical.height >= CARD_MIN.1 - 1.0 {
            *lock(&CARD_PLACE) = Some((position, size));
        }
    }
}

/// Brings the card up and focuses it: used when the user asks for it.
pub fn show_window(app: &AppHandle) {
    reveal_window(app);
    if let Some(window) = app.get_webview_window("main").filter(|_| !headless()) {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Whether the card is on screen, and whether the user has been in Niwa
/// since it was shown (one of its windows was in front).
static CARD_SHOWN: AtomicBool = AtomicBool::new(false);
static CARD_USED: AtomicBool = AtomicBool::new(false);
/// When the card last appeared. For `SETTLE` after that, another program
/// taking the front isn't the user leaving: launching Niwa (from the
/// installer, Start, the tray) shuffles the front for a moment.
static CARD_SHOWN_AT: Mutex<Option<Instant>> = Mutex::new(None);
const SETTLE: Duration = Duration::from_secs(2);
/// How often the program in front is checked while the card is shown.
const FRONT_CHECK_EVERY: Duration = Duration::from_millis(250);

/// Tucks the card into the mini timer when the user leaves it for another
/// program, as its minimise button does.
///
/// It watches which program is in front rather than the card's focus events:
/// those also fire when Windows doesn't give a newly shown card the keyboard
/// at all (launched by the installer, say), which used to tuck the card away
/// before anyone saw it (Oct 6, 2026). Now the card only goes once a Niwa
/// window has been in front and then another program's is. Niwa's own
/// windows (a file picker, the tray menu) don't count as leaving, and nor
/// does Windows' shell: opening Niwa from Start, Search and the taskbar can
/// take the front just after the card does, which tucked it away again, and
/// right-clicking the card's taskbar button tucked it away before "Close
/// window" could reach it (both Oct 6, 2026). Niwa's installer and
/// uninstaller don't count either: clicking the installer's Finish button
/// after it opened Niwa tucked the new card away (Oct 6, 2026).
fn watch_for_leaving(app: AppHandle) {
    if headless() {
        return;
    }
    std::thread::spawn(move || {
        let ours = std::process::id();
        // The last other program in front, and whether it's one of Niwa's.
        let mut last_seen: Option<(u32, bool)> = None;
        let mut is_niwa = |pid: u32| match last_seen {
            Some((seen, niwa)) if seen == pid => niwa,
            _ => {
                let niwa = is_niwa_program(pid);
                last_seen = Some((pid, niwa));
                niwa
            }
        };
        loop {
            std::thread::sleep(FRONT_CHECK_EVERY);
            if !CARD_SHOWN.load(Ordering::SeqCst) {
                continue;
            }
            let settling = lock(&CARD_SHOWN_AT).is_some_and(|at| at.elapsed() < SETTLE);
            match niwa_blocker::process::foreground() {
                Some(front) if front.pid == ours => CARD_USED.store(true, Ordering::SeqCst),
                Some(front) if front.shell => {}
                // Not the user leaving: the card has only just appeared, or
                // Niwa's installer had the front and Windows is handing it on
                // as it closes. Leaving counts once they're back in the card.
                Some(front) if settling || is_niwa(front.pid) => CARD_USED.store(false, Ordering::SeqCst),
                Some(_) if CARD_USED.swap(false, Ordering::SeqCst) => {
                    if let Some(card) = app.get_webview_window("main") {
                        set_shown(&card, false);
                        save_card_size(&app);
                        mini::show(&app);
                    }
                }
                _ => {}
            }
        }
    });
}

/// Whether a program is one of Niwa's own, such as its installer, by the
/// product name in its version resource.
fn is_niwa_program(pid: u32) -> bool {
    niwa_blocker::process::image_path(pid)
        .and_then(|path| niwa_blocker::identity::file_identity(&path))
        .is_some_and(|identity| identity.product_name == "Niwa")
}

/// The card's saved size, kept within its smallest and largest.
fn saved_card_size(store: &Store) -> Option<LogicalSize<f64>> {
    let saved = store.setting(CARD_SIZE).ok()??;
    let (width, height) = saved.split_once(',')?;
    let (width, height): (f64, f64) = (width.trim().parse().ok()?, height.trim().parse().ok()?);
    Some(LogicalSize::new(
        width.clamp(CARD_MIN.0, CARD_MAX.0),
        height.clamp(CARD_MIN.1, CARD_MAX.1),
    ))
}

/// Writes the size the user resized the card to, if they did. Called when
/// the card hides and when the app quits, not on every step of a resize.
fn save_card_size(app: &AppHandle) {
    let Some((width, height)) = lock(&UNSAVED_CARD_SIZE).take() else {
        return;
    };
    if let Some(state) = app.try_state::<AppState>() {
        let _ = lock(&state.store).set_setting(CARD_SIZE, &format!("{width:.0},{height:.0}"));
    }
}

#[tauri::command]
fn get_stats(state: State<AppState>) -> Result<StatsView, String> {
    let today = Local::now().date_naive();
    // Far enough back for last week and for the whole of this month.
    let from = (stats::week_start(today) - chrono::Duration::days(7)).min(stats::month_start(today));
    let since = local_midnight(from);

    let store = lock(&state.store);
    let sessions: Vec<EndedSession> = store
        .ended_since(since)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(ended_at, outcome, planned_minutes, focused_seconds)| EndedSession {
            day: local_day(ended_at),
            outcome,
            planned_minutes,
            focused_seconds,
        })
        .collect();
    let slips: Vec<SlipOn> = store
        .slips_since(since)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(at, app)| SlipOn { day: local_day(at), app })
        .collect();
    let week = stats::week(&sessions, &slips, today);
    let (_, streak_days, _) = totals(&store);
    let total_minutes = store.blocked_minutes().map_err(|e| e.to_string())?;

    Ok(StatsView {
        days: week.days.to_vec(),
        today: week.today,
        week_minutes: week.minutes,
        change_percent: week.change_percent,
        streak_days,
        month_minutes: week.month_minutes,
        completion_percent: week.completion_percent,
        total_minutes,
        tempting: week
            .tempting
            .into_iter()
            .map(|(name, count)| TemptingView { name, count })
            .collect(),
    })
}

#[tauri::command]
fn get_settings(app: AppHandle, state: State<AppState>) -> SettingsView {
    let store = lock(&state.store);
    SettingsView {
        sound: flag(&store, SOUND, true),
        start_with_windows: app.autolaunch().is_enabled().unwrap_or(false),
        check_updates: flag(&store, update::SETTING, true),
    }
}

#[tauri::command]
fn set_check_updates(state: State<AppState>, on: bool) -> Result<(), String> {
    lock(&state.store)
        .set_setting(update::SETTING, &on.to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_schedules(state: State<AppState>) -> Result<Vec<Schedule>, String> {
    lock(&state.store).schedules().map_err(|e| e.to_string())
}

/// Adds or changes a schedule. A window of it already going on is left out:
/// saving shouldn't start blocking (or lock in a Strict session) there and
/// then. It starts from the next one.
#[tauri::command]
fn save_schedule(state: State<AppState>, schedule: Schedule) -> Result<Vec<Schedule>, String> {
    schedule.validate()?;
    let store = lock(&state.store);
    let id = store.save_schedule(&schedule).map_err(|e| e.to_string())?;
    if let Some(window) = (Schedule { id, ..schedule }).window_at(Local::now().naive_local()) {
        let ends = Local.from_local_datetime(&window.ends).earliest().map(|at| at.timestamp()).unwrap_or_default();
        let _ = store.set_schedule_handled_until(id, ends);
    }
    let schedules = store.schedules().map_err(|e| e.to_string())?;
    scheduler::share_with_service(schedules.clone());
    Ok(schedules)
}

#[tauri::command]
fn delete_schedule(state: State<AppState>, id: i64) -> Result<Vec<Schedule>, String> {
    let store = lock(&state.store);
    store.delete_schedule(id).map_err(|e| e.to_string())?;
    let schedules = store.schedules().map_err(|e| e.to_string())?;
    scheduler::share_with_service(schedules.clone());
    Ok(schedules)
}

/// A time of day as Windows shows it to the user ("9:00 AM", "09:00").
pub fn format_time(at: chrono::NaiveDateTime) -> String {
    use chrono::Timelike;
    niwa_blocker::clock::time_of_day(at.hour(), at.minute())
}

#[tauri::command]
fn get_update() -> Option<UpdateView> {
    update::found()
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    update::install(&app).await
}

#[tauri::command]
fn set_sound(state: State<AppState>, on: bool) -> Result<(), String> {
    lock(&state.store)
        .set_setting(SOUND, &on.to_string())
        .map_err(|e| e.to_string())
}

/// Starting with Windows opens Niwa to the mini timer. Only ever turned on
/// here, when the user switches it on in Settings.
#[tauri::command]
fn set_start_with_windows(app: AppHandle, on: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    match on {
        true => launcher.enable(),
        false => launcher.disable(),
    }
    .map_err(|e| e.to_string())
}

/// Quits the app. In Gentle mode a running session ends with it, so the
/// session is given up first and its plant withers.
pub fn quit(app: &AppHandle) {
    before_exit(app);
    let state = app.state::<AppState>();
    // A Strict session carries on in the service without the card.
    if lock(&state.running).as_ref().is_some_and(|running| running.strict) {
        app.exit(0);
        return;
    }
    if let Some(running) = lock(&state.running).as_ref() {
        running.give_up();
    }
    let runner = lock(&state.runner).take();
    // Wait for the session to be recorded before exiting, but not on this
    // thread: the session thread needs the main thread to finish up.
    let app = app.clone();
    std::thread::spawn(move || {
        if let Some(runner) = runner {
            let _ = runner.join();
        }
        app.exit(0);
    });
}

/// Keeps where the windows were left, before Niwa quits or updates.
pub fn before_exit(app: &AppHandle) {
    mini::save_position(app);
    save_card_size(app);
}

fn data_dir(app: &AppHandle) -> PathBuf {
    std::env::var_os("NIWA_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| app.path().local_data_dir().ok().map(|dir| dir.join("Niwa")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn open_store(app: &AppHandle) -> Result<Store, Box<dyn std::error::Error>> {
    let dir = data_dir(app);
    std::fs::create_dir_all(&dir)?;
    let store = Store::open(&dir.join("niwa.db"))?;
    if store.block_lists()?.is_empty() {
        store.add_block_list(
            blocks::DEFAULT_LIST_NAME,
            &Installed::scan().default_config(),
        )?;
    }
    Ok(store)
}

/// Deals with sessions that were still running when the app last closed.
///
/// A Gentle session ends with the app, so its plant withers. A Strict session
/// carried on in the service: the card picks it up again, and the follower
/// records how it ended if it already has.
fn pick_up_unfinished(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut unfinished = lock(&state.store).unfinished().unwrap_or_default();
    let strict = unfinished
        .iter()
        .rposition(|session| session.mode == Mode::Strict)
        .map(|index| unfinished.remove(index));

    for session in unfinished {
        let _ = lock(&state.store).finish_session(session.id, unix_now(), Outcome::Withered, 0);
    }

    if let Some(session) = strict {
        let blocking = block_list(&lock(&state.store))
            .map(|stored| Installed::scan().block_list(&stored.config).1)
            .unwrap_or_default();
        let (running, handle) = strict::follow(
            app.clone(),
            Followed {
                session_id: session.id,
                species: session.species,
                planned_minutes: session.planned_minutes,
                list_name: session.block_list,
                blocking,
                slips_recorded: session.slip_count as usize,
                // Corrected from the service's copy of the plan once it answers.
                scheduled: false,
            },
        );
        *lock(&state.running) = Some(running);
        *lock(&state.runner) = Some(handle);
    }
}

/// `niwa-app.exe --quit` asks a running Niwa to quit, as its tray menu's
/// Quit does, and like it not during a session. The installer uses it to
/// close Niwa before replacing it.
const QUIT_ARG: &str = "--quit";

/// Whether another Niwa is running here: the same test the single-instance
/// plugin makes, its message window (`{identifier}-sic`, `{identifier}-siw`).
/// A process that has exited but is still listed has no window, so it
/// doesn't count.
fn another_card_running() -> bool {
    niwa_blocker::process::window_exists(&format!("{APP_ID}-sic"), &format!("{APP_ID}-siw"))
}

/// `identifier` in `tauri.conf.json`.
const APP_ID: &str = "app.niwa.desktop";

/// WebView2's switches, for the card (`additionalBrowserArgs` in
/// `tauri.conf.json`) and the mini timer alike: windows sharing WebView2
/// must ask for the same ones. The first is Tauri's own default. Then both
/// pages share one page process, and drawing runs inside WebView2's main
/// process rather than a GPU process of its own: together about a quarter
/// less memory, 186 MB against 242 MB and six processes instead of eight
/// (measured Oct 17, 2026).
pub const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
                                --renderer-process-limit=1 --in-process-gpu";

#[cfg(test)]
mod tests {
    fn conf() -> serde_json::Value {
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap()
    }

    #[test]
    fn app_id_is_the_configs() {
        assert_eq!(conf()["identifier"], super::APP_ID);
    }

    #[test]
    fn card_and_mini_timer_ask_webview2_for_the_same() {
        assert_eq!(conf()["app"]["windows"][0]["additionalBrowserArgs"], super::BROWSER_ARGS);
    }
}

fn main() {
    // Asked to quit with no Niwa running, there's nothing to do. Leaving
    // before anything starts matters: the installer asks from an admin
    // process, and an admin WebView2 would leave files in the user's
    // WebView2 folder that their own Niwa then couldn't open.
    if std::env::args().any(|arg| arg == QUIT_ARG) && !another_card_running() {
        return;
    }
    let mut builder = tauri::Builder::default();
    // A debug test run with its own data folder is a separate copy: it
    // mustn't hand itself over to (or take over from) the Niwa in use.
    if !(cfg!(debug_assertions) && std::env::var_os("NIWA_DATA_DIR").is_some()) {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Mid-session, quitting would end the session; the card comes up
            // instead and the installer says why it stopped.
            let session = app
                .try_state::<AppState>()
                .is_some_and(|state| lock(&state.running).is_some());
            match args.iter().any(|arg| arg == QUIT_ARG) && !session {
                true => quit(app),
                false => show_window(app),
            }
        }));
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--tray"]),
        ))
        .setup(|app| {
            // Asked to quit with no Niwa running: there's nothing to do.
            if std::env::args().any(|arg| arg == QUIT_ARG) {
                std::process::exit(0);
            }
            let store = open_store(app.handle())?;
            let card_size = saved_card_size(&store);
            // The state goes in before any window is made. Making one waits
            // on WebView2 while still handling messages, and the card's page
            // can load and ask for its overview in that time: before this,
            // that request failed and the card stayed blank (seen Oct 5,
            // 2026, just after the PC woke).
            app.manage(AppState {
                store: Mutex::new(store),
                running: Mutex::new(None),
                runner: Mutex::new(None),
                tray: Mutex::new(None),
            });
            if let (Some(window), Some(size)) = (app.get_webview_window("main"), card_size) {
                window.set_size(size)?;
                window.center()?;
            }
            mini::create(app)?;
            // Both windows start hidden; so do their pages until one is shown.
            for window in app.webview_windows().values() {
                sync_page(window);
            }
            // The card starts hidden. `--tray` (used when starting with
            // Windows) starts with the mini timer instead.
            if std::env::args().any(|arg| arg == "--tray") {
                mini::show(app.handle());
            } else {
                show_window(app.handle());
            }
            let tray = tray::build(app)?;
            *lock(&app.state::<AppState>().tray) = Some(tray);
            pick_up_unfinished(app.handle());
            watch_for_leaving(app.handle().clone());
            update::start(app.handle());
            scheduler::start(app.handle().clone());
            if let Ok(schedules) = lock(&app.state::<AppState>().store).schedules() {
                scheduler::share_with_service(schedules);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // A window the user is in must be drawing.
            if let WindowEvent::Focused(true) = event {
                if let Some(webview) = window.app_handle().get_webview_window(window.label()) {
                    sync_page(&webview);
                }
            }
            if let (WindowEvent::Moved(position), mini::LABEL) = (event, window.label()) {
                mini::moved(*position);
            }
            if let (WindowEvent::Moved(_) | WindowEvent::Resized(_), "main") = (event, window.label()) {
                remember_card_place(window);
            }
            if let (WindowEvent::Resized(size), "main") = (event, window.label()) {
                // A hidden or minimised window reports no size; that isn't one to keep.
                if size.width > 0 && size.height > 0 {
                    let logical = size.to_logical::<f64>(window.scale_factor().unwrap_or(1.0));
                    *lock(&UNSAVED_CARD_SIZE) = Some((logical.width, logical.height));
                }
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                // The mini timer isn't closed on its own; it goes when the card opens.
                if window.label() == mini::LABEL {
                    return;
                }
                // Closing the card some other way (Alt+F4) goes through the
                // card, so a running session gets its confirmation.
                let app = window.app_handle();
                let running = lock(&app.state::<AppState>().running).is_some();
                match running {
                    true => {
                        let _ = window.emit("close-requested", ());
                    }
                    false => quit(app),
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_overview,
            get_garden,
            start_session,
            request_exit,
            cancel_exit,
            save_block_list,
            preview_closing,
            pick_app,
            pick_exception,
            hide_window,
            open_card,
            get_stats,
            get_settings,
            set_sound,
            set_start_with_windows,
            set_check_updates,
            get_update,
            install_update,
            get_schedules,
            save_schedule,
            delete_schedule,
            mini_drag_started,
            quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Niwa");
}
