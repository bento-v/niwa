//! Lists running processes and closes them.
//!
//! Everything here is ordinary process management: a snapshot of the process
//! list, a window close request and `TerminateProcess`. Nothing is injected
//! into, hooked in, or read from another process.

use std::collections::HashSet;
use std::ffi::{c_void, OsString};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use windows::core::{BOOL, PWSTR};
use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessInformation};
use windows::Win32::Foundation::{
    CloseHandle, LocalFree, ERROR_INVALID_PARAMETER, HANDLE, HLOCAL, HWND, LPARAM,
    STATUS_INFO_LENGTH_MISMATCH, WAIT_OBJECT_0, WPARAM,
};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::WindowsProgramming::SYSTEM_PROCESS_INFORMATION;
use windows::Win32::System::Threading::{
    OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
    PROCESS_ACCESS_RIGHTS, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, FindWindowW, GetClassNameW, GetForegroundWindow, GetWindow,
    GetWindowLongW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, GWL_EXSTYLE, GW_OWNER,
    WM_CLOSE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

use crate::blocklist::is_protected;

/// A row of the process list. Cheap to get; has no full path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProcessEntry {
    pub pid: u32,
    pub parent_pid: u32,
    pub exe_name: String,
    /// When the process started, in 100 ns ticks. With the pid, this tells a
    /// process apart from a later one that reuses its pid.
    pub created: i64,
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful Win32 call and is closed once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn open(pid: u32, access: PROCESS_ACCESS_RIGHTS) -> windows::core::Result<OwnedHandle> {
    // SAFETY: plain FFI call; the returned handle is owned by `OwnedHandle`.
    unsafe { OpenProcess(access, false, pid) }.map(OwnedHandle)
}

/// Memory for `snapshot` to reuse. The process list runs to several hundred
/// kilobytes, and a poll twice a second shouldn't ask Windows for it afresh.
#[derive(Default)]
pub struct SnapshotBuffer(Vec<u64>);

/// Takes a snapshot of every running process.
pub fn list_processes() -> windows::core::Result<Vec<ProcessEntry>> {
    snapshot(&mut SnapshotBuffer::default())
}

/// Takes a snapshot of every running process, reusing `buffer`.
///
/// This asks the kernel for the process list directly, which is what the
/// ToolHelp snapshot does underneath, at about half the cost.
pub fn snapshot(buffer: &mut SnapshotBuffer) -> windows::core::Result<Vec<ProcessEntry>> {
    let buffer = &mut buffer.0;
    if buffer.is_empty() {
        buffer.resize(512 * 1024 / 8, 0);
    }
    let mut filled = 0u32;
    for _ in 0..8 {
        // SAFETY: the buffer is valid for the length given, in bytes.
        let status = unsafe {
            NtQuerySystemInformation(
                SystemProcessInformation,
                buffer.as_mut_ptr().cast(),
                (buffer.len() * 8) as u32,
                &mut filled,
            )
        };
        if status != STATUS_INFO_LENGTH_MISMATCH {
            status.ok()?;
            break;
        }
        // Processes can start between asking and filling: leave headroom.
        buffer.resize((filled as usize).div_ceil(8) + 64 * 1024 / 8, 0);
        filled = 0;
    }
    if filled == 0 {
        return Err(STATUS_INFO_LENGTH_MISMATCH.into());
    }

    let bytes = buffer.as_ptr().cast::<u8>();
    let end = (filled as usize).min(buffer.len() * 8);
    let mut processes = Vec::with_capacity(512);
    let mut offset = 0usize;
    while offset + size_of::<SYSTEM_PROCESS_INFORMATION>() <= end {
        // SAFETY: the entry lies within the filled part of the buffer.
        let entry: SYSTEM_PROCESS_INFORMATION = unsafe { bytes.add(offset).cast::<SYSTEM_PROCESS_INFORMATION>().read_unaligned() };
        let name = match entry.ImageName.Buffer.is_null() {
            true => String::new(),
            // SAFETY: the name points into the same buffer, `Length` bytes long.
            false => String::from_utf16_lossy(unsafe {
                std::slice::from_raw_parts(entry.ImageName.Buffer.0, entry.ImageName.Length as usize / 2)
            }),
        };
        // The documented structure leaves these fields unnamed: the start
        // time sits 24 bytes into `Reserved1`, and `Reserved2` holds the
        // parent's pid (InheritedFromUniqueProcessId).
        let created = i64::from_le_bytes(entry.Reserved1[24..32].try_into().unwrap_or_default());
        processes.push(ProcessEntry {
            pid: entry.UniqueProcessId.0 as usize as u32,
            parent_pid: entry.Reserved2 as usize as u32,
            exe_name: name,
            created,
        });
        if entry.NextEntryOffset == 0 {
            break;
        }
        offset += entry.NextEntryOffset as usize;
    }
    Ok(processes)
}

fn image_path_of(process: &OwnedHandle) -> Option<PathBuf> {
    let mut buffer = vec![0u16; 1024];
    let mut len = buffer.len() as u32;
    // SAFETY: `buffer` is valid for `len` UTF-16 units; the call writes at most that.
    unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )
        .ok()?;
    }
    Some(PathBuf::from(OsString::from_wide(&buffer[..len as usize])))
}

