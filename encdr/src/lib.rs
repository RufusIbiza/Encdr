pub mod core;
pub mod device;
pub mod screen;
pub mod usb;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};

pub use crate::core::descriptor::{DeviceDescriptor, LedProtocol, PixelFormat};
pub use crate::core::error::{EncdrError, Result};
pub use crate::core::event::{DeviceId, Event};
pub use crate::core::jog_ring::{JogDeck, JogRing, JogRingMode, JogWheelTracker};
pub use crate::core::led::LedValue;
pub use crate::core::seven_segment::SevenSegment;
pub use crate::device::hooks::PacketHook;
pub use crate::screen::GpuContext;

use crate::device::loader::DescriptorRegistry;
use crate::usb::device_thread::{DeviceCmd, DeviceHandle};
use crate::usb::hotplug;

/// Configuration for initializing Encdr.
#[derive(Default)]
pub struct EncdrConfig {
    /// Optional shared GPU context. If None, Encdr creates its own.
    pub gpu: Option<GpuContext>,
    /// Additional descriptor directories to load on startup.
    pub descriptor_dirs: Vec<String>,
    /// Skip loading built-in descriptors.
    pub skip_builtins: bool,
}

/// Main entry point for the Encdr library.
/// Manages device detection, I/O threads, and event delivery.
pub struct Encdr {
    registry: DescriptorRegistry,
    devices: HashMap<DeviceId, DeviceHandle>,
    event_tx: Sender<Event>,
    event_rx: Receiver<Event>,
    gpu: Option<Arc<GpuContext>>,
}

impl Encdr {
    /// Create a new Encdr instance and load device descriptors.
    pub fn new(config: EncdrConfig) -> Result<Self> {
        let mut registry = DescriptorRegistry::new();

        // Load built-in descriptors
        if !config.skip_builtins {
            registry.load_builtins()?;
        }

        // Load additional descriptor directories
        for dir in &config.descriptor_dirs {
            registry.load_dir(dir)?;
        }

        let (event_tx, event_rx) = crossbeam_channel::unbounded();

        let gpu = config.gpu.map(Arc::new);

        Ok(Self {
            registry,
            devices: HashMap::new(),
            event_tx,
            event_rx,
            gpu,
        })
    }

    /// Load additional device descriptors from a directory.
    pub fn load_descriptor_dir(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.registry.load_dir(path)
    }

    /// Load a single JSON descriptor string.
    pub fn load_descriptor_json(&mut self, json: &str) -> Result<Arc<DeviceDescriptor>> {
        self.registry.load_json(json)
    }

    /// Get the event receiver channel. Events are delivered here from all
    /// connected devices. Non-blocking: use `try_recv()` or blocking `recv()`.
    pub fn events(&self) -> &Receiver<Event> {
        &self.event_rx
    }

    /// Scan for connected devices matching loaded descriptors and connect to them.
    /// Emits DeviceConnected events for newly found devices.
    pub fn scan(&mut self) -> Result<Vec<DeviceId>> {
        crate::usb::service_detector::warn_active_services_if_detected();
        let detected = hotplug::scan_devices(&self.registry);
        let mut connected = Vec::new();

        for det in detected {
            if self.devices.contains_key(&det.device_id) {
                continue; // Already connected
            }

            let names = self.registry.intern_descriptor_names(&det.descriptor);

            match DeviceHandle::spawn(
                det.device_id,
                det.descriptor.clone(),
                &det.usb_info,
                names,
                self.event_tx.clone(),
                self.gpu.clone(),
            ) {
                Ok(handle) => {
                    // Emit connected event
                    self.event_tx
                        .send(Event::DeviceConnected {
                            id: det.device_id,
                            descriptor: det.descriptor.clone(),
                        })
                        .ok();

                    self.devices.insert(det.device_id, handle);
                    connected.push(det.device_id);
                    tracing::info!("Connected to: {}", det.descriptor.name);
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to connect to {}: {}",
                        det.descriptor.name,
                        e
                    );
                }
            }
        }

