use std::collections::HashMap;

use crate::core::descriptor::*;
use crate::core::event::{DeviceId, Event};
use crate::device::encoder::EncoderState;

/// Generic packet parser driven by a device descriptor.
/// Maintains state for change detection across packets.
pub struct PacketParser {
    device_id: DeviceId,
    /// Interned control names for zero-alloc event emission
    names: HashMap<String, &'static str>,
    /// Per-packet parser state, keyed by packet size
    packet_parsers: HashMap<usize, PacketState>,
}

/// State for a single input packet type
struct PacketState {
    /// Previous button/touch bitmask values for change detection
    button_states: Vec<ButtonState>,
    /// Multi-byte touch states (e.g. touchstrip touch)
    wide_touch_states: Vec<WideTouchState>,
    /// Encoder states (wrap16 type)
    encoder_states: HashMap<String, EncoderState>,
    /// Fine encoder states (signed16 bit type)
    fine_encoder_states: HashMap<String, EncoderState>,
    /// Slider states
    slider_states: HashMap<String, EncoderState>,
    /// The item descriptors for this packet
    items: Vec<InputItemDesc>,
}

struct ButtonState {
    name: String,
    byte: usize,
    mask: u8,
    prev: bool,
    is_touch: bool,
}

/// State for multi-byte touch detection (e.g. touchstrip: 16-bit value > 0)
struct WideTouchState {
    name: String,
    bytes: Vec<usize>,
    prev: bool,
}

impl PacketParser {
    pub fn new(
        device_id: DeviceId,
        descriptor: &DeviceDescriptor,
        names: HashMap<String, &'static str>,
    ) -> Self {
        let mut packet_parsers = HashMap::new();

        for packet_desc in &descriptor.input_packets {
            let mut button_states = Vec::new();
            let mut wide_touch_states = Vec::new();
            let mut encoder_states = HashMap::new();
            let mut fine_encoder_states = HashMap::new();
            let mut slider_states = HashMap::new();

            for item in &packet_desc.items {
                match item {
                    InputItemDesc::Button(b) => {
                        button_states.push(ButtonState {
                            name: b.name.clone(),
                            byte: b.byte,
                            mask: b.mask.0 as u8,
                            prev: false,
                            is_touch: false,
                        });
                    }
                    InputItemDesc::Touch(t) => {
                        if let Some(ref bytes) = t.bytes {
                            // Multi-byte touch (e.g. touchstrip: 16-bit value > 0)
                            wide_touch_states.push(WideTouchState {
                                name: t.name.clone(),
                                bytes: bytes.clone(),
                                prev: false,
                            });
                        } else if let Some(byte) = t.byte {
                            // Single-byte bitmask touch
                            let mask = t.mask.map(|m| m.0 as u8).unwrap_or(0xFF);
                            button_states.push(ButtonState {
                                name: t.name.clone(),
                                byte,
                                mask,
                                prev: false,
                                is_touch: true,
                            });
                        }
                    }
                    InputItemDesc::Encoder(e) => {
                        encoder_states.insert(e.name.clone(), EncoderState::default());
                    }
                    InputItemDesc::EncoderFine(e) => {
                        fine_encoder_states.insert(e.name.clone(), EncoderState::default());
                    }
                    InputItemDesc::Slider(s) => {
                        slider_states.insert(s.name.clone(), EncoderState::default());
                    }
                }
            }

            packet_parsers.insert(
                packet_desc.size,
                PacketState {
                    button_states,
                    wide_touch_states,
                    encoder_states,
                    fine_encoder_states,
                    slider_states,
                    items: packet_desc.items.clone(),
                },
            );
        }

        Self {
            device_id,
            names,
            packet_parsers,
        }
    }

