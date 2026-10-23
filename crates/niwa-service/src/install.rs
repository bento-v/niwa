//! Installing and removing the service. Both need administrator rights, once.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use niwa_core::ipc::{self, Request, SERVICE_NAME};
use windows_service::service::{
    ServiceAccess, ServiceAction, ServiceActionType, ServiceErrorControl, ServiceFailureActions,
    ServiceFailureResetPeriod, ServiceInfo, ServiceStartType, ServiceState, ServiceType,
};
use windows_service::service::Service;
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

/// Why installing over the service, or removing it, was refused. The
/// installer recognises it by the exit code it brings (see `main.rs`).
pub const SESSION_RUNNING: &str = "A Strict session is running. Niwa can't be changed or removed until it ends.";
/// Windows' "the specified service does not exist".
const ERROR_SERVICE_DOES_NOT_EXIST: i32 = 1060;
/// How long a stopping service gets before installing or removing gives up.
const STOP_TIMEOUT: Duration = Duration::from_secs(20);

/// Who may do what with the service. The stock rules, plus one change: users
/// in a login session ("IU") may also start it ("RP"), so the card can bring
/// the service back if it has been killed. Nobody but SYSTEM and
/// administrators can stop it or change it.
const SERVICE_RIGHTS: &str = "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWRPLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)";

/// Where the session state and its key live: `%ProgramData%\Niwa`.
pub fn state_dir() -> PathBuf {
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("Niwa")
}

fn run_tool(tool: &str, arguments: &[&str]) -> Result<(), String> {
    let output = Command::new(tool)
        .args(arguments)
        .output()
        .map_err(|e| format!("couldn't run {tool}: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{tool} failed: {}{}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn explain(error: windows_service::Error) -> String {
    match &error {
        windows_service::Error::Winapi(io) if io.raw_os_error() == Some(5) => {
            "Access denied. Run this from a terminal opened with \"Run as administrator\".".to_owned()
        }
        windows_service::Error::Winapi(io) => io.to_string(),
        other => other.to_string(),
    }
}

/// Whether the running service has a Strict session going.
pub fn session_running() -> bool {
    matches!(ipc::request(&Request::Status), Ok(Ok(status)) if status.active.is_some())
}

fn is_missing(error: &windows_service::Error) -> bool {
    matches!(error, windows_service::Error::Winapi(io) if io.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST))
}

/// Stops the service and waits until its program has exited, so the
/// installer can replace or remove the file.
fn stop_and_wait(service: &Service) -> Result<(), String> {
    if service.query_status().map_err(explain)?.current_state == ServiceState::Running {
        service.stop().map_err(explain)?;
    }
    let deadline = Instant::now() + STOP_TIMEOUT;
    while service.query_status().map_err(explain)?.current_state != ServiceState::Stopped {
        if Instant::now() >= deadline {
            return Err("The Niwa service didn't stop in time.".into());
        }
        thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

/// Registers the service, locks down its state folder and starts it. A
/// service already registered (by an older copy, say) is stopped and pointed
/// at this one, unless a session is running in it.
pub fn install() -> Result<(), String> {
    if session_running() {
        return Err(SESSION_RUNNING.into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(explain)?;

    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from("Niwa Strict Sessions"),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: vec![OsString::from("run")],
        dependencies: vec![],
        // No account means LocalSystem.
        account_name: None,
        account_password: None,
    };
    let access = ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::STOP | ServiceAccess::QUERY_STATUS;
    let service = match manager.open_service(SERVICE_NAME, access) {
        Ok(service) => {
            stop_and_wait(&service)?;
            service.change_config(&info).map_err(explain)?;
            service
        }
        Err(e) if is_missing(&e) => manager.create_service(&info, access).map_err(explain)?,
        Err(e) => return Err(explain(e)),
    };
    service
        .set_description("Keeps a Niwa Strict focus session blocking when the Niwa window is closed, and resumes it after a restart.")
        .map_err(explain)?;

    // If the service is killed, Windows starts it again a second later.
    service
        .update_failure_actions(ServiceFailureActions {
            reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(24 * 60 * 60)),
            reboot_msg: None,
            command: None,
            actions: Some(vec![
                ServiceAction {
                    action_type: ServiceActionType::Restart,
                    delay: Duration::from_secs(1),
                };
                3
            ]),
        })
        .map_err(explain)?;
    service
        .set_failure_actions_on_non_crash_failures(true)
        .map_err(explain)?;

    run_tool("sc.exe", &["sdset", SERVICE_NAME, SERVICE_RIGHTS])?;

    // The state folder: SYSTEM writes, everyone else can only look. The key
    // that seals the session file is created inside by the service itself and
    // is readable by SYSTEM alone.
    let dir = state_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // The key goes in first, while this installer can still write there.
    let key = dir.join("key.bin");
    if !key.exists() {
        let mut bytes = vec![0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
        std::fs::write(&key, bytes).map_err(|e| e.to_string())?;
    }
    run_tool(
        "icacls.exe",
        &[&key.to_string_lossy(), "/inheritance:r", "/grant:r", "*S-1-5-18:F"],
    )?;
    run_tool(
        "icacls.exe",
        &[
            &dir.to_string_lossy(),
            "/inheritance:r",
            "/grant:r",
            "*S-1-5-18:(OI)(CI)F",
            "*S-1-5-32-544:(OI)(CI)RX",
            "*S-1-5-32-545:(OI)(CI)RX",
        ],
    )?;

    service.start::<&str>(&[]).map_err(explain)?;
    Ok(())
}

/// Removes the service, waiting for it to stop. Refuses while a Strict
/// session is running: that is what stops uninstalling from being a way out
/// of one. With no service registered there is nothing to do.
pub fn uninstall() -> Result<(), String> {
    if session_running() {
        return Err(SESSION_RUNNING.into());
    }

    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(explain)?;
    let service = match manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    ) {
        Ok(service) => service,
        Err(e) if is_missing(&e) => return Ok(()),
        Err(e) => return Err(explain(e)),
    };
    stop_and_wait(&service)?;
    service.delete().map_err(explain)?;
    // Earlier versions blocked websites in the hosts file. The service takes
    // out a block they left when it starts; this catches one it never got to.
    // Uninstalling runs as an administrator, who can write the hosts file.
    if let Err(e) = niwa_blocker::hosts::clear() {
        eprintln!("Couldn't check the hosts file for an old Niwa block: {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_service_that_isnt_there_reads_as_missing() {
        // Connecting to the service manager and opening a service by name
        // needs no admin rights.
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).unwrap();
        let error = manager
            .open_service("NiwaNoSuchServiceForTests", ServiceAccess::QUERY_STATUS)
            .err()
            .expect("no such service");
        assert!(is_missing(&error));
    }
}
