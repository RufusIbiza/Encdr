//! Direct DAW Remote MIDI and SysEx implementation for NI Komplete Kontrol S-Series Mk3.
//!
//! Provides typed builders and incoming event parsers for the dedicated
//! `"KONTROL S-Series MK3 DAW"` MIDI port (MIDI Channel 16 / Status byte `0xBF`).
//!
//! # Features
//! - **Handshake & Session:** Hello, 14-bit SysEx mode enable, Goodbye, and Surface Configuration.
//! - **Mixer Channels:** Track enabled, selected, mute, solo, arm, track names, hex colors, and stereo VU meters.
//! - **Volume & Pan:** Live formatted string readouts (`"-6.0 dB"`, `"L 25"`, etc.).
//! - **Plugin & Parameters:** Plugin chain lists, parameter labels, display values, and page indicators.
//! - **Incoming Events:** Parses 14-bit relative knob adjustments, button states, navigation, and tempo.

/// MIDI channel used by Komplete Kontrol Mk3 DAW integration (Channel 16, 0-indexed 15).
pub const MIDI_CHANNEL: u8 = 0x0F;

/// MIDI Control Change status byte for Channel 16 (`0xB0 | 0x0F`).
pub const STATUS_CC: u8 = 0xBF;

/// DAW Remote protocol version.
pub const PROTOCOL_VERSION: u8 = 0x04;

/// Handshake message sent on session start (`[0xBF, 0x01, 0x04]`).
pub const HELLO_MESSAGE: [u8; 3] = [STATUS_CC, 0x01, PROTOCOL_VERSION];

/// Command to enable high-resolution 14-bit SysEx parameter adjustments (`[0xBF, 0x06, 0x01]`).
pub const USE_14BIT_SYSEX_UPDATES: [u8; 3] = [STATUS_CC, 0x06, 0x01];

/// Goodbye message sent on session termination (`[0xBF, 0x02, 0x00]`).
pub const GOODBYE_MESSAGE: [u8; 3] = [STATUS_CC, 0x02, 0x00];

/// Universal SysEx manufacturer header for Komplete Kontrol Mk3 DAW protocol.
pub const SYSEX_HEADER: [u8; 10] = [0xF0, 0x00, 0x21, 0x09, 0x00, 0x00, 0x44, 0x43, 0x01, 0x00];

/// End of SysEx byte.
pub const SYSEX_END: u8 = 0xF7;

/// SysEx command IDs used in the DAW Remote protocol.
pub mod sysex_ids {
    pub const SURFACE_CONFIGURATION: u8 = 0x03;
    pub const IDENTITY: u8 = 0x07;
    pub const SET_TEMPO: u8 = 0x19;
    pub const TRACK_ENABLED: u8 = 0x40;
    pub const FOCUS_FOLLOW: u8 = 0x41;
    pub const TRACK_SELECTED: u8 = 0x42;
    pub const MUTE_BUTTON: u8 = 0x43;
    pub const SOLO_BUTTON: u8 = 0x44;
    pub const TRACK_ARMED: u8 = 0x45;
    pub const VOLUME: u8 = 0x46;
    pub const PAN: u8 = 0x47;
    pub const TRACK_NAME: u8 = 0x48;
    pub const VU_METER: u8 = 0x49;
    pub const TRACK_COLOR: u8 = 0x4B;
    pub const SELECTED_TRACK_SELECT_PLUGIN: u8 = 0x70;
    pub const SELECTED_TRACK_PLUGIN_CHAIN_INFO: u8 = 0x71;
    pub const SELECTED_TRACK_PARAM_NAME: u8 = 0x72;
    pub const SELECTED_TRACK_PARAM_DISPLAY_VALUE: u8 = 0x73;
    pub const SELECTED_TRACK_PARAM_PAGE_NUM: u8 = 0x74;
    pub const ADJUST_PARAMETER_VALUE: u8 = 0x7F;
}

