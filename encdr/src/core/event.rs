use std::sync::Arc;

use crate::core::descriptor::DeviceDescriptor;

/// Unique identifier for a connected device instance.
/// Combines bus/address info to survive reconnects at different addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct DeviceId(pub(crate) u64);

impl DeviceId {
    pub(crate) fn from_usb(bus: u8, address: u8, vendor_id: u16, product_id: u16) -> Self {
        let id = (bus as u64) << 48
            | (address as u64) << 40
            | (vendor_id as u64) << 16
            | product_id as u64;
        DeviceId(id)
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dev:{:016x}", self.0)
    }
}

/// All input events carry the source device and the control's string name
/// (as defined in the JSON descriptor). Names are interned &'static str to
/// avoid per-event allocation.
#[derive(Debug, Clone)]
pub enum Event {
    DeviceConnected {
        id: DeviceId,
        descriptor: Arc<DeviceDescriptor>,
    },
    DeviceDisconnected {
        id: DeviceId,
    },
    Button {
        device: DeviceId,
        name: &'static str,
        pressed: bool,
    },
    Slider {
        device: DeviceId,
        name: &'static str,
        value: f32,
    },
    Encoder {
        device: DeviceId,
        name: &'static str,
        delta: i32,
    },
    EncoderFine {
        device: DeviceId,
        name: &'static str,
        delta: f32,
    },
    Touch {
        device: DeviceId,
        name: &'static str,
        touched: bool,
    },
    Grid {
        device: DeviceId,
        name: &'static str,
        index: u8,
        pressure: f32,
    },
}

impl Event {
    /// Returns the DeviceId associated with this event.
    pub fn device_id(&self) -> DeviceId {
        match self {
            Event::DeviceConnected { id, .. } => *id,
            Event::DeviceDisconnected { id } => *id,
            Event::Button { device, .. } => *device,
            Event::Slider { device, .. } => *device,
            Event::Encoder { device, .. } => *device,
            Event::EncoderFine { device, .. } => *device,
            Event::Touch { device, .. } => *device,
            Event::Grid { device, .. } => *device,
        }
    }
}
