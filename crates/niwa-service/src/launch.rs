//! Starts the card for the logged-in user if it isn't running.
//!
//! During a Strict session the card and the service keep each other alive.
//! The service runs as SYSTEM in its own session, so starting a program on
//! the user's desktop means borrowing that user's token.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use niwa_blocker::process::{image_path, list_processes, token_sid};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows::Win32::System::RemoteDesktop::{WTSGetActiveConsoleSessionId, WTSQueryUserToken};
use windows::Win32::System::Threading::{
    CreateProcessAsUserW, CREATE_UNICODE_ENVIRONMENT, PROCESS_INFORMATION, STARTUPINFOW,
};

const CARD_EXE: &str = "niwa-app.exe";

/// The card is the `niwa-app.exe` installed beside the service. Nothing else
/// is ever launched, whatever a client asks for.
pub fn card_path() -> Option<PathBuf> {
    let card = std::env::current_exe().ok()?.with_file_name(CARD_EXE);
    card.is_file().then_some(card)
}

fn is_running(card: &Path) -> bool {
    let wanted = card.to_string_lossy().to_lowercase();
    list_processes().unwrap_or_default().iter().any(|process| {
        process.exe_name.eq_ignore_ascii_case(CARD_EXE)
            && image_path(process.pid)
                .is_some_and(|path| path.to_string_lossy().to_lowercase() == wanted)
    })
}

struct Token(HANDLE);

impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful WTSQueryUserToken.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Brings the card back on screen if the session's owner is the one
/// logged in at the screen and the card isn't already running.
pub fn ensure_card_running(user_sid: &str) -> windows::core::Result<()> {
    let Some(card) = card_path() else {
        return Ok(());
    };
    if is_running(&card) {
        return Ok(());
    }

    // SAFETY: plain calls; the token is owned and closed by `Token`.
    let token = unsafe {
        let session = WTSGetActiveConsoleSessionId();
        if session == u32::MAX {
            // Nobody is logged in at the screen.
            return Ok(());
        }
        let mut token = HANDLE::default();
        WTSQueryUserToken(session, &mut token)?;
        Token(token)
    };
    // Someone else is at the screen: it isn't their session to be shown.
    if token_sid(token.0).as_deref() != Some(user_sid) {
        return Ok(());
    }

    // Started without `--tray`, so it shows: someone who ended the card mid-session
    // should see that the session is still on.
    let mut command: Vec<u16> = format!("\"{}\"", card.display())
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let folder: Vec<u16> = card
        .parent()
        .unwrap_or(Path::new("."))
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut desktop: Vec<u16> = "winsta0\\default\0".encode_utf16().collect();
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        lpDesktop: PWSTR(desktop.as_mut_ptr()),
        ..Default::default()
    };
    let mut started = PROCESS_INFORMATION::default();
    let mut environment: *mut c_void = std::ptr::null_mut();

    // SAFETY: every buffer is NUL-terminated and outlives the call; the
    // environment block and the returned handles are released below.
    unsafe {
        CreateEnvironmentBlock(&mut environment, Some(token.0), false)?;
        let created = CreateProcessAsUserW(
            Some(token.0),
            PCWSTR::null(),
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_UNICODE_ENVIRONMENT,
            Some(environment),
            PCWSTR(folder.as_ptr()),
            &startup,
            &mut started,
        );
        let _ = DestroyEnvironmentBlock(environment);
        created?;
        let _ = CloseHandle(started.hProcess);
        let _ = CloseHandle(started.hThread);
    }
    Ok(())
}