/// Control Change numbers for transport and mode buttons (Channel 16).
pub mod cc_buttons {
    pub const SHIFT: u8 = 0x04;
    pub const PLAY: u8 = 0x10;
    pub const RECORD: u8 = 0x12;
    pub const COUNT_IN: u8 = 0x13;
    pub const STOP: u8 = 0x14;
    pub const LOOP: u8 = 0x16;
    pub const METRONOME: u8 = 0x17;
    pub const TAP_TEMPO: u8 = 0x18;
    pub const UNDO: u8 = 0x20;
    pub const REDO: u8 = 0x21;
    pub const QUANTIZE: u8 = 0x22;
    pub const AUTO: u8 = 0x23;
}

/// Control Change numbers for navigation and 4D encoder relative gestures (Channel 16).
pub mod cc_nav {
    pub const BANK_MAPPING: u8 = 0x05;
    pub const TRACK: u8 = 0x30;
    pub const BANK: u8 = 0x31;
    pub const CLIP: u8 = 0x32;
    pub const MOVE_TRANSPORT: u8 = 0x34;
    pub const MOVE_LOOP: u8 = 0x35;
}

/// Control Change numbers for mixer strips and parameters (Channel 16).
pub mod cc_mixer {
    pub const TRACK_SELECT: u8 = 0x42;
    pub const TRACK_MUTE: u8 = 0x43;
    pub const TRACK_SOLO: u8 = 0x44;
    pub const SELECTED_TRACK_VOLUME: u8 = 0x64;
    pub const SELECTED_TRACK_PAN: u8 = 0x65;
    pub const VOLUME_BASE: u8 = 0x50; // 0x50..0x57 (Tracks 0..7)
    pub const PAN_BASE: u8 = 0x58;    // 0x58..0x5F (Tracks 0..7)
    pub const PARAM_BASE: u8 = 0x70;  // 0x70..0x77 (Knobs 0..7)
}

/// Bank mapping mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankMappingMode {
    MixerTracks,
    PluginParameters,
}

/// Group targeted by a 14-bit parameter adjustment (`0x7F`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnobGroup {
    MixerVolume,
    MixerPan,
    TrackParameter,
    Unknown(u8),
}

/// Parsed event incoming from the Komplete Kontrol Mk3 DAW port.
#[derive(Debug, Clone, PartialEq)]
pub enum KkMk3DawEvent {
    /// Button press or release on Channel 16.
    Button {
        cc: u8,
        name: &'static str,
        value: u8,
        pressed: bool,
    },
    /// Navigation or 4D encoder relative movement (relative 2's complement delta).
    Navigation {
        cc: u8,
        name: &'static str,
        delta: i8,
    },
    /// Bank mapping mode switched between mixer tracks and plugin parameters.
    BankMapping(BankMappingMode),
    /// Selected track volume fine relative delta.
    SelectedTrackVolumeDelta(i8),
    /// Selected track pan fine relative delta.
    SelectedTrackPanDelta(i8),
    /// 14-bit high-resolution relative parameter adjustment received via SysEx `0x7F`.
    KnobAdjustment14Bit {
        group: KnobGroup,
        index: u8,
        delta: f32,
    },
    /// Selected plugin chain slot switched on the hardware (`0x70`).
    PluginSelected { chain_index: u8 },
    /// Hardware requested a tempo change via tap / encoder (`0x19`).
    TempoChangeBpm(f32),
    /// Raw unhandled SysEx message.
    OtherSysEx(Vec<u8>),
}

/// High-level builder and parser for the Komplete Kontrol Mk3 DAW remote protocol.
#[derive(Debug, Default, Clone)]
pub struct KkMk3DawController;

impl KkMk3DawController {
    /// Create a new DAW remote controller.
    pub fn new() -> Self {
        Self
    }

    // ── Session & Setup ──────────────────────────────────────────────────