/// The full path of a process's executable, or `None` if the process has
/// exited or Windows won't let us look (some system processes).
pub fn image_path(pid: u32) -> Option<PathBuf> {
    image_path_of(&open(pid, PROCESS_QUERY_LIMITED_INFORMATION).ok()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseOutcome {
    /// The app closed itself after being asked.
    ClosedWhenAsked,
    /// The app didn't close in time (or had no window to ask) and was ended.
    Terminated,
    /// The process had already exited.
    AlreadyGone,
    /// The pid now belongs to a different program; nothing was done.
    NotTheSameProgram,
    /// Windows refused, for example because the process runs elevated or is
    /// shielded by anti-cheat.
    Failed(String),
}

impl CloseOutcome {
    pub fn is_closed(&self) -> bool {
        matches!(
            self,
            CloseOutcome::ClosedWhenAsked | CloseOutcome::Terminated | CloseOutcome::AlreadyGone
        )
    }
}

/// Closes a process: asks its windows to close, waits up to `patience`, then
/// terminates it if it is still running.
///
/// `expected_image` is the path the block list matched. It is checked again
/// here so a recycled pid can never get the wrong program closed.
pub fn close_process(pid: u32, expected_image: &Path, patience: Duration) -> CloseOutcome {
    if is_protected(expected_image) {
        return CloseOutcome::Failed("protected process".into());
    }

    let watch = match open(pid, PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE) {
        Ok(handle) => Some(handle),
        Err(e) if e.code() == ERROR_INVALID_PARAMETER.to_hresult() => {
            return CloseOutcome::AlreadyGone
        }
        Err(_) => None,
    };

    if let Some(watch) = &watch {
        if has_exited(watch, Duration::ZERO) {
            return CloseOutcome::AlreadyGone;
        }
        match image_path_of(watch) {
            Some(actual) if same_path(&actual, expected_image) => {}
            _ => return CloseOutcome::NotTheSameProgram,
        }
    }

    let windows = visible_windows(pid);
    for &window in &windows {
        // SAFETY: posting a message to a window handle; a stale handle just fails.
        unsafe {
            let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
    if let (false, Some(watch)) = (windows.is_empty(), &watch) {
        if has_exited(watch, patience) {
            return CloseOutcome::ClosedWhenAsked;
        }
    }

    let terminate = match open(pid, PROCESS_TERMINATE) {
        Ok(handle) => handle,
        Err(e) if e.code() == ERROR_INVALID_PARAMETER.to_hresult() => {
            return CloseOutcome::AlreadyGone
        }
        Err(e) => return CloseOutcome::Failed(e.message()),
    };
    // SAFETY: `terminate` is a live handle opened with PROCESS_TERMINATE.
    match unsafe { TerminateProcess(terminate.0, 1) } {
        Ok(()) => CloseOutcome::Terminated,
        Err(e) => match &watch {
            // Lost a race with the app closing on its own.
            Some(watch) if has_exited(watch, Duration::ZERO) => CloseOutcome::ClosedWhenAsked,
            _ => CloseOutcome::Failed(e.message()),
        },
    }
}

/// Asks a process's windows to close, without ending it. Returns whether it
/// had any windows to ask.
///
/// The Strict service can't do this itself, because a service doesn't share
/// the user's desktop, so the card does it on the service's behalf before
/// the service terminates the process.
pub fn ask_to_close(pid: u32, expected_image: &Path) -> bool {
    if is_protected(expected_image) || image_path(pid).is_none_or(|actual| !same_path(&actual, expected_image)) {
        return false;
    }
    let windows = visible_windows(pid);
    for &window in &windows {
        // SAFETY: posting a message to a window handle; a stale handle just fails.
        unsafe {
            let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
    !windows.is_empty()
}

/// Who a process runs as, and in which login session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    /// The account's security identifier, as text ("S-1-5-21-...").
    pub sid: String,
    /// The Windows session. Session 0 holds services, never a user's programs.
    pub session: u32,
}

/// Looks up a process's owner, or `None` if it has exited or can't be read.
pub fn owner(pid: u32) -> Option<Owner> {
    let process = open(pid, PROCESS_QUERY_LIMITED_INFORMATION).ok()?;
    let mut token = HANDLE::default();
    // SAFETY: `process` is a live handle; the token handle is owned below.
    unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut token).ok()? };
    let token = OwnedHandle(token);
    let sid = token_sid(token.0)?;

    let mut session = 0u32;
    // SAFETY: `session` is a valid out pointer.
    unsafe { ProcessIdToSessionId(pid, &mut session).ok()? };
    Some(Owner { sid, session })
}

/// The account a token belongs to, as SID text. `token` must be open for
/// TOKEN_QUERY.
pub fn token_sid(token: HANDLE) -> Option<String> {
    let mut needed = 0u32;
    // SAFETY: a first call with no buffer reports the size needed.
    unsafe {
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
    }
    // u64 elements keep the buffer aligned for the structure read from it.
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8).max(1)];
    let mut sid_text = PWSTR::null();
    // SAFETY: the buffer is at least `needed` bytes and aligned; after a
    // successful call it starts with a TOKEN_USER whose SID points into it.
    unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr() as *mut c_void),
            needed,
            &mut needed,
        )
        .ok()?;
        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        ConvertSidToStringSidW(user.User.Sid, &mut sid_text).ok()?;
        let sid = sid_text.to_string().ok();
        let _ = LocalFree(Some(HLOCAL(sid_text.0 as *mut c_void)));
        sid
    }
}

