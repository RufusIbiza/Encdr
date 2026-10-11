//! HID transport for HID-class interfaces that nusb cannot claim.
//!
//! macOS binds `AppleUserUSBHostHIDDevice` to every HID interface and nusb
//! cannot detach it; Windows binds `HidUsb` the same way. Both OSes do expose
//! the interface through their native HID stack (IOHIDManager / HID.dll),
//! which `hidapi` wraps. On Linux, nusb can detach `usbhid` itself, so this
//! backend is only built there with the `hid` feature (hidraw), mainly for
//! testing the code path.
//!
//! The wire format is identical to the raw interrupt endpoints: numbered
//! reports carry the report ID as their first byte in both directions, so the
//! descriptor-driven parsers and LED builders work unchanged. Devices whose
//! report descriptor declares no report IDs get the `0x00` prefix hidapi
//! requires on writes added transparently.

#[cfg(any(target_os = "macos", target_os = "windows", feature = "hid"))]
mod imp {
    use std::ffi::CString;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;

    use hidapi::{HidApi, HidDevice};

    /// How long a blocked `read` waits before re-checking the stop flag.
    const READ_POLL_MS: i32 = 100;

    pub const AVAILABLE: bool = true;

    /// A HID interface opened through the OS HID stack.
    pub struct HidInterface {
        path: CString,
        uses_report_ids: bool,
        /// Shared handle for output and feature reports. Reads use their own
        /// handle (see [`HidInterface::spawn_reader`]) so a blocked read never
        /// delays an LED write.
        writer: Mutex<HidDevice>,
    }

    impl HidInterface {
        /// Find and open the HID device node for USB interface
        /// `interface_number` of the device identified by `vid`/`pid`
        /// (and `serial`, when both sides report one).
        pub fn open(
            vid: u16,
            pid: u16,
            serial: Option<&str>,
            interface_number: u8,
        ) -> Result<Self, String> {
            let api = HidApi::new().map_err(|e| format!("hidapi init failed: {e}"))?;

            let candidates: Vec<_> = api
                .device_list()
                .filter(|d| d.vendor_id() == vid && d.product_id() == pid)
                .filter(|d| match (serial, d.serial_number()) {
                    (Some(want), Some(have)) if !want.is_empty() && !have.is_empty() => want == have,
                    _ => true,
                })
                .collect();

            let exact: Vec<_> = candidates
                .iter()
                .filter(|d| d.interface_number() == interface_number as i32)
                .collect();
            // Some platforms don't report interface numbers (-1). If that's
            // the case and there is exactly one node, it can only be ours.
            let info = match exact.as_slice() {
                [one, ..] => **one,
                [] if candidates.len() == 1 && candidates[0].interface_number() < 0 => candidates[0],
                [] => {
                    let seen: Vec<String> = candidates
                        .iter()
                        .map(|d| format!("iface {} usage {:04x}:{:04x}", d.interface_number(), d.usage_page(), d.usage()))
                        .collect();
                    return Err(format!(
                        "no HID device node for interface {interface_number} of {vid:04x}:{pid:04x} (found: [{}])",
                        seen.join(", ")
                    ));
                }
            };

            let path = info.path().to_owned();
            let writer = api
                .open_path(&path)
                .map_err(|e| format!("could not open HID device {path:?}: {e}"))?;

            let uses_report_ids = report_descriptor_uses_ids(&writer).unwrap_or(true);
            tracing::debug!(
                "Opened HID interface {} via hidapi (numbered reports: {})",
                interface_number,
                uses_report_ids
            );
            Ok(Self { path, uses_report_ids, writer: Mutex::new(writer) })
        }

        /// Send an output report. `data` is the raw interrupt OUT payload,
        /// i.e. starts with the report ID for numbered reports.
        pub fn write(&self, data: &[u8]) -> Result<(), String> {
            let dev = self.writer.lock().map_err(|_| "HID writer poisoned".to_string())?;
            let res = if self.uses_report_ids {
                dev.write(data)
            } else {
                let mut buf = Vec::with_capacity(data.len() + 1);
                buf.push(0);
                buf.extend_from_slice(data);
                dev.write(&buf)
            };
            res.map(|_| ()).map_err(|e| e.to_string())
        }

        /// Send a feature report (`data[0]` is the report ID).
        pub fn send_feature_report(&self, data: &[u8]) -> Result<(), String> {
            let dev = self.writer.lock().map_err(|_| "HID writer poisoned".to_string())?;
            if self.uses_report_ids {
                dev.send_feature_report(data)
            } else {
                let mut buf = Vec::with_capacity(data.len() + 1);
                buf.push(0);
                buf.extend_from_slice(data);
                dev.send_feature_report(&buf)
            }
            .map_err(|e| e.to_string())
        }