    /// Handshake greeting message sent to the DAW port to activate DAW mode.
    pub fn build_hello(&self) -> [u8; 3] {
        HELLO_MESSAGE
    }

    /// Enable 14-bit high resolution SysEx parameter adjustments for smooth rotary knob tracking.
    pub fn build_enable_14bit(&self) -> [u8; 3] {
        USE_14BIT_SYSEX_UPDATES
    }

    /// Goodbye message sent to deactivate DAW mode and restore standalone hardware state.
    pub fn build_goodbye(&self) -> [u8; 3] {
        GOODBYE_MESSAGE
    }

    /// Identity handshake message identifying the host DAW and protocol version.
    pub fn build_identity(&self, app_name: &str, major_ver: u8, minor_ver: u8) -> Vec<u8> {
        Self::build_sysex_str(sysex_ids::IDENTITY, major_ver, minor_ver, app_name)
    }

    /// Configure surface orientation (e.g. enabling vertical track navigation).
    pub fn build_surface_configuration_vertical(&self) -> Vec<u8> {
        Self::build_sysex_str(sysex_ids::SURFACE_CONFIGURATION, 1, 0, "track_orientation")
    }

    // ── Track Strips & Mixer ─────────────────────────────────────────────

    /// Enable or disable a track slot (0..7).
    pub fn build_track_enabled(&self, track_idx: u8, enabled: bool) -> Vec<u8> {
        Self::build_sysex_raw(sysex_ids::TRACK_ENABLED, if enabled { 1 } else { 0 }, track_idx, &[])
    }

    /// Set whether a track slot (0..7) is currently selected.
    pub fn build_track_selected(&self, track_idx: u8, selected: bool) -> Vec<u8> {
        Self::build_sysex_raw(sysex_ids::TRACK_SELECTED, if selected { 1 } else { 0 }, track_idx, &[])
    }

    /// Set mute button state for a track slot (0..7).
    pub fn build_track_mute(&self, track_idx: u8, muted: bool) -> Vec<u8> {
        Self::build_sysex_raw(sysex_ids::MUTE_BUTTON, if muted { 1 } else { 0 }, track_idx, &[])
    }

    /// Set solo button state for a track slot (0..7).
    pub fn build_track_solo(&self, track_idx: u8, soloed: bool) -> Vec<u8> {
        Self::build_sysex_raw(sysex_ids::SOLO_BUTTON, if soloed { 1 } else { 0 }, track_idx, &[])
    }

    /// Set record armed / enable state for a track slot (0..7).
    pub fn build_track_armed(&self, track_idx: u8, armed: bool) -> Vec<u8> {
        Self::build_sysex_raw(sysex_ids::TRACK_ARMED, if armed { 1 } else { 0 }, track_idx, &[])
    }

    /// Set the displayed name of a track slot (0..7).
    pub fn build_track_name(&self, track_idx: u8, name: &str) -> Vec<u8> {
        Self::build_sysex_str(sysex_ids::TRACK_NAME, 0, track_idx, name)
    }

    /// Set the track color in `#AARRGGBB` hex format (e.g. `"#FF0088FF"`).
    pub fn build_track_color(&self, track_idx: u8, hex_color: &str) -> Vec<u8> {
        Self::build_sysex_str(sysex_ids::TRACK_COLOR, 0, track_idx, hex_color)
    }

    /// Set the track color from RGBA floats (`0.0..1.0`).
    pub fn build_track_color_rgba(&self, track_idx: u8, r: f32, g: f32, b: f32, a: f32) -> Vec<u8> {
        let to_hex = |v: f32| -> String {
            let byte = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            format!("{:02x}", byte)
        };
        let hex = format!("#{}{}{}{}", to_hex(a), to_hex(r), to_hex(g), to_hex(b));
        self.build_track_color(track_idx, &hex)
    }