/// The owner of the process this code is running in.
pub fn current_owner() -> Option<Owner> {
    owner(std::process::id())
}

fn has_exited(process: &OwnedHandle, wait: Duration) -> bool {
    // SAFETY: the handle was opened with SYNCHRONIZE.
    unsafe { WaitForSingleObject(process.0, wait.as_millis() as u32) == WAIT_OBJECT_0 }
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

struct WindowSearch {
    pid: u32,
    found: Vec<HWND>,
}

/// The visible top-level windows that belong to a process.
fn visible_windows(pid: u32) -> Vec<HWND> {
    unsafe extern "system" fn visit(window: HWND, param: LPARAM) -> BOOL {
        // SAFETY: `param` is the `WindowSearch` passed to `EnumWindows` below,
        // which outlives the enumeration.
        let search = &mut *(param.0 as *mut WindowSearch);
        let mut owner = 0u32;
        GetWindowThreadProcessId(window, Some(&mut owner));
        if owner == search.pid && IsWindowVisible(window).as_bool() {
            search.found.push(window);
        }
        true.into()
    }

    let mut search = WindowSearch {
        pid,
        found: Vec::new(),
    };
    // SAFETY: `search` lives until `EnumWindows` returns.
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut search as *mut WindowSearch as isize));
    }
    search.found
}

/// The processes that show a window on the taskbar right now: visible,
/// top-level, not owned by another window and not a tool window. Tray icons,
/// overlays, desktop widgets and background helpers aren't included.
///
/// Microsoft Store apps draw inside a frame that belongs to Windows'
/// ApplicationFrameHost; the app inside the frame counts as well.
pub fn app_window_pids() -> HashSet<u32> {
    unsafe extern "system" fn visit(window: HWND, param: LPARAM) -> BOOL {
        // SAFETY: `param` is the Vec passed to `EnumWindows` below, which
        // outlives the enumeration.
        let found = &mut *(param.0 as *mut Vec<(HWND, u32)>);
        if is_app_window(window) {
            let mut pid = 0u32;
            GetWindowThreadProcessId(window, Some(&mut pid));
            found.push((window, pid));
        }
        true.into()
    }

    let mut found: Vec<(HWND, u32)> = Vec::new();
    // SAFETY: `found` lives until `EnumWindows` returns.
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut found as *mut Vec<(HWND, u32)> as isize));
    }

    let mut pids: HashSet<u32> = found.iter().map(|&(_, pid)| pid).collect();
    let frame_hosts: HashSet<u32> = pids.iter().copied().filter(|&pid| is_frame_host(pid)).collect();
    for &(window, pid) in &found {
        if frame_hosts.contains(&pid) {
            pids.extend(child_window_pids(window).into_iter().filter(|&child| child != pid));
        }
    }
    pids
}

/// Whether a top-level window with this class and title exists on this
/// desktop. Only a live process can have one.
pub fn window_exists(class: &str, title: &str) -> bool {
    // SAFETY: both strings are NUL-terminated and outlive the call.
    unsafe { FindWindowW(&windows::core::HSTRING::from(class), &windows::core::HSTRING::from(title)) }
        .is_ok_and(|window| !window.is_invalid())
}

/// The window that has the keyboard right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Foreground {
    pub pid: u32,
    /// Part of Windows' own shell rather than a program: the taskbar, the
    /// Start menu, Search, the tray's overflow, Alt+Tab. These take the front
    /// on the way to somewhere else, or for a moment while a program opens.
    pub shell: bool,
}

