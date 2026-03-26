use crate::core::event::{DeviceId, Event};

/// Escape hatch for protocols that can't be expressed declaratively in JSON.
/// Apps can register a PacketHook to intercept and parse raw USB packets
/// for devices with truly exotic protocols.
pub trait PacketHook: Send + Sync + 'static {
    /// Called for each raw USB input packet before the generic parser runs.
    /// Return `true` to consume the packet (skip generic parsing),
    /// or `false` to let the generic parser handle it.
    fn on_packet(
        &mut self,
        device_id: DeviceId,
        packet: &[u8],
        events: &mut Vec<Event>,
    ) -> bool;
}

/// No-op hook that passes all packets through to the generic parser.
pub(crate) struct NoopHook;

impl PacketHook for NoopHook {
    fn on_packet(
        &mut self,
        _device_id: DeviceId,
        _packet: &[u8],
        _events: &mut Vec<Event>,
    ) -> bool {
        false
    }
}
