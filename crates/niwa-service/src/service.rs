//! Running as a Windows service: talking to the Service Control Manager.

use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

use niwa_core::ipc::SERVICE_NAME;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};

use crate::engine::{Command, Settings};

define_windows_service!(ffi_service_main, service_main);

/// Hands the process over to the Service Control Manager. Only works when
/// Windows started the process as a service.
pub fn run() -> windows_service::Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = serve() {
        crate::log(&format!("the service stopped with an error: {e}"));
    }
}

fn status(state: ServiceState, session_active: bool) -> ServiceStatus {
    // While a Strict session runs, the service doesn't accept being stopped.
    // It still has to accept a shutdown, to save the session before the PC
    // goes off.
    let controls = match (state, session_active) {
        (ServiceState::Running, false) => ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        (ServiceState::Running, true) => ServiceControlAccept::SHUTDOWN,
        _ => ServiceControlAccept::empty(),
    };
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: controls,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }
}

fn serve() -> windows_service::Result<()> {
    let active = Arc::new(AtomicBool::new(false));
    let (stop, stopped) = mpsc::channel::<()>();

    let handler_active = active.clone();
    let handle = service_control_handler::register(SERVICE_NAME, move |control| match control {
        ServiceControl::Stop if handler_active.load(Ordering::SeqCst) => {
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Stop | ServiceControl::Shutdown => {
            let _ = stop.send(());
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    })?;

    let engine = crate::start(
        Settings {
            state_dir: crate::install::state_dir(),
            watch_card: true,
            time_scale: 1,
            run_installers: true,
        },
        active.clone(),
    );
    let Some(engine) = engine else {
        handle.set_service_status(status(ServiceState::Stopped, false))?;
        return Ok(());
    };

    let mut reported = active.load(Ordering::SeqCst);
    handle.set_service_status(status(ServiceState::Running, reported))?;
    loop {
        match stopped.recv_timeout(Duration::from_millis(500)) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                // Tell Windows when a session starts or ends, which changes
                // whether the service can be stopped.
                let now = active.load(Ordering::SeqCst);
                if now != reported {
                    reported = now;
                    handle.set_service_status(status(ServiceState::Running, now))?;
                }
            }
        }
    }

    // Save the session before going: this is what lets it resume after a restart.
    let (done, saved) = mpsc::channel();
    if engine.send(Command::Shutdown(done)).is_ok() {
        let _ = saved.recv_timeout(Duration::from_secs(5));
    }
    handle.set_service_status(status(ServiceState::Stopped, false))?;
    Ok(())
}