    /// Set the formatted volume display text string (e.g. `"-6.0 dB"`, `"+2.5 dB"`).
    pub fn build_volume_display(&self, track_idx: u8, display_str: &str) -> Vec<u8> {
        Self::build_sysex_str(sysex_ids::VOLUME, 0, track_idx, display_str)
    }

    /// Set the formatted pan display text string (e.g. `"C"`, `"L 25"`, `"R 50"`).
    pub fn build_pan_display(&self, track_idx: u8, display_str: &str) -> Vec<u8> {
        Self::build_sysex_str(sysex_ids::PAN, 0, track_idx, display_str)
    }

    /// Set 8-channel stereo VU level meters.
    ///
    /// Takes slices of 8 Left and 8 Right decibel levels (typically `-70.0 dB` to `+6.0 dB`).
    /// Encodes them into 8 pairs of 7-bit values (`16` raw bytes total).
    pub fn build_vu_meters(&self, left_db: &[f32; 8], right_db: &[f32; 8]) -> Vec<u8> {
        let mut vu_bytes = [0u8; 16];
        for i in 0..8 {
            vu_bytes[2 * i] = encode_7bit_decibel(left_db[i]);
            vu_bytes[2 * i + 1] = encode_7bit_decibel(right_db[i]);
        }
        Self::build_sysex_raw(sysex_ids::VU_METER, 2, 0, &vu_bytes)
    }

    // ── Plugin Chains & Parameters ───────────────────────────────────────

    /// Transmit the current track's plugin chain as a null-delimited list of plugin names.
    pub fn build_plugin_chain_info(&self, plugin_names: &[&str]) -> Vec<u8> {
        let joined = plugin_names.join("\0");
        Self::build_sysex_str(sysex_ids::SELECTED_TRACK_PLUGIN_CHAIN_INFO, 0, 0, &joined)
    }

    /// Select which plugin in the chain is currently active on the display.
    pub fn build_select_plugin(&self, chain_index: u8) -> Vec<u8> {
        Self::build_sysex_raw(sysex_ids::SELECTED_TRACK_SELECT_PLUGIN, 0, chain_index, &[])
    }

    /// Set parameter name label for a knob slot (0..7).
    pub fn build_parameter_name(&self, knob_idx: u8, name: &str) -> Vec<u8> {
        Self::build_sysex_str(sysex_ids::SELECTED_TRACK_PARAM_NAME, 0, knob_idx, name)
    }

    /// Set parameter formatted value display string for a knob slot (0..7).
    pub fn build_parameter_display_value(&self, knob_idx: u8, value_str: &str) -> Vec<u8> {
        Self::build_sysex_str(sysex_ids::SELECTED_TRACK_PARAM_DISPLAY_VALUE, 0, knob_idx, value_str)
    }

    /// Update parameter bank page indicators.
    pub fn build_parameter_page_info(&self, total_pages: u8, current_page_idx: u8) -> Vec<u8> {
        Self::build_sysex_raw(sysex_ids::SELECTED_TRACK_PARAM_PAGE_NUM, total_pages, current_page_idx, &[])
    }

    /// Set project tempo in BPM. Encoded as 10-nanosecond units per beat.
    pub fn build_tempo_bpm(&self, bpm: f32) -> Vec<u8> {
        let clamped_bpm = bpm.clamp(20.0, 999.0);
        let duration: u64 = (60.0e8 / (clamped_bpm as f64)).round() as u64;
        let bytes = [
            (duration & 0x7f) as u8,
            ((duration >> 7) & 0x7f) as u8,
            ((duration >> 14) & 0x7f) as u8,
            ((duration >> 21) & 0x7f) as u8,
            ((duration >> 28) & 0x7f) as u8,
        ];
        Self::build_sysex_raw(sysex_ids::SET_TEMPO, 0, 0, &bytes)
    }

    // ── Incoming Event Parser ────────────────────────────────────────────

