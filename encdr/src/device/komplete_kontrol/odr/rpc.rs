//! MessagePack-RPC packet framing and encoders for Komplete Kontrol Mk3 On-Device Rendering (ODR).
//!
//! Encodes and decodes the protocol spoken by Native Instruments
//! Komplete Kontrol Mk3 keyboards (S49, S61, S88) over USB Bulk Interface 3.
//!
//! # Protocol Specification
//! - **Packet Framing**: `<u32 little-endian length>` followed by `<MessagePack body>`.
//! - **MessagePack Format**: `[2, method, params]` (MessagePack-RPC notification).
//! - **Session Handshake**: Sent to bulk OUT endpoint `0x04` at connection startup:
//!   `[2, "handshake", [{host_protocol_version, symbol_registry, ...}]]`.
//! - **Symbol Registry**: Following handshake, all method IDs and map keys on the wire
//!   are sent as integer indices into the 437-entry symbol registry ([`super::symbols`]).
//! - **Instance Addressing**: Client methods target `instance_00` (index 14).
//! - **Heartbeat**: Sent to bulk OUT endpoint `0x03` every 8 seconds:
//!   `[2, sym("heartbeat"), []]`.
//! - **Incoming Events**: Read from bulk IN endpoint `0x83` (knob deltas, touches, responses).

use std::time::{SystemTime, UNIX_EPOCH};
use rmpv::Value;
use crate::core::error::{EncdrError, Result};
use super::models::{
    BrowserModel, DeviceSettings, FileAsset, KkMk3Page, MixerModel, PluginChainModel, PluginData,
    SmartPlayData,
};
use super::symbols::{sym_name, try_sym, SYMBOLS};

/// RPC message type constants in the MessagePack-RPC standard specification.
pub mod msg_type {
    pub const REQUEST: u32 = 0;
    pub const RESPONSE: u32 = 1;
    pub const NOTIFICATION: u32 = 2;
}

/// Fallback string method names for non-symbol or diagnostic RPCs.
pub mod rpc_methods {
    pub const SET_PLUGIN_DATA: &str = "client_instance_parameter_page_model_set_plugin_data";
    pub const UPDATE_PARAM_VALUE: &str = "client_instance_parameter_page_model_update_parameter_value";
    pub const SET_HOST_VIEWSTATE: &str = "client_instance_parameter_page_model_set_host_owned_viewstate";
    pub const SET_DEVICE_VIEWSTATE: &str = "client_instance_parameter_page_model_set_device_owned_viewstate";
    pub const SET_LIGHTGUIDE_LEDS: &str = "client_instance_lightguide_set";
    pub const SET_MIXER_METERS: &str = "client_instance_mixer_set_meters";
    pub const SET_MIXER_TRACK_DATA: &str = "client_instance_mixer_set_track_data";
    pub const SET_PLUGIN_CHAIN: &str = "client_instance_plugin_chain_model_set";
    pub const SET_PLUGIN_CHAIN_INDEX: &str = "client_instance_plugin_chain_model_set_current_index";
    pub const SET_SMARTPLAY_DATA: &str = "client_instance_smartplay_set_data";
    pub const SET_BROWSER_DATA: &str = "client_instance_browser_set_data_model";
    pub const SET_DEVICE_SETTINGS: &str = "device_setting";
    pub const SET_PAGE: &str = "client_page_set";
    pub const REGISTER_ASSET: &str = "assets_add";
}

/// Incoming ODR events decoded from USB bulk IN endpoint `0x83`.
#[derive(Debug, Clone, PartialEq)]
pub enum KkMk3OdrEvent {
    /// Relative rotary delta for an encoder (index 0..7, delta e.g. `+0.012`, `-0.008`).
    ParameterDelta { param_index: u32, delta: f32 },
    /// Absolute value update for an encoder (index 0..7, value 0.0..=1.0).
    ParameterSet { param_index: u32, value: f32 },
    /// Rotary knob capacitive touch state changed.
    ParameterTouched { param_index: u32, touched: bool },
    /// Device acknowledged handshake.
    HandshakeReply,
    /// Device requested a fresh handshake (e.g. after a reboot or session timeout).
    HeartbeatRequestHandshake,
    /// Unhandled or raw notification from device.
    Other { method: String, value: Value },
}

