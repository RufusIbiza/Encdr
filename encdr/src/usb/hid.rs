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
    use std::collections::{HashMap, HashSet};
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
        /// Declared output report payload sizes in bytes (excluding the report
        /// ID), from the device's report descriptor. Empty if unavailable.
        output_lens: HashMap<u8, usize>,
        /// Report IDs already warned about for oversized writes.
        warned_oversize: Mutex<HashSet<u8>>,
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

            let mut exact: Vec<_> = candidates
                .iter()
                .filter(|d| d.interface_number() == interface_number as i32)
                .collect();
            // Windows exposes each top-level HID collection of an interface as
            // its own device node. NI's control reports live on the
            // vendor-defined collection, so prefer that one.
            exact.sort_by_key(|d| std::cmp::Reverse(d.usage_page() >= 0xFF00));
            if exact.len() > 1 {
                let nodes: Vec<String> = exact
                    .iter()
                    .map(|d| format!("usage {:04x}:{:04x}", d.usage_page(), d.usage()))
                    .collect();
                tracing::warn!(
                    "Interface {} exposes {} HID collections ({}); using the first",
                    interface_number,
                    exact.len(),
                    nodes.join(", ")
                );
            }
            // Some platforms don't report interface numbers (-1). If that's
            // the case and there is exactly one node, it can only be ours.
            // Likewise, if exact interface number matching fails, but the device
            // only exposes a single HID node or a unique NI vendor-defined collection
            // (e.g. Komplete Kontrol Mk3 whose HID interface is 2), fall back to it.
            let info = match exact.as_slice() {
                [one, ..] => **one,
                [] if candidates.len() == 1 && candidates[0].interface_number() < 0 => candidates[0],
                [] if candidates.len() == 1 => {
                    tracing::warn!(
                        "No HID node matching interface {}, but found unique HID device node on interface {} (usage {:04x}:{:04x}); using it",
                        interface_number,
                        candidates[0].interface_number(),
                        candidates[0].usage_page(),
                        candidates[0].usage()
                    );
                    candidates[0]
                }
                [] if candidates.iter().filter(|d| d.usage_page() >= 0xFF00).count() == 1 => {
                    let vendor = candidates.iter().find(|d| d.usage_page() >= 0xFF00).unwrap();
                    tracing::warn!(
                        "No HID node matching interface {}, but found unique vendor-defined HID collection on interface {} (usage {:04x}:{:04x}); using it",
                        interface_number,
                        vendor.interface_number(),
                        vendor.usage_page(),
                        vendor.usage()
                    );
                    *vendor
                }
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

            let descriptor = read_report_descriptor(&writer);
            let uses_report_ids = descriptor.as_deref().map(descriptor_has_report_id).unwrap_or(true);
            let output_lens = descriptor.as_deref().map(output_report_lengths).unwrap_or_default();
            tracing::debug!(
                "Opened HID interface {} via hidapi (numbered reports: {}, output report payload sizes: {:?})",
                interface_number,
                uses_report_ids,
                output_lens
            );
            Ok(Self {
                path,
                uses_report_ids,
                output_lens,
                warned_oversize: Mutex::new(HashSet::new()),
                writer: Mutex::new(writer),
            })
        }

        /// Send an output report. `data` is the raw interrupt OUT payload,
        /// i.e. starts with the report ID for numbered reports.
        pub fn write(&self, data: &[u8]) -> Result<(), String> {
            self.warn_if_oversize(data);
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

        /// Windows pads short output reports but silently truncates long ones
        /// to the declared length, which would corrupt e.g. LED data. Say so
        /// once per report ID instead of failing mysteriously.
        fn warn_if_oversize(&self, data: &[u8]) {
            let (id, payload) = if self.uses_report_ids {
                match data.split_first() {
                    Some((id, rest)) => (*id, rest.len()),
                    None => return,
                }
            } else {
                (0, data.len())
            };
            let Some(&declared) = self.output_lens.get(&id) else { return };
            if payload > declared && self.warned_oversize.lock().map(|mut w| w.insert(id)).unwrap_or(false) {
                tracing::warn!(
                    "HID output report 0x{:02x}: writing {} payload bytes but the device declares {}; \
                     the OS may truncate the report",
                    id, payload, declared
                );
            }
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

    fn read_report_descriptor(dev: &HidDevice) -> Option<Vec<u8>> {
        let mut buf = [0u8; hidapi::MAX_REPORT_DESCRIPTOR_SIZE];
        let len = dev.get_report_descriptor(&mut buf).ok()?;
        Some(buf[..len].to_vec())
    }

    /// Declared Output report payload size in bytes (excluding the report ID),
    /// keyed by report ID (0 when the descriptor has no Report ID items).
    pub(super) fn output_report_lengths(desc: &[u8]) -> HashMap<u8, usize> {
        let mut bits: HashMap<u8, usize> = HashMap::new();
        let (mut size, mut count, mut id) = (0usize, 0usize, 0u8);
        let mut i = 0;
        while i < desc.len() {
            let prefix = desc[i];
            if prefix == 0xFE {
                i += 3 + desc.get(i + 1).copied().unwrap_or(0) as usize;
                continue;
            }
            let n = [0, 1, 2, 4][(prefix & 0x03) as usize];
            let value = desc[i + 1..(i + 1 + n).min(desc.len())]
                .iter()
                .enumerate()
                .fold(0usize, |acc, (k, b)| acc | (*b as usize) << (8 * k));
            match prefix & 0xFC {
                0x74 => size = value,
                0x94 => count = value,
                0x84 => id = value as u8,
                0x90 => *bits.entry(id).or_default() += size * count,
                _ => {}
            }
            i += 1 + n;
        }
        bits.into_iter().map(|(id, b)| (id, b.div_ceil(8))).collect()
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
    use super::imp::{descriptor_has_report_id, output_report_lengths};

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
    fn output_lengths_per_report_id() {
        // Report 0x80: 41 bytes out; report 0x81: 2 x 16-bit = 4 bytes out; an input item is ignored.
        let desc = [
            0xA1, 0x01, //
            0x85, 0x80, 0x75, 0x08, 0x95, 0x29, 0x91, 0x02, //
            0x85, 0x81, 0x75, 0x10, 0x95, 0x02, 0x91, 0x02, //
            0x85, 0x01, 0x75, 0x08, 0x95, 0x07, 0x81, 0x02, //
            0xC0,
        ];
        let lens = output_report_lengths(&desc);
        assert_eq!(lens.get(&0x80), Some(&41));
        assert_eq!(lens.get(&0x81), Some(&4));
        assert_eq!(lens.get(&0x01), None);
    }

    #[test]
    fn report_id_byte_inside_item_data_is_ignored() {
        // Usage Page with a data byte of 0x85 must not read as a Report ID.
        assert!(!descriptor_has_report_id(&[0x05, 0x85, 0x09, 0x01]));
    }
}