    /// Parse an incoming raw MIDI message buffer (CC or SysEx) from the DAW port.
    pub fn parse_incoming(&self, bytes: &[u8]) -> Option<KkMk3DawEvent> {
        if bytes.is_empty() {
            return None;
        }

        // SysEx check
        if bytes.starts_with(&SYSEX_HEADER) && bytes.ends_with(&[SYSEX_END]) {
            return self.parse_sysex(bytes);
        }

        // Channel 16 CC check
        if bytes.len() >= 3 && bytes[0] == STATUS_CC {
            return self.parse_cc(bytes[1], bytes[2]);
        }

        None
    }

    fn parse_sysex(&self, bytes: &[u8]) -> Option<KkMk3DawEvent> {
        let header_len = SYSEX_HEADER.len();
        if bytes.len() <= header_len + 1 {
            return None;
        }

        let cmd = bytes[header_len];

        match cmd {
            sysex_ids::ADJUST_PARAMETER_VALUE => {
                // Minimum length: header(10) + cmd(1) + group(1) + index(1) + lsb(1) + msb(1) + 0xF7(1) = 16 bytes
                if bytes.len() >= 16 {
                    let group_raw = bytes[11];
                    let index = bytes[12];
                    let lsb = bytes[13];
                    let msb = bytes[14];
                    let delta = decode_14bit_delta(lsb, msb);

                    let group = match group_raw {
                        0x00 => KnobGroup::MixerVolume,
                        0x01 => KnobGroup::MixerPan,
                        0x02 => KnobGroup::TrackParameter,
                        other => KnobGroup::Unknown(other),
                    };

                    return Some(KkMk3DawEvent::KnobAdjustment14Bit { group, index, delta });
                }
            }
            sysex_ids::SELECTED_TRACK_SELECT_PLUGIN => {
                if bytes.len() >= 13 {
                    let chain_index = bytes[12];
                    return Some(KkMk3DawEvent::PluginSelected { chain_index });
                }
            }
            sysex_ids::SET_TEMPO => {
                if bytes.len() >= 18 {
                    let mut duration: u64 = 0;
                    for (i, &b) in bytes[13..18].iter().enumerate() {
                        duration += (b as u64) << (7 * i);
                    }
                    if duration > 0 {
                        let bpm = (60.0e8 / (duration as f64)) as f32;
                        return Some(KkMk3DawEvent::TempoChangeBpm(bpm));
                    }
                }
            }
            _ => return Some(KkMk3DawEvent::OtherSysEx(bytes.to_vec())),
        }

        None
    }