    /// Parse a USB packet and return events. Dispatches by packet size.
    /// Events are appended to the provided buffer to avoid allocation.
    pub fn parse(&mut self, buf: &[u8], events: &mut Vec<Event>) {
        let Some(state) = self.packet_parsers.get_mut(&buf.len()) else {
            return;
        };

        // Parse buttons and single-byte touch sensors
        for btn in &mut state.button_states {
            if btn.byte >= buf.len() {
                continue;
            }
            let pressed = (buf[btn.byte] & btn.mask) != 0;
            if pressed != btn.prev {
                btn.prev = pressed;
                if let Some(&name) = self.names.get(&btn.name) {
                    if btn.is_touch {
                        events.push(Event::Touch {
                            device: self.device_id,
                            name,
                            touched: pressed,
                        });
                    } else {
                        events.push(Event::Button {
                            device: self.device_id,
                            name,
                            pressed,
                        });
                    }
                }
            }
        }

        // Parse multi-byte touch sensors (e.g. touchstrip: any non-zero byte = touched)
        for wt in &mut state.wide_touch_states {
            let touched = wt.bytes.iter().any(|&b| b < buf.len() && buf[b] != 0);
            if touched != wt.prev {
                wt.prev = touched;
                if let Some(&name) = self.names.get(&wt.name) {
                    events.push(Event::Touch {
                        device: self.device_id,
                        name,
                        touched,
                    });
                }
            }
        }

        // Parse other items from the descriptor
        for item in &state.items {
            match item {
                InputItemDesc::Encoder(desc) => {
                    if desc.byte >= buf.len() {
                        continue;
                    }
                    let raw = (buf[desc.byte] >> desc.bit_offset) & ((1 << desc.bits) - 1);
                    if let Some(enc_state) = state.encoder_states.get_mut(&desc.name) {
                        match desc.encoding {
                            EncoderEncoding::Wrap16 => {
                                if let Some(delta) = enc_state.update_wrap16(raw) {
                                    if let Some(&name) = self.names.get(&desc.name) {
                                        events.push(Event::Encoder {
                                            device: self.device_id,
                                            name,
                                            delta,
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                InputItemDesc::EncoderFine(desc) => {
                    if desc.bytes.len() < 2
                        || desc.bytes[0] >= buf.len()
                        || desc.bytes[1] >= buf.len()
                    {
                        continue;
                    }
                    // Little-endian 16-bit: low byte first, high byte second
                    let val =
                        (buf[desc.bytes[1]] as u16) << 8 | buf[desc.bytes[0]] as u16;
                    if let Some(enc_state) = state.fine_encoder_states.get_mut(&desc.name) {
                        match desc.encoding {
                            EncoderEncoding::Signed16 => {
                                if let Some(raw_delta) = enc_state.update_signed16(val) {
                                    let delta = raw_delta / desc.scale;
                                    if let Some(&name) = self.names.get(&desc.name) {
                                        events.push(Event::EncoderFine {
                                            device: self.device_id,
                                            name,
                                            delta,
                                        });
                                    }
                                }
                            }
                            EncoderEncoding::Wrap16Wide => {
                                if let Some(raw_delta) = enc_state.update_wrap16_wide(val) {
                                    let delta = raw_delta as f32 / desc.scale;
                                    if let Some(&name) = self.names.get(&desc.name) {
                                        events.push(Event::EncoderFine {
                                            device: self.device_id,
                                            name,
                                            delta,
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                InputItemDesc::Slider(desc) => {
                    let Some(raw_val) = read_slider_value(buf, desc) else { continue };
                    if let Some(sl_state) = state.slider_states.get_mut(&desc.name) {
                        if let Some(_changed) = sl_state.update_slider(raw_val) {
                            let value = if desc.normalize {
                                let max = desc
                                    .max_value
                                    .unwrap_or((1u32 << desc.bits) - 1);
                                raw_val as f32 / max as f32
                            } else {
                                raw_val as f32
                            };
                            if let Some(&name) = self.names.get(&desc.name) {
                                events.push(Event::Slider {
                                    device: self.device_id,
                                    name,
                                    value: value.clamp(0.0, 1.0),
                                });
                            }
                        }
                    }
                }
                // Buttons and Touch are handled above in the button_states loop
                InputItemDesc::Button(_) | InputItemDesc::Touch(_) => {}
            }
        }
    }
}

/// Read a slider's raw value from the packet buffer.
fn read_slider_value(buf: &[u8], desc: &SliderItemDesc) -> Option<u16> {
    if let Some(ref bytes) = desc.bytes {
        if bytes.len() >= 2 && bytes[0] < buf.len() && bytes[1] < buf.len() {
            // Little-endian: low byte at bytes[0], high byte at bytes[1]
            let val = (buf[bytes[1]] as u16) << 8 | buf[bytes[0]] as u16;
            let mask = if desc.bits < 16 {
                (1u16 << desc.bits) - 1
            } else {
                0xFFFF
            };
            Some(val & mask)
        } else {
            None
        }
    } else if let Some(byte) = desc.byte {
        if byte < buf.len() {
            Some(buf[byte] as u16)
        } else {
            None
        }
    } else {
        None
    }
}
