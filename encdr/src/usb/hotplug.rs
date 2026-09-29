use std::sync::Arc;

use nusb::{DeviceInfo, MaybeFuture};

use crate::core::descriptor::DeviceDescriptor;
use crate::core::event::DeviceId;
use crate::device::loader::DescriptorRegistry;

/// Result of scanning for devices that match loaded descriptors.
pub struct DetectedDevice {
    pub device_id: DeviceId,
    pub descriptor: Arc<DeviceDescriptor>,
    pub usb_info: DeviceInfo,
}

/// Scan all currently connected USB devices and return those matching
/// any loaded descriptor.
pub fn scan_devices(registry: &DescriptorRegistry) -> Vec<DetectedDevice> {
    let mut found = Vec::new();

    let devices = match nusb::list_devices().wait() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Failed to list USB devices: {}", e);
            return found;
        }
    };

    for info in devices {
        let vid = info.vendor_id();
        let pid = info.product_id();

        if let Some(desc) = registry.find(vid, pid) {
            #[cfg(target_os = "linux")]
            let bus = info.busnum();
            #[cfg(not(target_os = "linux"))]
            let bus = info.bus_id().parse::<u8>().unwrap_or(0);

            let device_id = DeviceId::from_usb(
                bus,
                info.device_address(),
                vid,
                pid,
            );
            found.push(DetectedDevice {
                device_id,
                descriptor: desc.clone(),
                usb_info: info,
            });
        }
    }

    found
}
