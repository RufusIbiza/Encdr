//! Talking to a controller through the NI hardware service instead of USB.
//!
//! On Windows, NI's driver owns the Maschine Mk3's screen endpoint, so raw USB
//! access to it fails unless the interface is rebound to WinUSB. The NI
//! hardware service, however, draws on those screens for any client that
//! registers with it, and keeps working alongside NI's own software. The
//! messages are the same everywhere; only the transport differs (named pipes on
//! Windows, message ports on macOS). This module is that client.
//!
//! Only screens go through the service for now; inputs and LEDs keep using the
//! OS HID stack, which has no such restriction.

pub mod protocol;

/// Start of the error for a frame the service answers with a refusal. The
/// service does that while another client, typically NI's own software after
/// the controller's instance button is pressed, has the screens.
pub const REJECTED_FRAME: &str = "the NI service rejected a frame";

/// Whether an error from `send_display` is the service refusing a frame, as
/// opposed to a broken connection.
pub fn is_rejected_frame(error: &str) -> bool {
    error.starts_with(REJECTED_FRAME)
}

#[cfg(any(windows, target_os = "macos"))]
use std::sync::mpsc::{Receiver, RecvTimeoutError};
#[cfg(any(windows, target_os = "macos"))]
use std::time::{Duration, Instant};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::ServiceSession;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::ServiceSession;

#[cfg(not(any(windows, target_os = "macos")))]
mod unsupported;
#[cfg(not(any(windows, target_os = "macos")))]
pub use unsupported::ServiceSession;

/// Wait on the notification queue for the serial number of the unit to use.
///
/// The USB serial number may not be spelled the way the service reports it.
/// Prefer an exact match, but once any unit has reported, don't wait longer
/// than a short grace period for a better one.
#[cfg(any(windows, target_os = "macos"))]
fn pick_serial(rx: &Receiver<Vec<u8>>, wanted: Option<&str>, timeout: Duration) -> Result<String, String> {
    const GRACE: Duration = Duration::from_millis(1500);
    let mut deadline = Instant::now() + timeout;
    let mut serials: Vec<String> = Vec::new();
    let mut chosen: Option<String> = None;
    while chosen.is_none() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match rx.recv_timeout(left.min(Duration::from_millis(200))) {
            Ok(data) => {
                if let Some(state) = protocol::parse_device_state(&data) {
                    if state.present {
                        if wanted.is_some_and(|s| s.eq_ignore_ascii_case(&state.serial)) {
                            chosen = Some(state.serial);
                        } else if !serials.contains(&state.serial) {
                            serials.push(state.serial);
                            deadline = deadline.min(Instant::now() + GRACE);
                        }
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    chosen
        .or_else(|| serials.into_iter().next())
        .ok_or_else(|| "the NI service did not report a connected device".to_string())
}

/// Turn the notifications a session receives into control changes: `true`
/// when this client became the active instance, `false` when another client
/// took the hardware. The channel closes when the notifications stop.
#[cfg(any(windows, target_os = "macos"))]
fn watch_control(rx: Receiver<Vec<u8>>) -> async_channel::Receiver<bool> {
    let (tx, changes) = async_channel::unbounded();
    let _ = std::thread::Builder::new().name("encdr-ni-control".to_string()).spawn(move || {
        for data in rx {
            if let Some(active) = protocol::parse_control_state(&data) {
                tracing::debug!("NI service: this client {} the hardware", if active { "has" } else { "no longer has" });
                if tx.send_blocking(active).is_err() {
                    break;
                }
            }
        }
    });
    changes
}

/// Connect to the service, retrying a few times.
///
/// The service sometimes drops a registration straight after accepting it,
/// typically right after a previous client went away; the next attempt then
/// succeeds. A service that isn't running, or a unit it never reports, is not
/// worth retrying.
#[cfg(any(windows, target_os = "macos"))]
pub fn connect_with_retry(
    product_id: u16,
    serial: Option<&str>,
    timeout: Duration,
) -> Result<ServiceSession, String> {
    const ATTEMPTS: u32 = 4;
    const PAUSE: Duration = Duration::from_millis(750);
    let mut attempt = 1;
    loop {
        match ServiceSession::connect(product_id, serial, timeout) {
            Ok(session) => return Ok(session),
            Err(e) if attempt >= ATTEMPTS || e.contains("not running") || e.contains("did not report") => {
                return Err(e);
            }
            Err(e) => {
                tracing::debug!("NI service connect attempt {attempt}/{ATTEMPTS} failed: {e}; retrying");
                attempt += 1;
                std::thread::sleep(PAUSE);
            }
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub use unsupported::connect_with_retry;
