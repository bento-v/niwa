//! The named pipe the card talks to the service over. Each connection carries
//! one JSON request line and gets one JSON reply line.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::sync::mpsc;
use std::thread;

use niwa_blocker::process::{image_path, owner};
use niwa_core::ipc::{Reply, Request};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{GetLastError, ERROR_PIPE_CONNECTED, HANDLE};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
    PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
    PIPE_WAIT,
};

use crate::engine::{Caller, Command};

/// The longest request accepted. A plan with a long app list is a few
/// kilobytes; anything near this is not a real request.
const MAX_REQUEST: u64 = 256 * 1024;

/// Accepts connections until the engine goes away. Never returns otherwise.
pub fn serve(path: &str, engine: mpsc::Sender<Command>) -> windows::core::Result<()> {
    let name: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

    // The service runs as SYSTEM, whose pipes are closed to ordinary users by
    // default. This lets programs in a login session read and write; SYSTEM
    // and administrators keep full control. Remote clients are refused below.
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: the SDDL string is a constant; the descriptor it allocates lives
    // for the life of the process, as the pipe server does.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)"),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )?;
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };

    loop {
        // SAFETY: `name` is NUL-terminated and `attributes` outlives the call.
        let pipe = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                64 * 1024,
                64 * 1024,
                0,
                Some(&attributes),
            )
        };
        if pipe.is_invalid() {
            return Err(windows::core::Error::from_win32());
        }

        // SAFETY: `pipe` is a valid pipe handle. A client that connected
        // between the two calls shows up as ERROR_PIPE_CONNECTED.
        let connected = unsafe {
            ConnectNamedPipe(pipe, None).is_ok() || GetLastError() == ERROR_PIPE_CONNECTED
        };
        // SAFETY: the handle is valid and owned from here by the `File`,
        // which closes it when dropped.
        let connection = unsafe { File::from_raw_handle(pipe.0) };
        if !connected {
            continue;
        }

        // Each connection gets its own thread, so a client that connects and
        // then says nothing can't hold up the others.
        let engine = engine.clone();
        thread::spawn(move || answer(connection, &engine));
    }
}

fn answer(mut connection: File, engine: &mpsc::Sender<Command>) {
    let handle = HANDLE(connection.as_raw_handle());
    let reply = read_request(&connection, handle)
        .and_then(|(request, caller)| {
            let (reply_to, reply) = mpsc::channel();
            engine.send(Command::Request(request, caller, reply_to)).ok()?;
            reply.recv().ok()
        })
        .unwrap_or_else(|| Reply::Error("Couldn't read the request.".into()));

    if let Ok(mut line) = serde_json::to_string(&reply) {
        line.push('\n');
        let _ = connection.write_all(line.as_bytes());
        // Wait for the client to read the reply before hanging up.
        let _ = connection.sync_all();
    }
    // SAFETY: `handle` is the connection's handle, still open here.
    unsafe {
        let _ = DisconnectNamedPipe(handle);
    }
}

fn read_request(connection: &File, handle: HANDLE) -> Option<(Request, Caller)> {
    let mut line = String::new();
    BufReader::new(Read::take(connection, MAX_REQUEST))
        .read_line(&mut line)
        .ok()?;
    let request = serde_json::from_str(&line).ok()?;

    let mut client_pid = 0u32;
    // SAFETY: `handle` is a connected pipe; `client_pid` is a valid out pointer.
    let known = unsafe { GetNamedPipeClientProcessId(handle, &mut client_pid) }.is_ok();
    let caller = Caller {
        owner: known.then(|| owner(client_pid)).flatten(),
        // Only the card beside the service may report the user's windows.
        is_card: known
            && image_path(client_pid)
                .zip(crate::launch::card_path())
                .is_some_and(|(actual, card)| {
                    actual.to_string_lossy().eq_ignore_ascii_case(&card.to_string_lossy())
                }),
    };
    Some((request, caller))
}