/// A symbol-encoded name: its registry index, or the plain string for names the
/// registry doesn't contain (the wire format accepts both; the handshake itself
/// uses string keys). Never panics, so an unregistered name can't take down a
/// caller.
#[inline]
pub fn sym_value(name: &str) -> Value {
    match try_sym(name) {
        Some(id) => Value::from(id),
        None => Value::from(name),
    }
}

/// Map with symbol keys (keys are encoded as integer symbol IDs, falling back to
/// string keys for names not in the registry).
#[inline]
pub fn smap(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(pairs.into_iter().map(|(k, v)| (sym_value(k), v)).collect())
}

/// Map with plain string keys (used where NI sends string keys, e.g. during handshake).
#[inline]
pub fn strmap(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(pairs.into_iter().map(|(k, v)| (Value::from(k), v)).collect())
}

/// Helper to encode an RGB color structure as an ODR symbol map.
#[inline]
pub fn rgb_val(r: u8, g: u8, b: u8) -> Value {
    smap(vec![
        ("r", (r as i64).into()),
        ("g", (g as i64).into()),
        ("b", (b as i64).into()),
        ("is_rgb565", false.into()),
    ])
}

/// Encode a MessagePack-RPC notification with a 4-byte little-endian length prefix.
pub fn frame_notification(method: Value, params: Vec<Value>) -> Vec<u8> {
    let msg = Value::Array(vec![2.into(), method, Value::Array(params)]);
    let mut body = Vec::new();
    rmpv::encode::write_value(&mut body, &msg).expect("MessagePack serialization failed");
    let mut out = (body.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(&body);
    out
}

/// Format current system time as "YYYY-MM-DDTHH:MM:SS+0000" in UTC.
pub fn utc_clock_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}+0000", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// MessagePack-RPC framer and encoder for Komplete Kontrol Mk3 ODR.
#[derive(Debug, Default, Clone)]
pub struct OdrRpcFramer {
    next_msg_id: u32,
}

impl OdrRpcFramer {
    pub fn new() -> Self {
        Self { next_msg_id: 1 }
    }

    /// Internal helper: target `instance_00` with a symbol-based method.
    fn build_instance_cmd(method: &str, params: Vec<Value>) -> Vec<u8> {
        let mut full = vec![sym_value("instance_00")];
        full.extend(params);
        frame_notification(sym_value(method), full)
    }

    // ── Session & Heartbeat ──────────────────────────────────────────────

    /// Build session handshake packet sent to Endpoint `0x04` on startup.
    ///
    /// Registers the 437-symbol registry, protocol version, and host clock.
    pub fn build_handshake() -> Vec<u8> {
        let registry = Value::Array(SYMBOLS.iter().map(|s| Value::from(*s)).collect());
        let params = strmap(vec![
            ("host_protocol_version", "2.5.0".into()),
            ("symbol_registry", registry),
            ("agent_version", "2.1.5 (R14)".into()),
            ("utc_clock", utc_clock_now().into()),
            ("disable_heartbeat_validation", false.into()),
        ]);
        frame_notification(Value::from("handshake"), vec![params])
    }

    /// Build heartbeat packet sent to Endpoint `0x03` every 8 seconds.
    pub fn build_heartbeat() -> Vec<u8> {
        frame_notification(sym_value("heartbeat"), vec![])
    }

    /// Build client instance registration notification (`client_instance_add`).
    pub fn build_instance_add(name: &str, is_host: bool) -> Vec<u8> {
        let params = strmap(vec![
            ("name", name.into()),
            ("version", "3.5.4".into()),
            ("type", "standalone".into()),
            ("is_host", is_host.into()),
        ]);
        Self::build_instance_cmd("client_instance_add", vec![params])
    }

    /// Build active page switch notification (`client_page_set`).
    pub fn build_set_page(&self, page: KkMk3Page) -> Result<Vec<u8>> {
        let page_sym = match page {
            KkMk3Page::Parameters => "parameter",
            KkMk3Page::Browser => "browser",
            KkMk3Page::Mixer => "mixer",
            KkMk3Page::SmartPlay => "smartplay",
            KkMk3Page::PluginChain => "parameter",
            KkMk3Page::Settings => "settings",
        };
        Ok(Self::build_instance_cmd("client_page_set", vec![sym_value(page_sym)]))
    }

