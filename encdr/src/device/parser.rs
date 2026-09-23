use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::core::descriptor::*;
use crate::core::event::{DeviceId, Event};
use crate::device::encoder::EncoderState;

/// Hardware pad index (0..15, as reported in Report 0x02 tuples) to control name.
const PAD_MAP: [&str; 16] = [
    "pad_13", "pad_14", "pad_15", "pad_16",
    "pad_9",  "pad_10", "pad_11", "pad_12",
    "pad_5",  "pad_6",  "pad_7",  "pad_8",
    "pad_1",  "pad_2",  "pad_3",  "pad_4",
];

/// Generic packet parser driven by a device descriptor.
/// Maintains state for change detection across packets.
pub struct PacketParser {
    device_id: DeviceId,
    /// Interned control names for zero-alloc event emission
    names: HashMap<String, &'static str>,
    /// Per-packet parser state, keyed by packet size
    packet_parsers: HashMap<usize, PacketState>,
    pad_stream_state: Option<PadStreamState>,
    created_at: Instant,
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

const MK2_MEDIAN_WINDOW: usize = 9;

struct PadStreamState {
    pad_states: [bool; 16],
    pad_pressures: [f32; 16],
    /// Last time each pad's tuple was seen in a Report 0x02 packet.
    last_update: [Instant; 16],
    /// Last time a NoteOn strike (0x10) occurred, used to debounce mechanical
    /// rebound/vibration in aftertouch immediately after strike.
    last_strike: [Option<Instant>; 16],
    /// Number of consecutive aftertouch reports seen while pressed.
    /// Used to distinguish sustained holds from single-packet quick taps.
    aftertouch_count: [u32; 16],
    /// Circular buffer for each pad's recent raw ADC samples for Mk2 median filtering
    mk2_history: [[u16; MK2_MEDIAN_WINDOW]; 16],
    mk2_history_idx: [usize; 16],
}

impl Default for PadStreamState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            pad_states: [false; 16],
            pad_pressures: [0.0; 16],
            last_update: [now; 16],
            last_strike: [None; 16],
            aftertouch_count: [0; 16],
            mk2_history: [[0u16; MK2_MEDIAN_WINDOW]; 16],
            mk2_history_idx: [0usize; 16],
        }
    }
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
        let mut has_pad_stream = false;

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

            if packet_desc.id == "pads" {
                has_pad_stream = true;
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

        let pad_stream_state = if has_pad_stream {
            Some(PadStreamState::default())
        } else {
            None
        };

        Self {
            device_id,
            names,
            packet_parsers,
            pad_stream_state,
            created_at: Instant::now(),
        }
    }

    /// Parse a USB packet and return events. Dispatches by packet size or report type.
    /// Events are appended to the provided buffer to avoid allocation.
    pub fn parse(&mut self, buf: &[u8], events: &mut Vec<Event>) {
        // Handle NI Maschine Mk3 pad event stream (Report 0x02)
        if buf.first() == Some(&0x02) {
            tracing::trace!("PAD RAW [len={}]: {:02x?}", buf.len(), &buf[0..buf.len().min(16)]);

            if let Some(ref mut state) = self.pad_stream_state {
                // If the pad report is completely empty (all zeros), release all pads
                let is_all_zeros = buf[1..].iter().all(|&b| b == 0);

                if is_all_zeros {
                    for p in 0..16 {
                        if state.pad_states[p] {
                            state.pad_states[p] = false;
                            state.pad_pressures[p] = 0.0;
                            state.aftertouch_count[p] = 0;
                            let pad_name = *self.names.get(PAD_MAP[p]).unwrap_or(&PAD_MAP[p]);
                            events.push(Event::Button {
                                device: self.device_id,
                                name: pad_name,
                                pressed: false,
                            });
                            events.push(Event::Grid {
                                device: self.device_id,
                                name: pad_name,
                                index: p as u8,
                                pressure: 0.0,
                            });
                        }
                    }
                    return;
                }

                // 128-byte "double-pumped" pad reports are two INDEPENDENT 64-byte
                // sub-messages ("Set A" = buf[0..64], "Set B" = buf[64..128]), each
                // starting with its own leading marker byte (also 0x02) before a stream
                // of up to 21 3-byte (pad_index, d1, d2) tuples. This matches openAV/Ctlra's
                // `ni_maschine_mk3_pads_decode_set`, which decodes each half identically
                // (see ctlra/devices/ni_maschine_mk3.c). Both sets must be scanned: NI's
                // firmware sometimes reports a note-off only in Set B, so scanning just
                // Set A can leave a pad's release event unseen — exactly the "stuck
                // pressed" symptom this loop must avoid. State is carried forward across
                // both sets (each tuple mutates `state` immediately), so Set B's scan
                // naturally builds on whatever Set A already observed.
                let num_chunks = buf.len() / 64;
                let chunk_slices: Vec<&[u8]> = if num_chunks > 0 {
                    (0..num_chunks).map(|c| &buf[c * 64..(c + 1) * 64]).collect()
                } else {
                    vec![buf]
                };

                for (chunk_idx, chunk) in chunk_slices.iter().enumerate() {
                    if chunk.is_empty() || chunk[0] != 0x02 {
                        continue;
                    }
                    for i in 0..16 {
                        let p_idx = 1 + i * 3;
                        if p_idx + 2 >= chunk.len() {
                            break;
                        }
                        let p = chunk[p_idx];
                        let d1 = chunk[p_idx + 1];
                        let d2 = chunk[p_idx + 2];

                        // End of this set's tuple list. Matches NI's own decoder (Ghidra
                        // FUN_1400b4df0 in NIHardwareService.exe): terminator requires ALL
                        // THREE bytes zero. A 2-byte check is too loose — pad_13 (hardware
                        // index 0) with tag 0x00 "Switch ON" and pressure_raw < 256
                        // legitimately produces d1 == 0x00, so d2 must also be checked or a
                        // real tuple gets misread as end-of-list and the rest of the set is
                        // silently dropped.
                        if p == 0 && d1 == 0 && d2 == 0 {
                            break;
                        }

                        if (p as usize) < 16 {
                            let p_usize = p as usize;
                            state.last_update[p_usize] = Instant::now();

                            let evt = d1 & 0xF0;
                            let pressure_raw = (((d1 & 0x0F) as u16) << 8) | (d2 as u16);
                            let prev_pressed = state.pad_states[p_usize];
                            let pad_name = *self.names.get(PAD_MAP[p_usize]).unwrap_or(&PAD_MAP[p_usize]);
                            let t = self.created_at.elapsed().as_secs_f32();

                            eprintln!(
                                "[{:.4}s][PARSER-USB] set={} tuple={}: p={} ({}) d1=0x{:02x} d2=0x{:02x} evt=0x{:02x} raw_p={} prev_pressed={}",
                                t,
                                if chunk_idx == 0 { "A" } else { "B" },
                                i,
                                p,
                                pad_name,
                                d1,
                                d2,
                                evt,
                                pressure_raw,
                                prev_pressed
                            );

                            match evt {
                                // NoteOn (0x10): Pad struck
                                0x10 => {
                                    let pressure = (pressure_raw as f32 / 4095.0).max(0.01);
                                    if !prev_pressed {
                                        state.pad_states[p_usize] = true;
                                        state.aftertouch_count[p_usize] = 0;
                                        eprintln!(
                                            "[{:.4}s][PARSER-EVT] => Event::Button {{ name: {}, pressed: true }} (hit, raw_p={})",
                                            t, pad_name, pressure_raw
                                        );
                                        events.push(Event::Button {
                                            device: self.device_id,
                                            name: pad_name,
                                            pressed: true,
                                        });
                                    }
                                    state.last_strike[p_usize] = Some(Instant::now());
                                    state.pad_pressures[p_usize] = pressure;
                                    events.push(Event::Grid {
                                        device: self.device_id,
                                        name: pad_name,
                                        index: p,
                                        pressure,
                                    });
                                }

                                // NoteOff (0x30) or PressOff (0x20): Pad released unconditionally,
                                // ignoring any residual baseline ADC capacitance/charge.
                                0x30 | 0x20 => {
                                    if prev_pressed {
                                        state.pad_states[p_usize] = false;
                                        state.pad_pressures[p_usize] = 0.0;
                                        state.last_strike[p_usize] = None;
                                        state.aftertouch_count[p_usize] = 0;
                                        eprintln!(
                                            "[{:.4}s][PARSER-EVT] => Event::Button {{ name: {}, pressed: false }} (explicit NoteOff/0x{:02x})",
                                            t, pad_name, evt
                                        );
                                        events.push(Event::Button {
                                            device: self.device_id,
                                            name: pad_name,
                                            pressed: false,
                                        });
                                        events.push(Event::Grid {
                                            device: self.device_id,
                                            name: pad_name,
                                            index: p,
                                            pressure: 0.0,
                                        });
                                    }
                                }

                                // Aftertouch (0x40): Pad held with updating continuous pressure.
                                // Uses hysteresis:
                                // - Press threshold: 32 (responsive to light touches/taps)
                                // - Release threshold: 16 (clean release above idle noise)
                                // - Debounce: ignore release for 30ms after NoteOn to prevent
                                //   mechanical strike vibration from falsely chattering release.
                                0x40 => {
                                    const PAD_PRESS_THRESHOLD: u16 = 32;
                                    const PAD_RELEASE_THRESHOLD: u16 = 16;
                                    const STRIKE_DEBOUNCE_WINDOW: Duration = Duration::from_millis(30);

                                    let in_strike_window = state.last_strike[p_usize]
                                        .map_or(false, |t| t.elapsed() < STRIKE_DEBOUNCE_WINDOW);

                                    if pressure_raw <= PAD_RELEASE_THRESHOLD {
                                        if prev_pressed && !in_strike_window {
                                            state.pad_states[p_usize] = false;
                                            state.pad_pressures[p_usize] = 0.0;
                                            state.last_strike[p_usize] = None;
                                            state.aftertouch_count[p_usize] = 0;
                                            eprintln!(
                                                "[{:.4}s][PARSER-EVT] => Event::Button {{ name: {}, pressed: false }} (pressure {} <= {})",
                                                t, pad_name, pressure_raw, PAD_RELEASE_THRESHOLD
                                            );
                                            events.push(Event::Button {
                                                device: self.device_id,
                                                name: pad_name,
                                                pressed: false,
                                            });
                                            events.push(Event::Grid {
                                                device: self.device_id,
                                                name: pad_name,
                                                index: p,
                                                pressure: 0.0,
                                            });
                                        }
                                    } else if pressure_raw >= PAD_PRESS_THRESHOLD {
                                        let pressure = pressure_raw as f32 / 4095.0;
                                        if !prev_pressed {
                                            state.pad_states[p_usize] = true;
                                            state.aftertouch_count[p_usize] = 1;
                                            eprintln!(
                                                "[{:.4}s][PARSER-EVT] => Event::Button {{ name: {}, pressed: true }} (0x40 press, raw_p={})",
                                                t, pad_name, pressure_raw
                                            );
                                            events.push(Event::Button {
                                                device: self.device_id,
                                                name: pad_name,
                                                pressed: true,
                                            });
                                        } else {
                                            state.aftertouch_count[p_usize] =
                                                state.aftertouch_count[p_usize].saturating_add(1);
                                        }
                                        let pressure_changed = (pressure - state.pad_pressures[p_usize]).abs() > 0.02;
                                        if pressure_changed || !prev_pressed {
                                            state.pad_pressures[p_usize] = pressure;
                                            events.push(Event::Grid {
                                                device: self.device_id,
                                                name: pad_name,
                                                index: p,
                                                pressure,
                                            });
                                        }
                                    } else if prev_pressed {
                                        // Within hysteresis deadband (16..32) while held: retain pressed, update pressure
                                        let pressure = pressure_raw as f32 / 4095.0;
                                        let pressure_changed = (pressure - state.pad_pressures[p_usize]).abs() > 0.02;
                                        if pressure_changed {
                                            state.pad_pressures[p_usize] = pressure;
                                            events.push(Event::Grid {
                                                device: self.device_id,
                                                name: pad_name,
                                                index: p,
                                                pressure,
                                            });
                                        }
                                    }
                                }

                                // Switch ON (0x00): Ghidra (NIHardwareService.exe, FUN_1400b4df0)
                                // confirms tag 0 is NI's own "Pad.Switch.Event" ON — a real press,
                                // parallel to 0x10 "Hit ON" — not purely a noise/rest report as
                                // previously assumed. But hardware also emits low-raw_p tag-0x00
                                // reports while settling after a release, so this arm must not
                                // blindly trust every occurrence (that would reintroduce ghost
                                // presses). Reuses the exact hysteresis already validated for 0x40:
                                // press >= 32, release <= 16, 30ms post-strike release debounce,
                                // 16..32 deadband retains state while updating reported pressure.
                                0x00 => {
                                    const PAD_PRESS_THRESHOLD: u16 = 32;
                                    const PAD_RELEASE_THRESHOLD: u16 = 16;
                                    const STRIKE_DEBOUNCE_WINDOW: Duration = Duration::from_millis(30);

                                    let in_strike_window = state.last_strike[p_usize]
                                        .map_or(false, |t| t.elapsed() < STRIKE_DEBOUNCE_WINDOW);

                                    if pressure_raw <= PAD_RELEASE_THRESHOLD {
                                        if prev_pressed && !in_strike_window {
                                            state.pad_states[p_usize] = false;
                                            state.pad_pressures[p_usize] = 0.0;
                                            state.last_strike[p_usize] = None;
                                            state.aftertouch_count[p_usize] = 0;
                                            eprintln!(
                                                "[{:.4}s][PARSER-EVT] => Event::Button {{ name: {}, pressed: false }} (0x00 pressure {} <= {})",
                                                t, pad_name, pressure_raw, PAD_RELEASE_THRESHOLD
                                            );
                                            events.push(Event::Button {
                                                device: self.device_id,
                                                name: pad_name,
                                                pressed: false,
                                            });
                                            events.push(Event::Grid {
                                                device: self.device_id,
                                                name: pad_name,
                                                index: p,
                                                pressure: 0.0,
                                            });
                                        }
                                    } else if pressure_raw >= PAD_PRESS_THRESHOLD {
                                        let pressure = pressure_raw as f32 / 4095.0;
                                        if !prev_pressed {
                                            state.pad_states[p_usize] = true;
                                            state.aftertouch_count[p_usize] = 0;
                                            state.last_strike[p_usize] = Some(Instant::now());
                                            eprintln!(
                                                "[{:.4}s][PARSER-EVT] => Event::Button {{ name: {}, pressed: true }} (0x00 switch press, raw_p={})",
                                                t, pad_name, pressure_raw
                                            );
                                            events.push(Event::Button {
                                                device: self.device_id,
                                                name: pad_name,
                                                pressed: true,
                                            });
                                        }
                                        let pressure_changed = (pressure - state.pad_pressures[p_usize]).abs() > 0.02;
                                        if pressure_changed || !prev_pressed {
                                            state.pad_pressures[p_usize] = pressure;
                                            events.push(Event::Grid {
                                                device: self.device_id,
                                                name: pad_name,
                                                index: p,
                                                pressure,
                                            });
                                        }
                                    } else if prev_pressed {
                                        // Within hysteresis deadband (16..32) while held: retain pressed, update pressure
                                        let pressure = pressure_raw as f32 / 4095.0;
                                        let pressure_changed = (pressure - state.pad_pressures[p_usize]).abs() > 0.02;
                                        if pressure_changed {
                                            state.pad_pressures[p_usize] = pressure;
                                            events.push(Event::Grid {
                                                device: self.device_id,
                                                name: pad_name,
                                                index: p,
                                                pressure,
                                            });
                                        }
                                    }
                                }

                                // Fallback for any unknown nibble: use flat threshold with hysteresis
                                _ => {
                                    const PAD_PRESS_THRESHOLD: u16 = 64;
                                    const PAD_RELEASE_THRESHOLD: u16 = 48;
                                    if prev_pressed {
                                        if pressure_raw <= PAD_RELEASE_THRESHOLD {
                                            state.pad_states[p_usize] = false;
                                            state.pad_pressures[p_usize] = 0.0;
                                            eprintln!(
                                                "[{:.4}s][PARSER-EVT] => Event::Button {{ name: {}, pressed: false }} (fallback nibble 0x{:02x}, raw_p={})",
                                                t, pad_name, evt, pressure_raw
                                            );
                                            events.push(Event::Button {
                                                device: self.device_id,
                                                name: pad_name,
                                                pressed: false,
                                            });
                                            events.push(Event::Grid {
                                                device: self.device_id,
                                                name: pad_name,
                                                index: p,
                                                pressure: 0.0,
                                            });
                                        }
                                    } else if pressure_raw >= PAD_PRESS_THRESHOLD {
                                        state.pad_states[p_usize] = true;
                                        let pressure = pressure_raw as f32 / 4095.0;
                                        state.pad_pressures[p_usize] = pressure;
                                        eprintln!(
                                            "[{:.4}s][PARSER-EVT] => Event::Button {{ name: {}, pressed: true }} (fallback nibble 0x{:02x}, raw_p={})",
                                            t, pad_name, evt, pressure_raw
                                        );
                                        events.push(Event::Button {
                                            device: self.device_id,
                                            name: pad_name,
                                            pressed: true,
                                        });
                                        events.push(Event::Grid {
                                            device: self.device_id,
                                            name: pad_name,
                                            index: p,
                                            pressure,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                return;
            }
        }

        // Handle NI Maschine Mk2 continuous pad ADC stream (Report 0x20)
        let mk2_pad_bytes = if buf.first() == Some(&0x20) && buf.len() >= 33 {
            Some(&buf[1..33])
        } else if buf.len() == 32 && self.pad_stream_state.is_some() && buf.first() != Some(&0x02) {
            Some(&buf[0..32])
        } else {
            None
        };

        if let Some(pad_data) = mk2_pad_bytes {
            if let Some(ref mut state) = self.pad_stream_state {
                const MK2_PRESS_THRESHOLD: f32 = 48.0 / 4095.0;
                const MK2_RELEASE_THRESHOLD: f32 = 16.0 / 4095.0;

                for p in 0..16 {
                    let offset = p * 2;
                    let raw_val = ((pad_data[offset] as u16) | ((pad_data[offset + 1] as u16) << 8)) & 0x0FFF;

                    // Push sample into sliding median filter
                    let hist_idx = state.mk2_history_idx[p] % MK2_MEDIAN_WINDOW;
                    state.mk2_history[p][hist_idx] = raw_val;
                    state.mk2_history_idx[p] = (state.mk2_history_idx[p] + 1) % MK2_MEDIAN_WINDOW;

                    // Compute median
                    let mut sorted = state.mk2_history[p];
                    sorted.sort_unstable();
                    let median_raw = sorted[MK2_MEDIAN_WINDOW / 2];

                    let pressure = median_raw as f32 / 4095.0;
                    let prev_pressed = state.pad_states[p];
                    let pad_name = *self.names.get(PAD_MAP[p]).unwrap_or(&PAD_MAP[p]);

                    if !prev_pressed {
                        if pressure >= MK2_PRESS_THRESHOLD {
                            state.pad_states[p] = true;
                            state.pad_pressures[p] = pressure;
                            events.push(Event::Button {
                                device: self.device_id,
                                name: pad_name,
                                pressed: true,
                            });
                            events.push(Event::Grid {
                                device: self.device_id,
                                name: pad_name,
                                index: p as u8,
                                pressure,
                            });
                        }
                    } else {
                        if pressure < MK2_RELEASE_THRESHOLD {
                            state.pad_states[p] = false;
                            state.pad_pressures[p] = 0.0;
                            events.push(Event::Button {
                                device: self.device_id,
                                name: pad_name,
                                pressed: false,
                            });
                            events.push(Event::Grid {
                                device: self.device_id,
                                name: pad_name,
                                index: p as u8,
                                pressure: 0.0,
                            });
                        } else {
                            let pressure_changed = (pressure - state.pad_pressures[p]).abs() > 0.02;
                            if pressure_changed {
                                state.pad_pressures[p] = pressure;
                                events.push(Event::Grid {
                                    device: self.device_id,
                                    name: pad_name,
                                    index: p as u8,
                                    pressure,
                                });
                            }
                        }
                    }
                }
                return;
            }
        }

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

    pub fn check_pad_timeouts(&mut self, events: &mut Vec<Event>) {
        let Some(ref mut state) = self.pad_stream_state else {
            return;
        };
        let now = Instant::now();
        for p in 0..16 {
            if !state.pad_states[p] {
                continue;
            }

            let silence_duration = now.duration_since(state.last_update[p]);

            // 1. Isolated tap timeout (e.g. NoteOn strike or single tap packet with NO continuous
            // aftertouch stream and NO explicit release packet received). If aftertouch_count <= 1,
            // it's an isolated tap; if silent for 150ms, synthesize release so synthetic tests or
            // lost release packets cleanly release.
            let is_isolated_tap = state.aftertouch_count[p] <= 1;
            if is_isolated_tap && silence_duration >= Duration::from_millis(150) {
                state.pad_states[p] = false;
                state.pad_pressures[p] = 0.0;
                state.aftertouch_count[p] = 0;
                let pad_name = *self.names.get(PAD_MAP[p]).unwrap_or(&PAD_MAP[p]);
                let t = self.created_at.elapsed().as_secs_f32();
                eprintln!(
                    "[{:.4}s][PARSER-TIMEOUT] Pad {} ({}) isolated tap silence={:.1}ms >= 150ms => RELEASE",
                    t, p, pad_name, silence_duration.as_secs_f32() * 1000.0
                );
                events.push(Event::Button {
                    device: self.device_id,
                    name: pad_name,
                    pressed: false,
                });
                events.push(Event::Grid {
                    device: self.device_id,
                    name: pad_name,
                    index: p as u8,
                    pressure: 0.0,
                });
                continue;
            }

            // 2. Sustained hold watchdog (30 seconds of total silence).
            // Since Maschine Mk3 firmware delta-reports and goes completely silent for seconds
            // during steady holds, normal silence NEVER triggers a release for sustained holds.
            if silence_duration >= Duration::from_secs(30) {
                state.pad_states[p] = false;
                state.pad_pressures[p] = 0.0;
                state.aftertouch_count[p] = 0;
                let pad_name = *self.names.get(PAD_MAP[p]).unwrap_or(&PAD_MAP[p]);
                let t = self.created_at.elapsed().as_secs_f32();
                eprintln!(
                    "[{:.4}s][PARSER-TIMEOUT] Pad {} ({}) watchdog timeout 30s => RELEASE",
                    t, p, pad_name
                );
                events.push(Event::Button {
                    device: self.device_id,
                    name: pad_name,
                    pressed: false,
                });
                events.push(Event::Grid {
                    device: self.device_id,
                    name: pad_name,
                    index: p as u8,
                    pressure: 0.0,
                });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::loader::DescriptorRegistry;

    fn mk3_parser() -> PacketParser {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();
        let desc = reg.find(0x17cc, 0x1600).expect("Maschine Mk3 descriptor missing").clone();
        let names = reg.intern_descriptor_names(&desc);
        PacketParser::new(DeviceId::from_usb(0, 0, 0x17cc, 0x1600), &desc, names)
    }

    fn mk2_parser() -> PacketParser {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();
        let desc = reg.find(0x17cc, 0x1140).expect("Maschine Mk2 descriptor missing").clone();
        let names = reg.intern_descriptor_names(&desc);
        PacketParser::new(DeviceId::from_usb(0, 0, 0x17cc, 0x1140), &desc, names)
    }

    /// Reproduces the exact double-pumped capture documented in
    /// openAV/Ctlra's ni_maschine_mk3.c comments: Set A still shows the pad
    /// held (d1=0x41 d2=0x75, pressure 373) while Set B — the second 64-byte
    /// half of the same 128-byte packet — reports it released (d1=0x30
    /// d2=0x00, pressure 0). A parser that only scans Set A (or that
    /// misaligns Set B's tuples) drops the release and leaves the pad stuck
    /// "pressed" forever.
    #[test]
    fn double_pumped_release_in_set_b_is_not_dropped() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 128];

        // Set A: report ID + one tuple for hardware pad index 1 ("pad_14"), held.
        buf[0] = 0x02;
        buf[1] = 0x01;
        buf[2] = 0x41;
        buf[3] = 0x75;

        // Set B: its own marker byte + the same pad now released.
        buf[64] = 0x02;
        buf[65] = 0x01;
        buf[66] = 0x30;
        buf[67] = 0x00;

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);

        let button_events: Vec<(&str, bool)> = events
            .iter()
            .filter_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            })
            .collect();

        assert_eq!(
            button_events,
            vec![("pad_14", true), ("pad_14", false)],
            "Set A's hit and Set B's release must both be observed, in order"
        );

        let last_grid_pressure = events
            .iter()
            .rev()
            .find_map(|e| match e {
                Event::Grid { name, pressure, .. } if *name == "pad_14" => Some(*pressure),
                _ => None,
            })
            .expect("expected a Grid event for pad_14");
        assert_eq!(last_grid_pressure, 0.0, "final reported pressure must reflect the release");
    }

    /// Reproduces the quick-tap scenario: a quick tap produces exactly one NoteOn
    /// tuple (pad index 15 → "pad_4") and then no further Report 0x02 packet ever
    /// arrives — not even a release. Without `check_pad_timeouts`, this pad would
    /// stay "pressed" forever. The quick-tap path (aftertouch_count == 0) uses a
    /// 150ms timeout for fast release.
    #[test]
    fn quick_tap_with_no_release_packet_times_out() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 128];
        buf[0] = 0x02;
        buf[1] = 0x0f; // Hardware pad index 15 = "pad_4"
        buf[2] = 0x4c; // Real hardware: 0x40 aftertouch nibble + pressure high nibble 0x0c
        buf[3] = 0xec; // pressure low byte = 0xec → pressure_raw = 3308

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);
        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_4", true)),
            "the NoteOn tuple must register as pressed"
        );

        // Quick tap timeout is 150ms (no aftertouch seen → aftertouch_count == 0).
        // Sleep past it so the sweep fires.
        std::thread::sleep(Duration::from_millis(160));
        events.clear();
        parser.check_pad_timeouts(&mut events);

        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_4", false)),
            "a pad that's gone silent must be synthesized as released"
        );
    }

    #[test]
    fn mk3_note_on_and_note_off_with_residual_pressure() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 64];

        // Hardware pad index 2 = "pad_15". Send NoteOn (0x10) with attack velocity.
        buf[0] = 0x02;
        buf[1] = 0x02;
        buf[2] = 0x14; // NoteOn, pressure high nibble = 0x04
        buf[3] = 0x00; // pressure low byte = 0x00 -> pressure_raw = 1024

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);

        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_15", true)),
            "NoteOn (0x10) must register as pressed"
        );

        // Now send NoteOff (0x30), but with non-zero residual pressure (e.g. 0x31 0x20 = raw 288 > 128)
        buf[1] = 0x02;
        buf[2] = 0x31; // NoteOff, residual high nibble = 0x01
        buf[3] = 0x20; // residual low byte = 0x20 -> pressure_raw = 288

        events.clear();
        parser.parse(&buf, &mut events);

        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_15", false)),
            "NoteOff (0x30) must release immediately, ignoring residual sensor pressure"
        );

        let final_pressure = events
            .iter()
            .rev()
            .find_map(|e| match e {
                Event::Grid { name, pressure, .. } if *name == "pad_15" => Some(*pressure),
                _ => None,
            })
            .expect("expected Grid event on release");
        assert_eq!(final_pressure, 0.0);
    }

    #[test]
    fn mk3_aftertouch_noise_floor_releases() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 64];

        // Start held via aftertouch
        buf[0] = 0x02;
        buf[1] = 0x03; // pad_16
        buf[2] = 0x42; // Aftertouch, raw 0x200 = 512
        buf[3] = 0x00;

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);
        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_16", true))
        );

        // Drop below noise floor (<= 16)
        buf[1] = 0x03;
        buf[2] = 0x40; // Aftertouch, raw 10 (<= 16)
        buf[3] = 0x0a;

        events.clear();
        parser.parse(&buf, &mut events);
        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_16", false)),
            "Pressure at noise floor must release pad"
        );
    }

    #[test]
    fn mk3_baseline_0x00_does_not_trigger_ghost_press() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 64];

        // Hardware pad index 12 = "pad_1". Send baseline rest report (0x00) with raw_p = 24.
        buf[0] = 0x02;
        buf[1] = 0x0c; // Hardware pad index 12
        buf[2] = 0x00; // evt = 0x00
        buf[3] = 0x18; // pressure = 24

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);

        assert!(
            !events.iter().any(|e| matches!(e, Event::Button { pressed: true, .. })),
            "Baseline 0x00 report must never trigger a ghost press"
        );
    }

    #[test]
    fn mk3_release_followed_by_baseline_0x00_stays_released() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 64];

        // 1. Strike pad 1 (hw index 12) with NoteOn (0x10)
        buf[0] = 0x02;
        buf[1] = 0x0c;
        buf[2] = 0x11; // 0x10 + high nibble 1 -> raw 256
        buf[3] = 0x00;

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);
        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_1", true)),
            "Initial strike must be pressed"
        );

        // 2. Explicit NoteOff (0x20)
        buf[2] = 0x20;
        buf[3] = 0x00;
        events.clear();
        parser.parse(&buf, &mut events);
        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_1", false)),
            "NoteOff must release"
        );

        // 3. Immediate trailing baseline rest report (0x00 raw_p = 14)
        buf[2] = 0x00;
        buf[3] = 0x0e;
        events.clear();
        parser.parse(&buf, &mut events);
        assert!(
            !events.iter().any(|e| matches!(e, Event::Button { .. })),
            "Trailing 0x00 baseline report must not emit any button events or re-trigger press"
        );
    }

    /// Regression test for the terminator bug: hardware pad index 0 ("pad_13")
    /// sending a real tag-0x00 "Switch ON" tuple with pressure_raw < 256 produces
    /// d1 == 0x00 (tag nibble 0, pressure-high-nibble 0), which a 2-byte
    /// (p==0 && d1==0) terminator check misreads as end-of-list — silently
    /// dropping this tuple AND every tuple after it in the same 64-byte set.
    /// This must fail on the old 2-byte check and pass once the terminator
    /// requires all three bytes (p, d1, d2) to be zero.
    #[test]
    fn mk3_terminator_does_not_swallow_tuple_after_pad13_switch_on_zero_d1() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 64];
        buf[0] = 0x02;

        // Tuple 0: pad_13 (hw index 0), tag 0x00 "Switch ON", pressure_raw = 32
        // (right at the press threshold) -> d1 == 0x00 exactly.
        buf[1] = 0x00;
        buf[2] = 0x00;
        buf[3] = 0x20;

        // Tuple 1: pad_14 (hw index 1), NoteOn, pressure_raw = 1024. Must still
        // be reached and decoded, not swallowed by a false terminator at tuple 0.
        buf[4] = 0x01;
        buf[5] = 0x14;
        buf[6] = 0x00;

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);

        let pressed: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                Event::Button { name, pressed: true, .. } => Some(*name),
                _ => None,
            })
            .collect();

        assert!(pressed.contains(&"pad_13"), "pad_13's own Switch ON tuple must register");
        assert!(
            pressed.contains(&"pad_14"),
            "pad_14's tuple, which follows a (p=0,d1=0,d2!=0) non-terminator tuple, must not be swallowed"
        );
    }

    /// Guards against over-correcting the terminator fix: a genuine
    /// (p=0, d1=0, d2=0) tuple must still stop the scan, so a bogus tuple
    /// placed after it is never decoded.
    #[test]
    fn mk3_true_zero_terminator_still_stops_scan() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 64];
        buf[0] = 0x02;

        // Tuple 0: pad_15 (hw index 2), NoteOn, pressure_raw = 1024.
        buf[1] = 0x02;
        buf[2] = 0x14;
        buf[3] = 0x00;

        // Tuple 1: the true terminator (0, 0, 0).
        buf[4] = 0x00;
        buf[5] = 0x00;
        buf[6] = 0x00;

        // Tuple 2: a well-formed but bogus hit for pad_16 (hw index 3), placed
        // after the terminator — must never be reached.
        buf[7] = 0x03;
        buf[8] = 0x14;
        buf[9] = 0x00;

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);

        assert!(
            events.iter().any(|e| matches!(e, Event::Button { name: "pad_15", pressed: true, .. })),
            "the tuple before the terminator must still be decoded"
        );
        assert!(
            !events.iter().any(|e| matches!(e, Event::Button { name: "pad_16", .. }))
                && !events.iter().any(|e| matches!(e, Event::Grid { name: "pad_16", .. })),
            "no event must be emitted for a tuple placed after the true terminator"
        );
    }

    /// No existing Mk3 Report-0x02 test exercises hardware pad index 0
    /// ("pad_13") at all — the only other pad-index-0 test covers the
    /// unrelated Mk2 decode path. Basic press/release round trip to close
    /// that coverage gap.
    #[test]
    fn mk3_pad13_hit_press_release_round_trip() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 64];
        buf[0] = 0x02;

        // NoteOn (0x10) for pad_13 (hw index 0), pressure_raw = 1024.
        buf[1] = 0x00;
        buf[2] = 0x14;
        buf[3] = 0x00;

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);
        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_13", true)),
            "NoteOn must register pad_13 as pressed"
        );
        let pressure = events
            .iter()
            .find_map(|e| match e {
                Event::Grid { name, pressure, .. } if *name == "pad_13" => Some(*pressure),
                _ => None,
            })
            .expect("expected a Grid event for pad_13");
        assert!((pressure - 1024.0 / 4095.0).abs() < 1e-4);

        // NoteOff (0x30) releases unconditionally.
        buf[2] = 0x30;
        buf[3] = 0x00;
        events.clear();
        parser.parse(&buf, &mut events);
        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_13", false)),
            "NoteOff must release pad_13"
        );
        let final_pressure = events
            .iter()
            .rev()
            .find_map(|e| match e {
                Event::Grid { name, pressure, .. } if *name == "pad_13" => Some(*pressure),
                _ => None,
            })
            .expect("expected a Grid event on release");
        assert_eq!(final_pressure, 0.0);
    }

    /// Direct regression test for the Switch-ON (0x00) suppression bug: with
    /// pressure_raw at/above the press threshold, tag 0x00 must now register
    /// a real press (previously always suppressed).
    #[test]
    fn mk3_switch_on_0x00_press_above_threshold_fires_press() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 64];
        buf[0] = 0x02;

        // Pad index 12 = "pad_1" (same pad as the baseline-suppression test,
        // for direct contrast). tag 0x00, pressure_raw = 40 (>= press threshold 32).
        buf[1] = 0x0c;
        buf[2] = 0x00;
        buf[3] = 0x28;

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);

        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_1", true)),
            "Switch ON (0x00) at or above the press threshold must now register a press"
        );
        let pressure = events
            .iter()
            .find_map(|e| match e {
                Event::Grid { name, pressure, .. } if *name == "pad_1" => Some(*pressure),
                _ => None,
            })
            .expect("expected a Grid event for pad_1");
        assert!((pressure - 40.0 / 4095.0).abs() < 1e-4);
    }

    #[test]
    fn mk3_aftertouch_strike_bounce_does_not_chatter() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 64];

        // 1. Strike pad 1 with NoteOn (0x10 raw_p = 171)
        buf[0] = 0x02;
        buf[1] = 0x0c;
        buf[2] = 0x10;
        buf[3] = 0xab;

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);
        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_1", true))
        );

        // 2. Immediately in the same millisecond, mechanical bounce dip in aftertouch (0x40 raw_p = 35)
        buf[2] = 0x40;
        buf[3] = 0x23; // raw 35 <= 48
        events.clear();
        parser.parse(&buf, &mut events);

        assert!(
            !events.iter().any(|e| matches!(e, Event::Button { pressed: false, .. })),
            "Mechanical rebound within 30ms of strike must not falsely release pad"
        );
    }

    #[test]
    fn mk2_continuous_stream_with_median_filter_and_hysteresis() {
        let mut parser = mk2_parser();
        let mut buf = [0u8; 33];
        buf[0] = 0x20; // Report ID 0x20

        // Pad index 0 is "pad_13" (bytes 1 and 2 in payload)
        // 1. Single spike of 800 (noise) followed by zeros
        buf[1] = 0x20; // 0x0320 = 800
        buf[2] = 0x03;

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);
        assert!(
            !events.iter().any(|e| matches!(e, Event::Button { pressed: true, .. })),
            "Single noise spike must be rejected by median filter"
        );

        // Reset buffer to 0
        buf[1] = 0x00;
        buf[2] = 0x00;
        for _ in 0..5 {
            events.clear();
            parser.parse(&buf, &mut events);
        }

        // 2. Consistent press above threshold (e.g. 5 consecutive readings of 400)
        buf[1] = 0x90; // 0x0190 = 400
        buf[2] = 0x01;
        let mut pressed = false;
        for _ in 0..5 {
            events.clear();
            parser.parse(&buf, &mut events);
            if events.iter().any(|e| matches!(e, Event::Button { name: "pad_13", pressed: true, .. })) {
                pressed = true;
            }
        }
        assert!(pressed, "Consecutive genuine readings must trigger press");

        // 3. Drop to below release threshold (e.g. 10)
        buf[1] = 0x0a;
        buf[2] = 0x00;
        let mut released = false;
        for _ in 0..5 {
            events.clear();
            parser.parse(&buf, &mut events);
            if events.iter().any(|e| matches!(e, Event::Button { name: "pad_13", pressed: false, .. })) {
                released = true;
            }
        }
        assert!(released, "Dropping below release threshold must release pad");
    }

    #[test]
    fn mk3_sustained_hold_does_not_time_out_on_silence() {
        let mut parser = mk3_parser();
        let mut buf = [0u8; 64];

        // 1. Initial NoteOn strike (0x10)
        buf[0] = 0x02;
        buf[1] = 0x0f; // pad_4
        buf[2] = 0x11; // raw 256
        buf[3] = 0x00;

        let mut events = Vec::new();
        parser.parse(&buf, &mut events);
        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_4", true)),
            "NoteOn strike registers pressed"
        );

        // 2. Stream several aftertouch (0x40) reports with sustained pressure (raw 400)
        buf[2] = 0x41;
        buf[3] = 0x90;
        for _ in 0..5 {
            events.clear();
            parser.parse(&buf, &mut events);
        }

        // 3. User holds pad steady: USB bus goes completely silent for 200ms (> 150ms timeout)
        std::thread::sleep(Duration::from_millis(200));
        events.clear();
        parser.check_pad_timeouts(&mut events);

        // Assert: NO release event must be emitted! The pad must remain pressed!
        assert!(
            !events.iter().any(|e| matches!(e, Event::Button { name: "pad_4", pressed: false, .. })),
            "Sustained hold must NOT be released by 150ms silence timeout"
        );

        // 4. User lifts finger: Hardware sends explicit NoteOff (0x20)
        buf[2] = 0x20;
        buf[3] = 0x00;
        events.clear();
        parser.parse(&buf, &mut events);

        assert_eq!(
            events.iter().find_map(|e| match e {
                Event::Button { name, pressed, .. } => Some((*name, *pressed)),
                _ => None,
            }),
            Some(("pad_4", false)),
            "Explicit NoteOff (0x20) releases pad cleanly"
        );
    }
}

