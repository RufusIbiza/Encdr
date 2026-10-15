//! Talking to a controller through the NI hardware service instead of USB.
//!
//! On Windows, NI's driver owns the Maschine Mk3's screen endpoint, so raw USB
//! access to it fails unless the interface is rebound to WinUSB. The NI
//! hardware service, however, draws on those screens for any client that
//! registers with it over named pipes, and keeps working alongside NI's own
//! software. This module is that client.
//!
//! Only screens go through the service for now; inputs and LEDs keep using the
//! OS HID stack, which has no such restriction.

pub mod protocol;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::ServiceSession;

#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
pub use unsupported::ServiceSession;