    /// Build client focus acquisition notification (`client_focus_acquire`).
    pub fn build_focus_acquire() -> Vec<u8> {
        Self::build_instance_cmd("client_focus_acquire", vec![])
    }

    /// Build client focus release notification (`client_focus_release`).
    pub fn build_focus_release() -> Vec<u8> {
        Self::build_instance_cmd("client_focus_release", vec![])
    }

    /// Build initial host-owned client state notification.
    pub fn build_set_host_instance_state(preset_name: &str, tempo: f32) -> Vec<u8> {
        let state = smap(vec![
            ("transport_section_state", Value::Nil),
            ("undo_state", Value::Nil),
            ("current_preset_name", preset_name.into()),
            ("current_track_name", Value::Nil),
            ("current_track_number", Value::Nil),
            ("status_message", Value::Nil),
            ("tempo", Value::F32(tempo)),
            ("dawcontrol_plugin_identifier", "NIKB00".into()),
            ("master_volume", Value::Nil),
            ("active_keyswitch", Value::Nil),
            ("smartplay_enabled", true.into()),
            ("browser_enabled", true.into()),
            ("quantize_state", Value::Nil),
            ("host_pad_mode", Value::Nil),
            ("selected_sound_id", Value::Nil),
            ("plugin_identifier", Value::Nil),
            ("tree_level_str", Value::Nil),
        ]);
        Self::build_instance_cmd("client_instance_set_host_owned_instance_client_state", vec![state])
    }

    // ── Parameter Pages & Controls ───────────────────────────────────────

    /// Build parameter page model packet (`client_instance_parameter_page_model_set_plugin_data`).
    ///
    /// Encodes parameters 1..8 and top header appearance.
    pub fn build_set_plugin_data(&self, plugin_data: &PluginData) -> Result<Vec<u8>> {
        let params: Vec<(Value, Value)> = plugin_data
            .parameters
            .iter()
            .enumerate()
            .take(8)
            .map(|(i, param)| {
                let p = strmap(vec![(
                    "continuous_parameter",
                    strmap(vec![
                        (
                            "parameter_value",
                            strmap(vec![
                                ("value", Value::F32(param.value)),
                                ("display_value", param.display_value.clone().into()),
                            ]),
                        ),
                        ("parameter_style", strmap(vec![("knob_parameter_style", Value::Nil)])),
                    ]),
                )]);
                (Value::from(i as i64 + 1), p)
            })
            .collect();

        let bg_val = match &plugin_data.background {
            Some(bg) => bg.parse::<i64>().map(Value::from).unwrap_or_else(|_| bg.clone().into()),
            None => Value::Nil,
        };

        let payload = smap(vec![
            ("name", plugin_data.name.clone().into()),
            ("background", bg_val),
            ("parameters", Value::Map(params)),
            ("plugin_color", rgb_val(plugin_data.plugin_color.r, plugin_data.plugin_color.g, plugin_data.plugin_color.b)),
            ("nks_control_color", rgb_val(235, 233, 234)),
        ]);

        Ok(Self::build_instance_cmd("client_instance_parameter_page_model_set_plugin_data", vec![payload]))
    }

    /// Build layout sections for parameter labels (`client_instance_parameter_page_model_set_host_owned_viewstate`).
    pub fn build_set_host_viewstate(&self, plugin_data: &PluginData) -> Vec<u8> {
        let mut sections: Vec<Value> = Vec::new();
        let total = plugin_data.parameters.len().min(8);

        if total > 0 {
            // Group parameters by section name if specified, otherwise split into left/right 4-knob banks
            let mut sec1_params = Vec::new();
            let mut sec2_params = Vec::new();
            let sec1_title = plugin_data.parameters.first().map(|p| p.section_name.as_str()).unwrap_or("Controls");
            let sec2_title = if total > 4 {
                plugin_data.parameters.get(4).map(|p| p.section_name.as_str()).unwrap_or("Controls")
            } else {
                "Controls"
            };

            for (i, p) in plugin_data.parameters.iter().enumerate().take(total) {
                let item = smap(vec![("id", (i as i64 + 1).into()), ("name", p.name.clone().into())]);
                if i < 4 {
                    sec1_params.push(item);
                } else {
                    sec2_params.push(item);
                }
            }

            let s1 = smap(vec![
                ("section_name", (if sec1_title.is_empty() { "Controls" } else { sec1_title }).into()),
                ("parameters", Value::Array(sec1_params)),
            ]);
            sections.push(s1);

            if !sec2_params.is_empty() {
                let s2 = smap(vec![
                    ("section_name", (if sec2_title.is_empty() { "Controls" } else { sec2_title }).into()),
                    ("parameters", Value::Array(sec2_params)),
                ]);
                sections.push(s2);
            }
        }

        let layout = smap(vec![
            ("nks1_layout", Value::Array(vec![Value::Array(sections)])),
            ("nks2_layout", Value::Nil),
        ]);

        Self::build_instance_cmd("client_instance_parameter_page_model_set_host_owned_viewstate", vec![layout])
    }