    fn parse_cc(&self, cc: u8, val: u8) -> Option<KkMk3DawEvent> {
        let decode_rel = |v: u8| -> i8 {
            if v >= 64 {
                (v as i16 - 128) as i8
            } else {
                v as i8
            }
        };

        match cc {
            cc_buttons::SHIFT => Some(KkMk3DawEvent::Button { cc, name: "shift", value: val, pressed: val > 0 }),
            cc_buttons::PLAY => Some(KkMk3DawEvent::Button { cc, name: "play", value: val, pressed: val > 0 }),
            cc_buttons::RECORD => Some(KkMk3DawEvent::Button { cc, name: "record", value: val, pressed: val > 0 }),
            cc_buttons::COUNT_IN => Some(KkMk3DawEvent::Button { cc, name: "count_in", value: val, pressed: val > 0 }),
            cc_buttons::STOP => Some(KkMk3DawEvent::Button { cc, name: "stop", value: val, pressed: val > 0 }),
            cc_buttons::LOOP => Some(KkMk3DawEvent::Button { cc, name: "loop", value: val, pressed: val > 0 }),
            cc_buttons::METRONOME => Some(KkMk3DawEvent::Button { cc, name: "metronome", value: val, pressed: val > 0 }),
            cc_buttons::TAP_TEMPO => Some(KkMk3DawEvent::Button { cc, name: "tap_tempo", value: val, pressed: val > 0 }),
            cc_buttons::UNDO => Some(KkMk3DawEvent::Button { cc, name: "undo", value: val, pressed: val > 0 }),
            cc_buttons::REDO => Some(KkMk3DawEvent::Button { cc, name: "redo", value: val, pressed: val > 0 }),
            cc_buttons::QUANTIZE => Some(KkMk3DawEvent::Button { cc, name: "quantize", value: val, pressed: val > 0 }),
            cc_buttons::AUTO => Some(KkMk3DawEvent::Button { cc, name: "auto", value: val, pressed: val > 0 }),

            cc_nav::BANK_MAPPING => Some(KkMk3DawEvent::BankMapping(if val == 0 {
                BankMappingMode::MixerTracks
            } else {
                BankMappingMode::PluginParameters
            })),
            cc_nav::TRACK => Some(KkMk3DawEvent::Navigation { cc, name: "track", delta: decode_rel(val) }),
            cc_nav::BANK => Some(KkMk3DawEvent::Navigation { cc, name: "bank", delta: decode_rel(val) }),
            cc_nav::CLIP => Some(KkMk3DawEvent::Navigation { cc, name: "clip", delta: decode_rel(val) }),
            cc_nav::MOVE_TRANSPORT => Some(KkMk3DawEvent::Navigation { cc, name: "move_transport", delta: decode_rel(val) }),
            cc_nav::MOVE_LOOP => Some(KkMk3DawEvent::Navigation { cc, name: "move_loop", delta: decode_rel(val) }),

            cc_mixer::SELECTED_TRACK_VOLUME => Some(KkMk3DawEvent::SelectedTrackVolumeDelta(decode_rel(val))),
            cc_mixer::SELECTED_TRACK_PAN => Some(KkMk3DawEvent::SelectedTrackPanDelta(decode_rel(val))),

            _ => None,
        }
    }

    // ── Internal Helpers ─────────────────────────────────────────────────

    fn build_sysex_raw(cmd: u8, value: u8, track: u8, payload: &[u8]) -> Vec<u8> {
        let mut msg = Vec::with_capacity(SYSEX_HEADER.len() + 3 + payload.len() + 1);
        msg.extend_from_slice(&SYSEX_HEADER);
        msg.push(cmd);
        msg.push(value);
        msg.push(track);
        msg.extend_from_slice(payload);
        msg.push(SYSEX_END);
        msg
    }

    fn build_sysex_str(cmd: u8, value: u8, track: u8, text: &str) -> Vec<u8> {
        Self::build_sysex_raw(cmd, value, track, text.as_bytes())
    }
}

/// Decode a 14-bit two's complement parameter delta from raw LSB and MSB wire bytes.
pub fn decode_14bit_delta(lsb: u8, msb: u8) -> f32 {
    let diff = ((lsb as u16) & 0x7F) | (((msb as u16) & 0x7F) << 7);
    let signed_diff = if (diff & 0x2000) != 0 {
        (diff as i32) - 0x4000
    } else {
        diff as i32
    };
    (signed_diff as f32) / 8192.0
}