        Ok(connected)
    }

    /// Set an LED on a device by name (matches first LED group containing the name).
    pub fn set_led(&self, device_id: DeviceId, name: &str, value: LedValue) {
        if let Some(handle) = self.devices.get(&device_id) {
            handle.send(DeviceCmd::SetLed {
                name: name.to_string(),
                value,
            });
        }
    }

    /// Set an LED on a device within a specific LED group.
    pub fn set_led_in_group(
        &self,
        device_id: DeviceId,
        group: &str,
        name: &str,
        value: LedValue,
    ) {
        if let Some(handle) = self.devices.get(&device_id) {
            handle.send(DeviceCmd::SetLedInGroup {
                group: group.to_string(),
                name: name.to_string(),
                value,
            });
        }
    }

    /// Set a strip LED array on a device by name (matches first group containing the name).
    pub fn set_led_strip(&self, device_id: DeviceId, name: &str, values: &[u8]) {
        if let Some(handle) = self.devices.get(&device_id) {
            handle.send(DeviceCmd::SetLedStrip {
                name: name.to_string(),
                values: values.to_vec(),
            });
        }
    }

    /// Set a strip LED array on a device within a specific LED group.
    pub fn set_led_strip_in_group(
        &self,
        device_id: DeviceId,
        group: &str,
        name: &str,
        values: &[u8],
    ) {
        if let Some(handle) = self.devices.get(&device_id) {
            handle.send(DeviceCmd::SetLedStripInGroup {
                group: group.to_string(),
                name: name.to_string(),
                values: values.to_vec(),
            });
        }
    }

    /// Set a 7-segment display control on a device by name.
    ///
    /// Writes the raw segment bitmask (bit 0=a .. bit 6=g, bit 7=dp) as a single LED value.
    pub fn set_seven_segment(&self, device_id: DeviceId, name: &str, seg: SevenSegment) {
        self.set_led(device_id, name, LedValue::Single(seg.raw_mask()));
    }

    /// Set a 7-segment display control within a specific LED group.
    pub fn set_seven_segment_in_group(
        &self,
        device_id: DeviceId,
        group: &str,
        name: &str,
        seg: SevenSegment,
    ) {
        self.set_led_in_group(device_id, group, name, LedValue::Single(seg.raw_mask()));
    }

    /// Sets a dual 7-segment display (left and right digits) from a text string.
    ///
    /// Decimal points attached to characters (e.g. `"1."`) are automatically merged.
    pub fn set_seven_segment_str(
        &self,
        device_id: DeviceId,
        digit_left: &str,
        digit_right: &str,
        text: &str,
    ) {
        let encoded = SevenSegment::encode_str(text);
        let left = encoded.first().copied().unwrap_or(SevenSegment::BLANK);
        let right = encoded.get(1).copied().unwrap_or(SevenSegment::BLANK);
        self.set_seven_segment(device_id, digit_left, left);
        self.set_seven_segment(device_id, digit_right, right);
    }

    /// Sets a DJ loop length display (e.g. 32, 16, 8, 4, 2, 1, 0.5, 0.25) across two digits
    /// with an optional active loop dot.
    pub fn set_loop_display(
        &self,
        device_id: DeviceId,
        digit_left: &str,
        digit_right: &str,
        beats: f32,
        active: bool,
    ) {
        self.set_loop_display_with_dot(device_id, digit_left, digit_right, None, beats, active);
    }

    /// Sets a DJ loop length display across two digits, with an optional separate status dot LED.
    pub fn set_loop_display_with_dot(
        &self,
        device_id: DeviceId,
        digit_left: &str,
        digit_right: &str,
        dot_name: Option<&str>,
        beats: f32,
        active: bool,
    ) {
        let [left, right] = SevenSegment::encode_loop_length(beats, active);
        self.set_seven_segment(device_id, digit_left, left);
        self.set_seven_segment(device_id, digit_right, right);
        if let Some(dot) = dot_name {
            self.set_led(device_id, dot, if active { LedValue::Single(255) } else { LedValue::Off });
        }
    }

    /// Sets the jog wheel LED ring on a Traktor Kontrol S4 Mk3 to needle position mode.
    ///
    /// - `deck`: `JogDeck::Left` or `JogDeck::Right`
    /// - `position`: 16-bit needle tick position (0..2879, where 2880 is a full revolution)
    /// - `color`: LED color / brightness (e.g. `LedValue::Rgb`, `LedValue::Single`, or `LedValue::Off`)
    pub fn set_jog_ring_needle(
        &self,
        device_id: DeviceId,
        deck: JogDeck,
        position: u16,
        color: LedValue,
    ) {
        self.set_jog_ring_mode(device_id, deck, JogRingMode::Needle, position, color);
    }

    /// Sets the jog wheel LED ring on a Traktor Kontrol S4 Mk3 to a specific mode.
    ///
    /// Supports `JogRingMode::Off`, `JogRingMode::DimFlash`, `JogRingMode::Needle`,
    /// `JogRingMode::RingFlash`, and `JogRingMode::DimSpot`.
    pub fn set_jog_ring_mode(
        &self,
        device_id: DeviceId,
        deck: JogDeck,
        mode: JogRingMode,
        position: u16,
        color: LedValue,
    ) {
        let (group, deck_idx_name, deck_idx_val, mode_name, pos_lsb, pos_msb, color_name) = match deck {
            JogDeck::Left => (
                "left_wheel_leds",
                "left_wheel_deck_index",
                0,
                "left_wheel_mode",
                "left_wheel_pos_lsb",
                "left_wheel_pos_msb",
                "left_wheel_color",
            ),
            JogDeck::Right => (
                "right_wheel_leds",
                "right_wheel_deck_index",
                1,
                "right_wheel_mode",
                "right_wheel_pos_lsb",
                "right_wheel_pos_msb",
                "right_wheel_color",
            ),
        };

        let pos = position % JogRing::TICKS_PER_REV;
        self.set_led_in_group(device_id, group, deck_idx_name, LedValue::Single(deck_idx_val));
        self.set_led_in_group(device_id, group, mode_name, LedValue::Single(mode as u8));
        self.set_led_in_group(device_id, group, pos_lsb, LedValue::Single((pos & 0xFF) as u8));
        self.set_led_in_group(device_id, group, pos_msb, LedValue::Single(((pos >> 8) & 0xFF) as u8));
        self.set_led_in_group(device_id, group, color_name, color);
    }

    /// Sets the 32 individually addressable LEDs on a Traktor Kontrol S4 Mk3 jog wheel ring.
    ///
    /// Automatically switches the ring into `JogRingMode::Addressable` (Mode 5).
    pub fn set_jog_ring_leds(
        &self,
        device_id: DeviceId,
        deck: JogDeck,
        leds: &[u8],
    ) {
        let (group, deck_idx_name, deck_idx_val, mode_name, ring_name) = match deck {
            JogDeck::Left => ("left_wheel_leds", "left_wheel_deck_index", 0, "left_wheel_mode", "left_wheel_ring"),
            JogDeck::Right => ("right_wheel_leds", "right_wheel_deck_index", 1, "right_wheel_mode", "right_wheel_ring"),
        };

        self.set_led_in_group(device_id, group, deck_idx_name, LedValue::Single(deck_idx_val));
        self.set_led_in_group(device_id, group, mode_name, LedValue::Single(JogRingMode::Addressable as u8));
        self.set_led_strip_in_group(device_id, group, ring_name, leds);
    }

    /// Sets the jog wheel LED ring using a configured `JogRing` buffer.
    pub fn set_jog_ring(&self, device_id: DeviceId, deck: JogDeck, ring: &JogRing) {
        self.set_jog_ring_leds(device_id, deck, ring.as_bytes());
    }

    /// Turns off the jog wheel LED ring for a deck.
    pub fn set_jog_ring_off(&self, device_id: DeviceId, deck: JogDeck) {
        let (group, mode_name) = match deck {
            JogDeck::Left => ("left_wheel_leds", "left_wheel_mode"),
            JogDeck::Right => ("right_wheel_leds", "right_wheel_mode"),
        };
        self.set_led_in_group(device_id, group, mode_name, LedValue::Single(JogRingMode::Off as u8));
    }

    /// Synchronizes the jog wheel LED ring needle spot with an incoming event.
    ///
    /// Handles:
    /// - Absolute position slider events (`"left_jog_pos"`, `"right_jog_pos"`) generated by
    ///   manual wheel turn or motorized platter rotation.
    ///
    /// The ring needle immediately moves in 1:1 physical lockstep with the jog wheel.
    /// Returns `Some((deck, ticks))` if the event was an S4 Mk3 jog wheel event and updated the ring,
    /// or `None` otherwise.
    pub fn sync_jog_ring_from_event(
        &self,
        event: &Event,
        color: LedValue,
    ) -> Option<(JogDeck, u16)> {
        match event {
            Event::Slider { device, name, value } => {
                let deck = if *name == "left_jog_pos" {
                    Some(JogDeck::Left)
                } else if *name == "right_jog_pos" {
                    Some(JogDeck::Right)
                } else {
                    None
                }?;
                let ticks = ((value * JogRing::TICKS_PER_REV as f32).round() as u16)
                    % JogRing::TICKS_PER_REV;
                self.set_jog_ring_needle(*device, deck, ticks, color);
                Some((deck, ticks))
            }
            _ => None,
        }
    }

    /// Synchronizes the jog wheel LED ring needle from a normalized rotation value (0.0 .. 1.0).
    ///
    /// `0.0` represents top dead center (12 o'clock / tick 0), and `1.0` represents one full revolution.
    pub fn sync_jog_ring_normalized(
        &self,
        device_id: DeviceId,
        deck: JogDeck,
        normalized: f32,
        color: LedValue,
    ) {
        let ticks = ((normalized.rem_euclid(1.0) * JogRing::TICKS_PER_REV as f32).round() as u16)
            % JogRing::TICKS_PER_REV;
        self.set_jog_ring_needle(device_id, deck, ticks, color);
    }

    /// Synchronizes the jog wheel LED ring needle from an angle in radians (0.0 .. 2*PI).
    pub fn sync_jog_ring_radians(
        &self,
        device_id: DeviceId,
        deck: JogDeck,
        radians: f32,
        color: LedValue,
    ) {
        let normalized = radians / (2.0 * std::f32::consts::PI);
        self.sync_jog_ring_normalized(device_id, deck, normalized, color);
    }

    /// Synchronizes the jog wheel LED ring needle directly to an absolute tick position (0..2879).
    pub fn sync_jog_ring_position(
        &self,
        device_id: DeviceId,
        deck: JogDeck,
        ticks: u16,
        color: LedValue,
    ) {
        self.set_jog_ring_needle(device_id, deck, ticks, color);
    }


    /// Submit a screen frame. The pixels should be in RGBA8888 format
    /// (or the device's native format to skip conversion).
    pub fn submit_screen(&self, device_id: DeviceId, screen: &str, pixels: &[u8]) {
        self.submit_screen_with_format(device_id, screen, pixels, PixelFormat::Rgba8888);
    }

    /// Submit a screen frame with an explicit pixel format.
    pub fn submit_screen_with_format(
        &self,
        device_id: DeviceId,
        screen: &str,
        pixels: &[u8],
        format: PixelFormat,
    ) {
        if let Some(handle) = self.devices.get(&device_id) {
            handle.send(DeviceCmd::SubmitScreen {
                screen: screen.to_string(),
                pixels: pixels.to_vec(),
                format,
            });
        }
    }

    /// Submit a combined double-width frame for a dual-screen device.
    ///
    /// The pixels should be in RGBA8888 format with dimension
    /// `(left_width + right_width) x height`. Encdr splits the frame
    /// horizontally and feeds each half independently through format conversion,
    /// dirty-rect diffing, and USB transfer.
    pub fn submit_dual_screen(
        &self,
        device_id: DeviceId,
        left_screen: &str,
        right_screen: &str,
        pixels: &[u8],
    ) {
        self.submit_dual_screen_with_format(
            device_id,
            left_screen,
            right_screen,
            pixels,
            PixelFormat::Rgba8888,
        );
    }

    /// Submit a combined double-width frame for a dual-screen device with an explicit pixel format.
    pub fn submit_dual_screen_with_format(
        &self,
        device_id: DeviceId,
        left_screen: &str,
        right_screen: &str,
        pixels: &[u8],
        format: PixelFormat,
    ) {
        if let Some(handle) = self.devices.get(&device_id) {
            handle.send(DeviceCmd::SubmitDualScreen {
                left_screen: left_screen.to_string(),
                right_screen: right_screen.to_string(),
                pixels: pixels.to_vec(),
                format,
            });
        }
    }

    /// Disconnect a specific device.
    pub fn disconnect(&mut self, device_id: DeviceId) {
        if let Some(handle) = self.devices.remove(&device_id) {
            handle.disconnect();
        }
    }

    /// Disconnect all devices and shut down.
    pub fn shutdown(&mut self) {
        let ids: Vec<DeviceId> = self.devices.keys().copied().collect();
        for id in ids {
            self.disconnect(id);
        }
    }

    /// Get the descriptor for a connected device.
    pub fn device_descriptor(&self, device_id: DeviceId) -> Option<&Arc<DeviceDescriptor>> {
        self.devices.get(&device_id).map(|h| &h.descriptor)
    }

    /// List all currently connected device IDs.
    pub fn connected_devices(&self) -> Vec<DeviceId> {
        self.devices.keys().copied().collect()
    }

    /// Get all loaded descriptors (for probing/listing).
    pub fn loaded_descriptors(&self) -> Vec<Arc<DeviceDescriptor>> {
        self.registry.all().cloned().collect()
    }
}

impl Drop for Encdr {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod send_sync_check {
    use super::Encdr;
    fn assert_send_sync<T: Send + Sync>() {}
    #[test]
    fn encdr_is_send_sync() {
        assert_send_sync::<Encdr>();
    }
}