    /// Build parameter value update packet (`client_instance_parameter_page_model_update_parameter_value`).
    pub fn build_update_parameter_value(&self, param_index: u32, value: f32) -> Result<Vec<u8>> {
        let display = format!("{:.0}%", value * 100.0);
        self.build_update_parameter_value_with_display(param_index, value, &display)
    }

    /// Build parameter value update packet with custom readout string.
    pub fn build_update_parameter_value_with_display(
        &self,
        param_index: u32,
        value: f32,
        display: &str,
    ) -> Result<Vec<u8>> {
        let slot_id = (param_index as i64 + 1).into();
        let payload = smap(vec![(
            "continuous_parameter",
            smap(vec![("value", Value::F32(value)), ("display_value", display.into())]),
        )]);
        Ok(Self::build_instance_cmd(
            "client_instance_parameter_page_model_update_parameter_value",
            vec![slot_id, payload],
        ))
    }

    // ── Keybed Light Guide ───────────────────────────────────────────────

    /// Build Light Guide update packet using direct `(midi_note, color_index)` addressing.
    ///
    /// `color_index` is 0..63 (idle/off is index 44).
    pub fn build_set_lightguide_indexed(&self, leds: &[(u8, u8)]) -> Result<Vec<u8>> {
        let array: Vec<Value> = leds
            .iter()
            .map(|&(note, color)| Value::Array(vec![(note as i64).into(), (color as i64).into()]))
            .collect();
        let payload = smap(vec![("client_midi_addressing", Value::Array(array))]);
        Ok(Self::build_instance_cmd("client_instance_lightguide_set", vec![payload]))
    }

    /// Build Light Guide update packet from RGB key values.
    ///
    /// Automatically converts RGB values to the nearest Mk3 Light Guide color palette indices.
    pub fn build_set_lightguide(&self, rgb_keys: &[(u8, u8, u8)]) -> Result<Vec<u8>> {
        let first_note = match rgb_keys.len() {
            88 => 21u8,
            61 => 36u8,
            _ => 36u8,
        };
        let mut leds = Vec::with_capacity(128);
        for note in 0..128u8 {
            let color = if note >= first_note && (note - first_note) < rgb_keys.len() as u8 {
                let (r, g, b) = rgb_keys[(note - first_note) as usize];
                rgb_to_lightguide_color(r, g, b)
            } else {
                44 // LIGHTGUIDE_IDLE
            };
            leds.push((note, color));
        }
        self.build_set_lightguide_indexed(&leds)
    }

    // ── Extended Models ──────────────────────────────────────────────────

    /// Build serial plugin chain notification (`client_instance_plugin_chain_model_set`).
    pub fn build_set_plugin_chain(&self, chain: &PluginChainModel) -> Result<Vec<u8>> {
        let plugins: Vec<Value> = chain
            .plugins
            .iter()
            .map(|p| {
                smap(vec![
                    ("name", p.name.clone().into()),
                    ("identifier", Value::Binary(b"Encdr\0\0\0\0\0\0\0\0\0\0\x01".to_vec())),
                    ("can_bypass", false.into()),
                    ("bypass_enabled", p.is_bypassed.into()),
                    ("can_remove", false.into()),
                    ("can_move_forward", false.into()),
                    ("can_move_backward", false.into()),
                    ("type", Value::Nil),
                    ("supported_nks_version", Value::Nil),
                ])
            })
            .collect();

        let payload = smap(vec![
            ("plugins", Value::Array(plugins)),
            ("current_plugin_index", (chain.current_index as i64).into()),
        ]);
        Ok(Self::build_instance_cmd("client_instance_plugin_chain_model_set", vec![payload]))
    }

