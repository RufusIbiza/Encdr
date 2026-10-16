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