/// The window that has the keyboard right now, or `None` when no window
/// does (for a moment while focus moves).
pub fn foreground() -> Option<Foreground> {
    // SAFETY: these calls only read the window manager's state.
    let (pid, class) = unsafe {
        let window = GetForegroundWindow();
        if window.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, Some(&mut pid));
        let mut class = [0u16; 256];
        let len = GetClassNameW(window, &mut class).max(0) as usize;
        (pid, String::from_utf16_lossy(&class[..len]))
    };
    if pid == 0 {
        return None;
    }
    let shell = image_path(pid).is_some_and(|image| is_shell_window(&image, &class));
    Some(Foreground { pid, shell })
}

/// Windows' shell processes. Their windows are shell UI, except File
/// Explorer's own windows and the desktop, which the user goes to.
const SHELL_PROCESSES: [&str; 4] = [
    "explorer.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
];
const NOT_SHELL_UI: [&str; 4] = ["CabinetWClass", "ExploreWClass", "Progman", "WorkerW"];

fn is_shell_window(image: &Path, class: &str) -> bool {
    let in_windows = std::env::var_os("SystemRoot").is_some_and(|root| {
        image.to_string_lossy().to_lowercase().starts_with(&root.to_string_lossy().to_lowercase())
    });
    let name = image
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    in_windows && SHELL_PROCESSES.contains(&name.as_str()) && !NOT_SHELL_UI.contains(&class)
}

/// The test Windows uses for which windows get a taskbar button.
fn is_app_window(window: HWND) -> bool {
    // SAFETY: these read properties of a window handle; a stale one reads as empty.
    unsafe {
        if !IsWindowVisible(window).as_bool() {
            return false;
        }
        let style = GetWindowLongW(window, GWL_EXSTYLE) as u32;
        if style & WS_EX_APPWINDOW.0 != 0 {
            return true;
        }
        let owned = GetWindow(window, GW_OWNER).is_ok_and(|owner| !owner.is_invalid());
        !owned && style & WS_EX_TOOLWINDOW.0 == 0
    }
}

/// True for Windows' own ApplicationFrameHost.exe, which hosts Store apps.
fn is_frame_host(pid: u32) -> bool {
    let Some(image) = image_path(pid) else { return false };
    let system = std::env::var_os("SystemRoot")
        .map(|root| PathBuf::from(root).join("System32").join("ApplicationFrameHost.exe"));
    system.is_some_and(|expected| same_path(&image, &expected))
}

/// The processes that own windows inside `window`.
fn child_window_pids(window: HWND) -> Vec<u32> {
    unsafe extern "system" fn visit(child: HWND, param: LPARAM) -> BOOL {
        // SAFETY: `param` is the Vec passed to `EnumChildWindows` below.
        let pids = &mut *(param.0 as *mut Vec<u32>);
        let mut pid = 0u32;
        GetWindowThreadProcessId(child, Some(&mut pid));
        if pid != 0 && !pids.contains(&pid) {
            pids.push(pid);
        }
        true.into()
    }

    let mut pids: Vec<u32> = Vec::new();
    // SAFETY: `pids` lives until `EnumChildWindows` returns.
    unsafe {
        let _ = EnumChildWindows(Some(window), Some(visit), LPARAM(&mut pids as *mut Vec<u32> as isize));
    }
    pids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_windows_are_told_from_programs() {
        let root = PathBuf::from(std::env::var_os("SystemRoot").unwrap());
        let explorer = root.join("explorer.exe");
        let search = root.join(r"SystemApps\MicrosoftWindows.Client.CBS_cw5n1h2txyewy\SearchHost.exe");
        // Seen taking the front while Niwa opened from Start (Oct 6, 2026).
        assert!(is_shell_window(&explorer, "Shell_TrayWnd"));
        assert!(is_shell_window(&explorer, "ApplicationManager_DesktopShellWindow"));
        assert!(is_shell_window(&search, "Windows.UI.Core.CoreWindow"));
        // File Explorer and the desktop are places the user goes.
        assert!(!is_shell_window(&explorer, "CabinetWClass"));
        assert!(!is_shell_window(&explorer, "Progman"));
        // Only Windows' own copies count.
        assert!(!is_shell_window(Path::new(r"C:\Games\explorer.exe"), "Shell_TrayWnd"));
        assert!(!is_shell_window(&root.join("notepad.exe"), "Notepad"));
    }

    #[test]
    fn snapshot_finds_this_process() {
        let mut buffer = SnapshotBuffer::default();
        for _ in 0..2 {
            let processes = snapshot(&mut buffer).expect("process list");
            let me = processes
                .iter()
                .find(|entry| entry.pid == std::process::id())
                .expect("this process in the list");
            let exe = std::env::current_exe().unwrap();
            assert_eq!(me.exe_name, exe.file_name().unwrap().to_string_lossy());
            assert!(me.created > 0);
            assert!(processes.iter().any(|entry| entry.pid == me.parent_pid));
            assert!(processes.len() > 10);
        }
    }
}