    /// Build focused plugin chain index notification.
    pub fn build_set_plugin_chain_index(&self, index: u32) -> Result<Vec<u8>> {
        Ok(Self::build_instance_cmd(
            "client_instance_plugin_chain_model_set_current_index",
            vec![(index as i64).into()],
        ))
    }

    /// Build mixer track data notification (`client_instance_mixer_set_track_data`).
    pub fn build_set_mixer_track_data(&self, mixer: &MixerModel) -> Result<Vec<u8>> {
        let tracks: Vec<Value> = mixer
            .tracks
            .iter()
            .map(|t| {
                smap(vec![
                    ("short_name", t.name.clone().into()),
                    ("color", rgb_val(t.color.r, t.color.g, t.color.b)),
                    ("volume", Value::F32(t.volume)),
                    ("volume_label", t.volume_display.clone().into()),
                    ("pan", Value::F32(t.pan)),
                    ("pan_label", t.pan_display.clone().into()),
                    ("mute", t.muted.into()),
                    ("solo", t.soloed.into()),
                    ("arm", t.armed.into()),
                ])
            })
            .collect();
        let payload = smap(vec![("track_infos", Value::Array(tracks))]);
        Ok(Self::build_instance_cmd("client_instance_mixer_set_track_data", vec![payload]))
    }

    /// Build stereo VU meter notification (`client_instance_mixer_set_meters`).
    pub fn build_set_mixer_meters(&self, left_meters: &[f32], right_meters: &[f32]) -> Result<Vec<u8>> {
        let pairs: Vec<Value> = left_meters
            .iter()
            .copied()
            .zip(right_meters.iter().copied())
            .map(|(l, r)| Value::Array(vec![Value::F32(l), Value::F32(r)]))
            .collect();
        let payload = smap(vec![("meter", Value::Array(pairs))]);
        Ok(Self::build_instance_cmd("client_instance_mixer_set_meters", vec![payload]))
    }

    /// Build Smart Play engine notification (`client_instance_smartplay_set_data`).
    pub fn build_set_smartplay_data(&self, smartplay: &SmartPlayData) -> Result<Vec<u8>> {
        let payload = smap(vec![
            ("smartplay_enabled", true.into()),
            (
                "scale",
                smap(vec![
                    ("enabled", smartplay.scale.enabled.into()),
                    ("root_key", (smartplay.scale.root_key as i64).into()),
                    ("type", smartplay.scale.scale_type.clone().into()),
                ]),
            ),
            (
                "arpeggiator",
                smap(vec![
                    ("enabled", smartplay.arp.enabled.into()),
                    ("sequence", smartplay.arp.pattern.clone().into()),
                    ("rate", smartplay.arp.rate.clone().into()),
                ]),
            ),
        ]);
        Ok(Self::build_instance_cmd("client_instance_smartplay_set_data", vec![payload]))
    }

    /// Build preset browser data notification (`client_instance_browser_set_data_model`).
    pub fn build_set_browser_model(&self, browser: &BrowserModel) -> Result<Vec<u8>> {
        let sounds: Vec<Value> = browser
            .sounds
            .iter()
            .map(|s| {
                smap(vec![
                    ("name", s.name.clone().into()),
                    ("accessible_name", s.name.clone().into()),
                ])
            })
            .collect();
        let payload = smap(vec![("items", Value::Array(sounds))]);
        Ok(Self::build_instance_cmd("client_instance_browser_set_data_model", vec![payload]))
    }

    /// Build device settings notification (`device_setting`).
    pub fn build_set_device_settings(&self, settings: &DeviceSettings) -> Result<Vec<u8>> {
        let payload = smap(vec![
            ("display_brightness", (settings.display_brightness as i64).into()),
            ("led_brightness", (settings.led_brightness as i64).into()),
        ]);
        Ok(Self::build_instance_cmd("device_setting", vec![payload]))
    }