/// Encode a decibel float value (from -70.0 dB to +6.0 dB) to a 7-bit MIDI value (0..127).
pub fn encode_7bit_decibel(value_db: f32) -> u8 {
    let min_db = -70.0;
    let max_db = 6.0;
    let clamped = value_db.clamp(min_db, max_db);
    let normalized = (clamped - min_db) / (max_db - min_db);
    (normalized * 127.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_messages() {
        let daw = KkMk3DawController::new();
        assert_eq!(daw.build_hello(), [0xBF, 0x01, 0x04]);
        assert_eq!(daw.build_enable_14bit(), [0xBF, 0x06, 0x01]);
        assert_eq!(daw.build_goodbye(), [0xBF, 0x02, 0x00]);
    }

    #[test]
    fn test_track_name_sysex() {
        let daw = KkMk3DawController::new();
        let msg = daw.build_track_name(2, "Vocal Lead");
        assert!(msg.starts_with(&SYSEX_HEADER));
        assert_eq!(msg[10], sysex_ids::TRACK_NAME);
        assert_eq!(msg[11], 0); // value
        assert_eq!(msg[12], 2); // track index
        assert_eq!(&msg[13..13 + 10], b"Vocal Lead");
        assert_eq!(*msg.last().unwrap(), SYSEX_END);
    }

    #[test]
    fn test_track_color_rgba() {
        let daw = KkMk3DawController::new();
        let msg = daw.build_track_color_rgba(0, 1.0, 0.0, 0.5, 1.0);
        // Expect "#ff" (alpha), "ff" (red), "00" (green), "80" (blue)
        let color_slice = &msg[13..msg.len() - 1];
        let color_str = std::str::from_utf8(color_slice).unwrap();
        assert_eq!(color_str, "#ffff0080");
    }

    #[test]
    fn test_vu_meter_encoding() {
        let daw = KkMk3DawController::new();
        let left = [-70.0, 6.0, -32.0, -70.0, -70.0, -70.0, -70.0, -70.0];
        let right = [-70.0, 6.0, 0.0, -70.0, -70.0, -70.0, -70.0, -70.0];
        let msg = daw.build_vu_meters(&left, &right);
        assert_eq!(msg[10], sysex_ids::VU_METER);
        assert_eq!(msg[11], 2); // value
        assert_eq!(msg[12], 0); // track
        assert_eq!(msg[13], 0); // -70 dB -> 0
        assert_eq!(msg[14], 0); // -70 dB -> 0
        assert_eq!(msg[15], 127); // +6 dB -> 127
        assert_eq!(msg[16], 127); // +6 dB -> 127
    }

    #[test]
    fn test_14bit_delta_decoding() {
        // Positive small increment: diff = 1 (lsb=1, msb=0)
        let delta = decode_14bit_delta(1, 0);
        assert!((delta - (1.0 / 8192.0)).abs() < 1e-6);

        // Negative small decrement: diff = -1 in 14-bit is 0x3FFF (lsb=0x7F, msb=0x7F)
        let delta_neg = decode_14bit_delta(0x7F, 0x7F);
        assert!((delta_neg - (-1.0 / 8192.0)).abs() < 1e-6);
    }

    #[test]
    fn test_parse_incoming_knob_14bit() {
        let daw = KkMk3DawController::new();
        // Construct SysEx 0x7F message
        let mut msg = Vec::new();
        msg.extend_from_slice(&SYSEX_HEADER);
        msg.push(sysex_ids::ADJUST_PARAMETER_VALUE);
        msg.push(0x02); // TrackParameter group
        msg.push(0x04); // Knob 4
        msg.push(0x01); // LSB
        msg.push(0x00); // MSB
        msg.push(SYSEX_END);

        let parsed = daw.parse_incoming(&msg);
        match parsed {
            Some(KkMk3DawEvent::KnobAdjustment14Bit { group, index, delta }) => {
                assert_eq!(group, KnobGroup::TrackParameter);
                assert_eq!(index, 4);
                assert!(delta > 0.0);
            }
            other => panic!("Unexpected event parsed: {:?}", other),
        }
    }

    #[test]
    fn test_parse_incoming_cc() {
        let daw = KkMk3DawController::new();
        // Play button pressed
        let event = daw.parse_incoming(&[STATUS_CC, cc_buttons::PLAY, 1]);
        assert_eq!(
            event,
            Some(KkMk3DawEvent::Button {
                cc: cc_buttons::PLAY,
                name: "play",
                value: 1,
                pressed: true
            })
        );

        // Track encoder turn next (+1)
        let event_nav = daw.parse_incoming(&[STATUS_CC, cc_nav::TRACK, 1]);
        assert_eq!(
            event_nav,
            Some(KkMk3DawEvent::Navigation {
                cc: cc_nav::TRACK,
                name: "track",
                delta: 1
            })
        );
    }
}
