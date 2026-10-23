//! Updates: the one time Niwa goes online (approved Oct 6, 2026).
//!
//! Once a day, unless turned off in Settings, Niwa reads a small file in its
//! latest GitHub release that names the latest version. Nothing about the user is sent.
//! When there's a newer one the card offers it, and only outside a session.
//! Installing downloads the new installer and checks its signature against
//! the key built into Niwa. Then the Strict service installs it, checking it
//! again first, so Windows doesn't ask for admin; the new service reopens the
//! card. With no service, or one too old to do that, the card runs the
//! installer itself and Windows asks for admin, since the installer also
//! replaces the service.
//!
//! The address is `plugins.updater.endpoints` in `tauri.conf.json`. Were
//! it empty, Niwa would never check. Debug builds don't check either, unless
//! `NIWA_UPDATE_FEED` names a test feed.

use std::sync::Mutex;
use std::time::Duration;

use niwa_core::ipc::Request;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, Updater, UpdaterExt};

use crate::views::UpdateView;
use crate::{flag, lock, unix_now, AppState};

/// Whether Niwa looks for updates. On unless turned off.
pub const SETTING: &str = "check_updates";
/// When it last looked, as Unix time.
const LAST_CHECK: &str = "last_update_check";
const CHECK_EVERY: i64 = 24 * 60 * 60;
/// The first look waits a little, so it doesn't slow Niwa's start.
const FIRST_LOOK: Duration = Duration::from_secs(60);
/// How often a running Niwa sees whether a day has passed.
const LOOK_EVERY: Duration = Duration::from_secs(60 * 60);

/// The newer version found, waiting for the user.
static FOUND: Mutex<Option<Update>> = Mutex::new(None);

/// A debug build's test feed (`NIWA_UPDATE_FEED`), which may be plain http.
fn test_feed() -> Option<tauri::Url> {
    cfg!(debug_assertions)
        .then(|| std::env::var("NIWA_UPDATE_FEED").ok()?.parse().ok())
        .flatten()
}

fn updater(app: &AppHandle) -> Option<Updater> {
    let mut builder = app.updater_builder();
    if let Some(feed) = test_feed() {
        builder = builder.endpoints(vec![feed]).ok()?;
    } else if cfg!(debug_assertions) {
        return None;
    }
    let handle = app.clone();
    // Runs just before the installer starts and Niwa exits.
    builder = builder.on_before_exit(move || crate::before_exit(&handle));
    // An empty address list fails here: updates aren't set up yet.
    builder.build().ok()
}

/// Starts the daily look for updates.
pub fn start(app: &AppHandle) {
    if updater(app).is_none() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let first = match test_feed() {
            Some(_) => Duration::from_secs(5),
            None => FIRST_LOOK,
        };
        std::thread::sleep(first);
        loop {
            if due(&app) {
                tauri::async_runtime::block_on(look(&app));
            }
            std::thread::sleep(LOOK_EVERY);
        }
    });
}

fn due(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let store = lock(&state.store);
    if !flag(&store, SETTING, true) {
        return false;
    }
    let last: i64 = store
        .setting(LAST_CHECK)
        .ok()
        .flatten()
        .and_then(|last| last.parse().ok())
        .unwrap_or(0);
    test_feed().is_some() || unix_now() - last >= CHECK_EVERY
}

async fn look(app: &AppHandle) {
    let Some(updater) = updater(app) else { return };
    // A failed look (offline, say) is tried again in an hour.
    let Ok(found) = updater.check().await else { return };
    let state = app.state::<AppState>();
    let _ = lock(&state.store).set_setting(LAST_CHECK, &unix_now().to_string());
    if let Some(update) = found {
        let view = view_of(&update);
        *lock(&FOUND) = Some(update);
        let _ = app.emit_to("main", "update-ready", view);
    }
}

fn view_of(update: &Update) -> UpdateView {
    UpdateView {
        version: update.version.clone(),
        asks_admin: !crate::strict::up_to_date(),
    }
}

/// The update waiting for the user, if there is one.
pub fn found() -> Option<UpdateView> {
    let update = lock(&FOUND).clone()?;
    Some(view_of(&update))
}

/// Downloads and installs the waiting update. On success Niwa exits and the
/// installer takes over, so this only returns with why it couldn't.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    if lock(&app.state::<AppState>().running).is_some() {
        return Err("Niwa can update once this session ends.".into());
    }
    let Some(update) = lock(&FOUND).clone() else {
        return Err("There's no update waiting.".into());
    };
    let bytes = update.download(|_, _| {}, || {}).await.map_err(|error| {
        use tauri_plugin_updater::Error::*;
        match error {
            // Not signed with Niwa's key, or not for the version announced.
            Minisign(_) | SignatureUtf8(_) | Base64(_) | SignedVersionMismatch { .. } | MissingSignedVersion => {
                "The update didn't pass Niwa's safety check, so it wasn't installed.".to_string()
            }
            _ => "The update couldn't be downloaded. Try again later.".to_string(),
        }
    })?;
    if crate::strict::up_to_date() {
        return through_service(app, &update, &bytes);
    }
    // A debug test run stops here: running the installer would install the service.
    if test_feed().is_some() {
        return Err(format!("Test run: {} downloaded and verified, not installed.", update.version));
    }
    update
        .install(bytes)
        .map_err(|_| "The update didn't start. Try again.".to_string())
}

/// Hands the downloaded installer to the Strict service, which checks it
/// again and runs it without asking for admin. On success Niwa exits: the
/// installer replaces it, and the new service opens it again.
fn through_service(app: &AppHandle, update: &Update, bytes: &[u8]) -> Result<(), String> {
    let file = std::env::temp_dir().join(format!("niwa-update-{}.exe", std::process::id()));
    std::fs::write(&file, bytes).map_err(|_| "The update couldn't be saved. Try again.".to_string())?;
    let asked = crate::strict::ask(&Request::InstallUpdate {
        installer: file.to_string_lossy().into_owned(),
        signature: update.signature.clone(),
        version: update.version.clone(),
    });
    // The service has its own copy by the time it answers.
    let _ = std::fs::remove_file(&file);
    asked?;
    crate::before_exit(app);
    app.exit(0);
    Ok(())
}
