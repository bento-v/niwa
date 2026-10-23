//! How the card talks to the Strict service: one JSON request and one JSON
//! reply per connection, over a named pipe.

use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::schedule::Schedule;
use crate::strict::{EndedAs, SlipRecord, StrictPlan};

pub const SERVICE_NAME: &str = "NiwaStrict";

/// What this build of the service understands. 2 added app categories,
/// allow-only lists and `Request::Windows`; 3 added website categories and
/// `Request::Sites`; 4 added websites the user typed in; 5 made the wait to
/// end a Strict session early a third of the session instead of 15 minutes.
/// Website blocking (`Request::Sites`) was taken out again on Oct 6, 2026
/// without a new number: cards since then never ask for it. 6 added
/// `Request::InstallUpdate`. 7 added scheduled plans (up to 12 hours) and
/// `Request::Schedules`.
/// A service that reports less would act differently, so the card won't
/// start a Strict session (or a list it wouldn't understand) on it.
pub const PROTOCOL: u32 = 7;
const DEFAULT_PIPE: &str = r"\\.\pipe\niwa-strict";

/// The pipe's path. Debug builds can point it elsewhere with `NIWA_PIPE`, so
/// tests can run a second service beside an installed one.
pub fn pipe_path() -> String {
    if cfg!(debug_assertions) {
        if let Ok(path) = std::env::var("NIWA_PIPE") {
            return path;
        }
    }
    DEFAULT_PIPE.to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Request {
    Status,
    Start { plan: StrictPlan },
    /// Begin the cooldown that ends the session early.
    RequestExit,
    CancelExit,
    /// End the session at once. Only debug builds of the service honour this:
    /// it exists so a bug during development can't lock anyone out.
    DevAbort,
    /// Allow-only: the processes with a taskbar window right now. The service
    /// can't see the user's desktop, so the card reports them, and the
    /// service closes no allow-only candidate without a fresh report. Answered
    /// like `Status`.
    Windows { pids: Vec<u32> },
    /// Install a newer Niwa without asking for admin (approved Oct 6, 2026):
    /// the installer the card downloaded, a file the service reads, with its
    /// update signature. Only from the card, never during a session. The
    /// service runs it only if `update::check_installer` passes; the
    /// installer then replaces the service, and the new one reopens the card.
    /// Answered like `Status` once the installer has started.
    InstallUpdate {
        installer: String,
        signature: String,
        version: String,
    },
    /// The card's schedules, sent whenever they change. If a window comes
    /// round while the card isn't running, the service opens it (once per
    /// window) for the account that sent them, and the card starts the
    /// session. Only from the card in a login session. Answered like `Status`.
    Schedules { schedules: Vec<Schedule> },
}

/// An app that was open when the session started and is about to be closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Closing {
    pub label: String,
    pub secs_left: u32,
}

/// A blocked program the service is about to end. The card, which shares the
/// user's desktop, asks its windows to close first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToClose {
    pub pid: u32,
    pub label: String,
    /// The program's full path, checked again before its windows are asked
    /// to close so a recycled pid can't get the wrong one closed.
    pub image_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Active {
    pub plan: StrictPlan,
    pub remaining_secs: u64,
    pub exit_secs_left: Option<u64>,
    pub slips: Vec<SlipRecord>,
    pub closing: Vec<Closing>,
    pub to_close: Vec<ToClose>,
}

/// The most recent session that has ended, kept until another starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finished {
    pub session_id: i64,
    pub outcome: EndedAs,
    pub focused_secs: u64,
    pub remaining_secs: u64,
    pub slips: Vec<SlipRecord>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub active: Option<Active>,
    pub finished: Option<Finished>,
    /// The service's `PROTOCOL`. Services from before it was added say 0.
    #[serde(default)]
    pub version: u32,
}

// One reply is built per request and sent straight away, so the size
// difference between the variants costs nothing worth boxing for.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reply {
    Ok(Status),
    Error(String),
}

/// Sends one request to the service and waits for its reply.
///
/// An `io::Error` means the service couldn't be reached; `Ok(Err(message))`
/// means it answered with a refusal to show the user.
pub fn request(request: &Request) -> io::Result<Result<Status, String>> {
    let path = pipe_path();
    // The pipe serves one caller at a time; if it's busy, it will be free in
    // a moment.
    const PIPE_BUSY: i32 = 231;
    let mut attempts = 0;
    let mut pipe = loop {
        match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(pipe) => break pipe,
            Err(e) if e.raw_os_error() == Some(PIPE_BUSY) && attempts < 20 => {
                attempts += 1;
                thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(e),
        }
    };

    let mut line = serde_json::to_string(request).map_err(io::Error::other)?;
    line.push('\n');
    pipe.write_all(line.as_bytes())?;
    pipe.flush()?;

    let mut reply = String::new();
    BufReader::new(pipe).read_line(&mut reply)?;
    match serde_json::from_str(&reply).map_err(io::Error::other)? {
        Reply::Ok(status) => Ok(Ok(status)),
        Reply::Error(message) => Ok(Err(message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_tagged_by_type() {
        assert_eq!(serde_json::to_string(&Request::Status).unwrap(), r#"{"type":"status"}"#);
        assert_eq!(
            serde_json::from_str::<Request>(r#"{"type":"requestExit"}"#).unwrap(),
            Request::RequestExit
        );
    }

    #[test]
    fn replies_round_trip() {
        let reply = Reply::Ok(Status::default());
        let json = serde_json::to_string(&reply).unwrap();
        assert_eq!(serde_json::from_str::<Reply>(&json).unwrap(), reply);
        let refusal = Reply::Error("A session is already running.".into());
        let json = serde_json::to_string(&refusal).unwrap();
        assert_eq!(serde_json::from_str::<Reply>(&json).unwrap(), refusal);
    }

    #[test]
    fn an_unreachable_service_is_an_io_error() {
        // Nothing listens on the default pipe during unit tests unless the
        // service happens to be installed; either way this must not hang.
        let _ = request(&Request::Status);
    }
}
