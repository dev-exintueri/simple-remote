//! Spike only (throwaway): minimal LocalSystem service used to verify the WiX MSI
//! (service install, failure restart, ProgramData ACL). Not product code.
//!
//! Behaviour
//! - Appends "<unix-seconds> running pid=<pid>" to C:\ProgramData\SimpleRemoteSpike\service.log every 5 s.
//! - If C:\ProgramData\SimpleRemoteSpike\crash-once exists at start, deletes it and
//!   exits the process with code 1 after 2 s without reporting Stopped (simulated crash).

#[cfg(windows)]
fn main() -> windows_service::Result<()> {
    spike::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("windows only");
}

#[cfg(windows)]
mod spike {
    use std::ffi::OsString;
    use std::io::Write;
    use std::path::Path;
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::{define_windows_service, service_dispatcher, Result};

    const SERVICE_NAME: &str = "SimpleRemoteSpike";
    const DATA_DIR: &str = r"C:\ProgramData\SimpleRemoteSpike";

    pub fn run() -> Result<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)
    }

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<OsString>) {
        let _ = run_service();
    }

    fn log(line: &str) {
        let path = Path::new(DATA_DIR).join("service.log");
        // The MSI creates DATA_DIR; the service does not create it.
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let _ = writeln!(f, "{now} {line} pid={}", std::process::id());
        }
    }

    fn status(state: ServiceState, accept: ServiceControlAccept) -> ServiceStatus {
        ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accept,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        }
    }

    fn run_service() -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let handler = move |event| -> ServiceControlHandlerResult {
            match event {
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                ServiceControl::Stop => {
                    let _ = tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };
        let handle = service_control_handler::register(SERVICE_NAME, handler)?;
        handle.set_service_status(status(ServiceState::Running, ServiceControlAccept::STOP))?;
        log("started");

        let marker = Path::new(DATA_DIR).join("crash-once");
        if marker.exists() {
            let _ = std::fs::remove_file(&marker);
            log("crash-once marker found, exiting with code 1");
            std::thread::sleep(Duration::from_secs(2));
            std::process::exit(1);
        }

        loop {
            match rx.recv_timeout(Duration::from_secs(5)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => log("running"),
            }
        }
        log("stopping");
        handle.set_service_status(status(ServiceState::Stopped, ServiceControlAccept::empty()))?;
        Ok(())
    }
}