    /// Build image asset registration packet.
    pub fn build_register_asset(&mut self, asset: &FileAsset) -> Result<(u32, Vec<u8>)> {
        let msg_id = self.next_msg_id;
        self.next_msg_id = self.next_msg_id.wrapping_add(1);
        let payload = smap(vec![
            ("id", asset.identifier.clone().into()),
            ("image", Value::Binary(asset.data.clone())),
        ]);
        let bytes = Self::build_instance_cmd("assets_add", vec![payload]);
        Ok((msg_id, bytes))
    }

    // ── Incoming Packet Parsing ──────────────────────────────────────────

    /// Parse the first framed message in `packet` (`<u32 LE length><MessagePack>`).
    ///
    /// Returns `Ok(None)` if `packet` doesn't hold a complete frame. A USB transfer
    /// can carry part of a message or several messages, so device-side readers
    /// should use [`OdrStreamDecoder`] instead.
    pub fn parse_incoming(packet: &[u8]) -> Result<Option<KkMk3OdrEvent>> {
        if packet.len() < 4 {
            return Ok(None);
        }
        let frame_len = u32::from_le_bytes([packet[0], packet[1], packet[2], packet[3]]) as usize;
        if packet.len() < 4 + frame_len {
            return Ok(None);
        }
        Ok(Self::decode_message(&packet[4..4 + frame_len])?.into_iter().next())
    }

    /// Decode one unframed MessagePack message body into events.
    ///
    /// A `plugin_action` map can carry several actions, so this returns every one
    /// of them; any other message becomes a single event.
    pub fn decode_message(body: &[u8]) -> Result<Vec<KkMk3OdrEvent>> {
        let val = rmpv::decode::read_value(&mut &body[..])
            .map_err(|e| EncdrError::Protocol(format!("MsgPack decode failed: {e}")))?;

        let arr = match val.as_array() {
            Some(a) if a.len() >= 2 => a,
            _ => return Ok(Vec::new()),
        };

        let method_name = arr[1]
            .as_i64()
            .and_then(sym_name)
            .map(|s| s.to_string())
            .or_else(|| arr[1].as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        match method_name.as_str() {
            "handshake_reply" => return Ok(vec![KkMk3OdrEvent::HandshakeReply]),
            "agent_heartbeat_request_handshake" => return Ok(vec![KkMk3OdrEvent::HeartbeatRequestHandshake]),
            _ => {}
        }

        if method_name == "plugin_action" {
            // [2, plugin_action, [instance, {action: [param_id, value], ...}]]
            let entries = arr
                .get(2)
                .and_then(|p| p.as_array())
                .and_then(|p| p.get(1))
                .and_then(|m| m.as_map());
            if let Some(entries) = entries {
                let mut events = Vec::new();
                for (k, v) in entries {
                    let action = k
                        .as_i64()
                        .and_then(sym_name)
                        .map(|s| s.to_string())
                        .or_else(|| k.as_str().map(|s| s.to_string()))
                        .unwrap_or_default();
                    // Parameter ids on the wire are 1-based slots (1..=8).
                    let Some([idx, arg]) = v.as_array().map(|a| a.as_slice()) else { continue };
                    let Some(param_index) = idx.as_i64().filter(|i| (1..=8).contains(i)).map(|i| (i - 1) as u32)
                    else {
                        continue;
                    };
                    match action.as_str() {
                        "plugin_parameter_value_change" => {
                            if let Some(d) = arg.as_f64() {
                                events.push(KkMk3OdrEvent::ParameterDelta { param_index, delta: d as f32 });
                            }
                        }
                        "plugin_parameter_value_set" => {
                            if let Some(d) = arg.as_f64() {
                                events.push(KkMk3OdrEvent::ParameterSet { param_index, value: d as f32 });
                            }
                        }
                        "plugin_parameter_erp_touched" => {
                            if let Some(t) = arg.as_bool() {
                                events.push(KkMk3OdrEvent::ParameterTouched { param_index, touched: t });
                            }
                        }
                        _ => {}
                    }
                }
                if !events.is_empty() {
                    return Ok(events);
                }
            }
        }

        Ok(vec![KkMk3OdrEvent::Other { method: method_name, value: val }])
    }
}

/// Reassembles `<u32 LE length><MessagePack>` frames from the bulk IN stream.
///
/// Bulk transfers don't respect message boundaries: a long message (e.g. the
/// settings sent after `handshake_reply`) spans several transfers, and short
/// ones can share a transfer. Feed every completed transfer to [`push`](Self::push).
#[derive(Debug, Default)]
pub struct OdrStreamDecoder {
    buf: Vec<u8>,
}

impl OdrStreamDecoder {
    /// Frames larger than this are treated as stream corruption.
    pub const MAX_FRAME: usize = 16 * 1024 * 1024;