        /// Start a thread that reads input reports and forwards them to the
        /// returned [`HidReader`]. `report_size` is the largest expected report.
        pub fn spawn_reader(&self, name: String, report_size: usize) -> Result<HidReader, String> {
            let api = HidApi::new().map_err(|e| format!("hidapi init failed: {e}"))?;
            let dev = api
                .open_path(&self.path)
                .map_err(|e| format!("could not open HID device {:?} for reading: {e}", self.path))?;

            let (tx, rx) = async_channel::bounded(256);
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = stop.clone();
            let size = report_size.max(64) + 1;

            let thread = thread::Builder::new()
                .name(name)
                .spawn(move || {
                    let mut buf = vec![0u8; size];
                    while !thread_stop.load(Ordering::Relaxed) {
                        match dev.read_timeout(&mut buf, READ_POLL_MS) {
                            Ok(0) => continue,
                            Ok(n) => {
                                if tx.send_blocking(Ok(buf[..n].to_vec())).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                tx.send_blocking(Err(e.to_string())).ok();
                                break;
                            }
                        }
                    }
                })
                .map_err(|e| e.to_string())?;

            Ok(HidReader { rx: Box::pin(rx), stop, thread: Some(thread) })
        }
    }

    /// Input reports from a HID interface, delivered as an async stream.
    pub struct HidReader {
        pub rx: Pin<Box<async_channel::Receiver<Result<Vec<u8>, String>>>>,
        stop: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl Drop for HidReader {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            self.rx.close();
            if let Some(t) = self.thread.take() {
                t.join().ok();
            }
        }
    }

    /// Whether the HID report descriptor declares Report ID items (0x85).
    fn report_descriptor_uses_ids(dev: &HidDevice) -> Option<bool> {
        let mut buf = [0u8; hidapi::MAX_REPORT_DESCRIPTOR_SIZE];
        let len = dev.get_report_descriptor(&mut buf).ok()?;
        Some(descriptor_has_report_id(&buf[..len]))
    }

    pub(super) fn descriptor_has_report_id(desc: &[u8]) -> bool {
        let mut i = 0;
        while i < desc.len() {
            let prefix = desc[i];
            if prefix == 0xFE {
                // Long item: next byte is the data length.
                i += 3 + desc.get(i + 1).copied().unwrap_or(0) as usize;
                continue;
            }
            if prefix & 0xFC == 0x84 {
                return true;
            }
            i += 1 + [0, 1, 2, 4][(prefix & 0x03) as usize];
        }
        false
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", feature = "hid")))]
mod imp {
    //! Stub used where the HID backend isn't built, so callers need no cfg.

    use std::pin::Pin;

    pub const AVAILABLE: bool = false;

    pub struct HidInterface;

    impl HidInterface {
        pub fn open(_: u16, _: u16, _: Option<&str>, _: u8) -> Result<Self, String> {
            Err("HID backend not compiled in (enable the `hid` feature)".to_string())
        }
        pub fn write(&self, _: &[u8]) -> Result<(), String> {
            Err("HID backend not compiled in".to_string())
        }
        pub fn send_feature_report(&self, _: &[u8]) -> Result<(), String> {
            Err("HID backend not compiled in".to_string())
        }
        pub fn spawn_reader(&self, _: String, _: usize) -> Result<HidReader, String> {
            Err("HID backend not compiled in".to_string())
        }
    }

    pub struct HidReader {
        pub rx: Pin<Box<async_channel::Receiver<Result<Vec<u8>, String>>>>,
    }
}

pub use imp::{HidInterface, HidReader, AVAILABLE};

#[cfg(all(test, any(target_os = "macos", target_os = "windows", feature = "hid")))]
mod tests {
    use super::imp::descriptor_has_report_id;

    #[test]
    fn detects_report_id_item() {
        // Usage Page, Usage, Collection, Report ID 1, ...
        assert!(descriptor_has_report_id(&[0x06, 0x00, 0xFF, 0x09, 0x01, 0xA1, 0x01, 0x85, 0x01, 0xC0]));
    }

    #[test]
    fn unnumbered_descriptor_has_none() {
        assert!(!descriptor_has_report_id(&[0x06, 0x00, 0xFF, 0x09, 0x01, 0xA1, 0x01, 0x75, 0x08, 0xC0]));
    }

    #[test]
    fn report_id_byte_inside_item_data_is_ignored() {
        // Usage Page with a data byte of 0x85 must not read as a Report ID.
        assert!(!descriptor_has_report_id(&[0x05, 0x85, 0x09, 0x01]));
    }
}
