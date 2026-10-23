//! Installs a newer Niwa for the card, so updates don't ask for admin
//! (approved Oct 6, 2026).
//!
//! The service already has the rights the installer needs, so the card hands
//! it the installer it downloaded instead of running it itself. The service
//! reads the file, checks it with `niwa_core::update::check_installer` (signed
//! with Niwa's update key, for the version asked, newer than this service),
//! copies it into its own folder, which only SYSTEM can write to, and runs
//! that copy silently. Nothing else is ever run this way.
//!
//! The installer stops and replaces this service. Before running it, the
//! service notes whose card to reopen; the new service reads that note when
//! it starts and brings the card back on that user's screen.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use niwa_core::update::{check_installer, MAX_INSTALLER_BYTES};

/// Inside the state folder: where the checked installer is copied.
const UPDATES: &str = "updates";
/// Inside the state folder: whose card to reopen after an update, and when.
const REOPEN: &str = "reopen-card.txt";
/// A note older than this is from an update that never finished; it's ignored.
const REOPEN_WITHIN_SECS: i64 = 15 * 60;

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default()
}

/// Checks the installer at `path` and starts it, silently, as an update.
/// `user_sid` is the account whose card asked. With `run` false (the engine
/// in a console, for development) the installer is checked and copied but
/// never run.
pub fn install(state_dir: &Path, path: &str, signature: &str, version: &str, user_sid: &str, run: bool) -> Result<(), String> {
    let path = Path::new(path);
    let size = fs::metadata(path)
        .ok()
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .ok_or("The downloaded update couldn't be found.")?;
    if size > MAX_INSTALLER_BYTES {
        return Err("That update is far too large to be Niwa.".into());
    }
    let bytes = fs::read(path).map_err(|_| "The downloaded update couldn't be read.".to_string())?;
    // Checked once it's in memory: what was checked is what gets run.
    check_installer(&bytes, signature, version, env!("CARGO_PKG_VERSION"))?;

    // `check_installer` has made sure `version` is plain digits and dots, so
    // it is safe in a file name.
    let dir = state_dir.join(UPDATES);
    let _ = fs::remove_dir_all(&dir);
    let installer = dir.join(format!("Niwa_{version}_x64-setup.exe"));
    fs::create_dir_all(&dir)
        .and_then(|()| fs::write(&installer, &bytes))
        .map_err(|e| format!("The update couldn't be saved: {e}"))?;
    if !run {
        return Err(format!("Test run: Niwa {version} checked and saved, not installed."));
    }

    fs::write(state_dir.join(REOPEN), format!("{user_sid}\n{}\n", unix_now()))
        .map_err(|e| format!("The update couldn't be started: {e}"))?;
    // `/S` runs it silently; `/UPDATE` keeps the user's shortcuts and data.
    // Not `/R`: the installer would start the card as SYSTEM. The new
    // service reopens it as the user instead.
    let mut child = Command::new(&installer)
        .args(["/S", "/UPDATE"])
        .current_dir(&dir)
        .spawn()
        .map_err(|e| format!("The update couldn't be started: {e}"))?;
    crate::log(&format!("installing Niwa {version}"));
    // If the installer gets as far as replacing this service, this process
    // is gone before it returns. If it's still here when the installer ends,
    // the update stopped early: bring the card back, since it has closed.
    let user_sid = user_sid.to_owned();
    let reopen = state_dir.join(REOPEN);
    std::thread::spawn(move || {
        let status = child.wait();
        crate::log(&format!("the Niwa installer ended early: {status:?}"));
        let _ = fs::remove_file(reopen);
        let _ = crate::launch::ensure_card_running(&user_sid);
    });
    Ok(())
}

/// After an update: reopens the card for the user who asked for it, if the
/// note is recent, and clears the note and the copied installer.
pub fn reopen_card(state_dir: &Path) {
    let note = state_dir.join(REOPEN);
    let Ok(text) = fs::read_to_string(&note) else {
        return;
    };
    let _ = fs::remove_file(&note);
    // The installer that started this service may still be running from
    // here; if so, it's cleared after the next update instead.
    let _ = fs::remove_dir_all(state_dir.join(UPDATES));
    let mut lines = text.lines();
    let (Some(user_sid), Some(at)) = (lines.next(), lines.next().and_then(|at| at.parse::<i64>().ok())) else {
        return;
    };
    if (0..REOPEN_WITHIN_SECS).contains(&(unix_now() - at)) {
        if let Err(e) = crate::launch::ensure_card_running(user_sid) {
            crate::log(&format!("couldn't reopen the card after an update: {e}"));
        }
    }
}