    pub fn new() -> Self {
        Self::default()
    }

    /// Append received bytes and return the events of every message completed by them.
    pub fn push(&mut self, data: &[u8]) -> Vec<KkMk3OdrEvent> {
        self.buf.extend_from_slice(data);
        let mut events = Vec::new();
        let mut pos = 0;
        while self.buf.len() - pos >= 4 {
            let b = &self.buf[pos..];
            let n = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
            if n > Self::MAX_FRAME {
                tracing::warn!("ODR stream: implausible frame length {n}; discarding {} buffered bytes", self.buf.len() - pos);
                pos = self.buf.len();
                break;
            }
            if b.len() < 4 + n {
                break;
            }
            match OdrRpcFramer::decode_message(&b[4..4 + n]) {
                Ok(evs) => events.extend(evs),
                Err(e) => tracing::warn!("ODR stream: undecodable message ({n} bytes): {e}"),
            }
            pos += 4 + n;
        }
        self.buf.drain(..pos);
        events
    }

    /// Drop any partially received message (e.g. after a re-handshake).
    pub fn clear(&mut self) {
        self.buf.clear();
    }
}

/// Convert an RGB color triplet to the nearest Komplete Kontrol Mk3 Light Guide color index (0..63).
pub fn rgb_to_lightguide_color(r: u8, g: u8, b: u8) -> u8 {
    if r == 0 && g == 0 && b == 0 {
        return 44; // LIGHTGUIDE_IDLE (off/dim)
    }

    // Hue-based 64-color mapping across the spectrum
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;

    let max = rf.max(gf.max(bf));
    let min = rf.min(gf.min(bf));
    let delta = max - min;

    if delta < 0.1 {
        return 63; // White/monochrome highlight
    }

    let mut hue = if max == rf {
        (gf - bf) / delta
    } else if max == gf {
        2.0 + (bf - rf) / delta
    } else {
        4.0 + (rf - gf) / delta
    } * 60.0;

    if hue < 0.0 {
        hue += 360.0;
    }

    // Map 0..360 hue to 0..60 palette index
    ((hue / 360.0 * 60.0) as u8).min(63)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::symbols::sym;

    fn framed(v: &Value) -> Vec<u8> {
        let mut body = Vec::new();
        rmpv::encode::write_value(&mut body, v).unwrap();
        let mut out = (body.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(&body);
        out
    }

    fn plugin_action(actions: Vec<(&str, Value)>) -> Value {
        Value::Array(vec![
            2.into(),
            sym("plugin_action").into(),
            Value::Array(vec![
                sym("instance_00").into(),
                Value::Map(actions.into_iter().map(|(k, v)| (Value::from(sym(k)), v)).collect()),
            ]),
        ])
    }

    /// Every method verified on hardware must go out as a registry index, never
    /// via the string fallback.
    #[test]
    fn verified_methods_are_registered() {
        for m in [
            "heartbeat",
            "instance_00",
            "client_instance_add",
            "client_page_set",
            "client_focus_acquire",
            "client_instance_set_host_owned_instance_client_state",
            "client_instance_parameter_page_model_set_plugin_data",
            "client_instance_parameter_page_model_set_host_owned_viewstate",
            "client_instance_parameter_page_model_update_parameter_value",
            "client_instance_plugin_chain_model_set",
            "client_instance_lightguide_set",
            "client_midi_addressing",
            "parameter",
            "handshake_reply",
            "agent_heartbeat_request_handshake",
            "plugin_action",
            "plugin_parameter_value_change",
        ] {
            assert!(try_sym(m).is_some(), "'{m}' missing from symbol registry");
        }
    }

    #[test]
    fn unregistered_keys_fall_back_to_strings() {
        assert_eq!(sym_value("display_brightness"), Value::from("display_brightness"));
        // Must not panic.
        let bytes = OdrRpcFramer::new().build_set_device_settings(&DeviceSettings::default()).unwrap();
        assert!(bytes.len() > 4);
    }

    #[test]
    fn stream_decoder_reassembles_split_and_joined_frames() {
        let a = framed(&plugin_action(vec![(
            "plugin_parameter_value_change",
            Value::Array(vec![3.into(), Value::F64(-0.5)]),
        )]));
        let b = framed(&Value::Array(vec![2.into(), sym("handshake_reply").into(), Value::Array(vec![])]));
        let mut stream = a.clone();
        stream.extend_from_slice(&b);

        let mut dec = OdrStreamDecoder::new();
        // Split mid-frame: nothing complete yet.
        assert!(dec.push(&stream[..5]).is_empty());
        let evs = dec.push(&stream[5..]);
        assert_eq!(
            evs,
            vec![
                KkMk3OdrEvent::ParameterDelta { param_index: 2, delta: -0.5 },
                KkMk3OdrEvent::HandshakeReply,
            ]
        );
        assert!(dec.push(&[]).is_empty());
    }

    #[test]
    fn plugin_action_with_several_actions_yields_all() {
        let msg = plugin_action(vec![
            ("plugin_parameter_value_change", Value::Array(vec![1.into(), Value::F64(0.25)])),
            ("plugin_parameter_value_set", Value::Array(vec![8.into(), Value::F64(1.0)])),
        ]);
        let evs = OdrRpcFramer::decode_message(&framed(&msg)[4..]).unwrap();
        assert_eq!(
            evs,
            vec![
                KkMk3OdrEvent::ParameterDelta { param_index: 0, delta: 0.25 },
                KkMk3OdrEvent::ParameterSet { param_index: 7, value: 1.0 },
            ]
        );
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn test_heartbeat_wire_format() {
        // Known-good frame verified on hardware.
        let hb = OdrRpcFramer::build_heartbeat();
        assert_eq!(hex(&hb), "0400000093026190");
    }

    #[test]
    fn test_update_parameter_value_wire_format() {
        // update_parameter_value(1, 0.4974874258041382, "  0")
        let framer = OdrRpcFramer::new();
        let upd = framer
            .build_update_parameter_value_with_display(0, 0.4974874258041382, "  0")
            .unwrap();
        assert_eq!(hex(&upd), "17000000930254930e0181ccfd82ccbaca3efeb6acccf2a3202030");
    }

    #[test]
    fn test_client_instance_add_wire_format() {
        let add = OdrRpcFramer::build_instance_add("Komplete Kontrol", true);
        let h = hex(&add);
        assert!(h.starts_with("4300000093024292") && h.contains("0e84a46e616d65b04b6f6d706c657465"));
    }

    #[test]
    fn test_handshake_wire_format() {
        let hs = OdrRpcFramer::build_handshake();
        // Starts with length prefix and notification header
        assert!(hs.len() > 1000);
        let len = u32::from_le_bytes([hs[0], hs[1], hs[2], hs[3]]) as usize;
        assert_eq!(len, hs.len() - 4);
    }

    #[test]
    fn test_parse_plugin_action_delta() {
        // Test parsing incoming rotary knob delta
        let mut msg_body = Vec::new();
        let notification = Value::Array(vec![
            2.into(),
            sym("plugin_action").into(),
            Value::Array(vec![
                sym("instance_00").into(),
                Value::Map(vec![(
                    Value::from(sym("plugin_parameter_value_change")),
                    Value::Array(vec![2.into(), Value::F64(0.025)]),
                )]),
            ]),
        ]);
        rmpv::encode::write_value(&mut msg_body, &notification).unwrap();
        let mut packet = (msg_body.len() as u32).to_le_bytes().to_vec();
        packet.extend_from_slice(&msg_body);

        let event = OdrRpcFramer::parse_incoming(&packet).unwrap().unwrap();
        assert_eq!(
            event,
            KkMk3OdrEvent::ParameterDelta {
                param_index: 1,
                delta: 0.025,
            }
        );
    }
}
