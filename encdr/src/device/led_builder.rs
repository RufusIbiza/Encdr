use std::collections::HashMap;

use crate::core::descriptor::*;
use crate::core::led::LedValue;

/// Builds USB LED output buffers from named LED values, driven by the descriptor.
pub struct LedBuilder {
    group_id: String,
    buffer_size: usize,
    prefix: Vec<u8>,
    endpoint_address: u8,
    transfer_type: TransferType,
    /// Byte written for `LedValue::Backlight` on single-color LEDs in this group
    backlight_byte: u8,
    /// Byte written for `LedValue::Dim` on single-color LEDs in this group
    dim_byte: u8,
    /// Byte written for `LedValue::Bright` on single-color LEDs in this group
    bright_byte: u8,
    /// Map from LED name → how to write it into the buffer
    led_map: HashMap<String, LedMapping>,
    /// The current LED buffer (dirty-tracked)
    buffer: Vec<u8>,
    dirty: bool,
}

/// Bytes written for the named levels on one single-byte LED.
#[derive(Clone, Copy)]
struct Levels {
    backlight: u8,
    dim: u8,
    bright: u8,
}

impl Levels {
    /// White (index 17 in the 1-based palette) at intensities 0, 1 and 3.
    const INDEXED_WHITE: Levels = Levels { backlight: 17 << 2, dim: (17 << 2) | 1, bright: (17 << 2) | 3 };

    /// These levels with the LED's own descriptor overrides applied.
    fn with_overrides(self, led: &SingleLedDesc) -> Levels {
        Levels {
            backlight: led.backlight_value.unwrap_or(self.backlight),
            dim: led.dim_value.unwrap_or(self.dim),
            bright: led.bright_value.unwrap_or(self.bright),
        }
    }
}

#[derive(Clone, Copy)]
enum LedMapping {
    Single { offset: usize, levels: Levels },
    Rgb { r: usize, g: usize, b: usize },
    Strip { offset: usize, count: usize },
    Indexed { offset: usize, levels: Levels },
}

impl LedBuilder {
    pub fn new(desc: &LedLayoutDesc, interface: &InterfaceDesc) -> Self {
        let mut led_map = HashMap::new();
        let mut buffer = vec![0u8; desc.buffer_size];
        let group_levels = Levels {
            backlight: desc.backlight_byte(),
            dim: desc.dim_byte(),
            bright: desc.bright_byte(),
        };

        for item in &desc.items {
            match item {
                LedItemDesc::Single(s) => {
                    if let Some(value) = s.default
                        && let Some(slot) = buffer.get_mut(s.offset) {
                            *slot = value;
                        }
                    led_map.insert(
                        s.name.clone(),
                        LedMapping::Single { offset: s.offset, levels: group_levels.with_overrides(s) },
                    );
                }
                LedItemDesc::Indexed(i) => {
                    if let Some(value) = i.default
                        && let Some(slot) = buffer.get_mut(i.offset) {
                            *slot = value;
                        }
                    led_map.insert(
                        i.name.clone(),
                        LedMapping::Indexed { offset: i.offset, levels: Levels::INDEXED_WHITE.with_overrides(i) },
                    );
                }
                LedItemDesc::Rgb(r) => {
                    led_map.insert(
                        r.name.clone(),
                        LedMapping::Rgb {
                            r: r.offsets.r,
                            g: r.offsets.g,
                            b: r.offsets.b,
                        },
                    );
                }
                LedItemDesc::Strip(s) => {
                    led_map.insert(
                        s.name.clone(),
                        LedMapping::Strip {
                            offset: s.offset,
                            count: s.count,
                        },
                    );
                }
            }
        }


        let out_ep = interface.endpoints.out.as_ref();
        let endpoint_address = out_ep.map(|ep| ep.address.0 as u8).unwrap_or(0x01);
        let transfer_type = out_ep.map(|ep| ep.transfer_type).unwrap_or(TransferType::Interrupt);
        let backlight_byte = desc.backlight_byte();
        let dim_byte = desc.dim_byte();
        let bright_byte = desc.bright_byte();
        // Push any non-zero defaults (e.g. a display backlight) on the first flush.
        let dirty = buffer.iter().any(|&b| b != 0);

        Self {
            group_id: desc.id.clone(),
            buffer_size: desc.buffer_size,
            prefix: desc.prefix_bytes(),
            endpoint_address,
            transfer_type,
            backlight_byte,
            dim_byte,
            bright_byte,
            led_map,
            buffer,
            dirty,
        }
    }

    /// Returns the resolved byte value for `LedValue::Backlight` in this group.
    pub fn backlight_byte(&self) -> u8 {
        self.backlight_byte
    }

    /// Returns the resolved byte value for `LedValue::Dim` in this group.
    pub fn dim_byte(&self) -> u8 {
        self.dim_byte
    }

    /// Returns the resolved byte value for `LedValue::Bright` in this group.
    pub fn bright_byte(&self) -> u8 {
        self.bright_byte
    }

    #[inline]
    fn write_byte(&mut self, offset: usize, value: u8) {
        if let Some(slot) = self.buffer.get_mut(offset)
            && *slot != value {
                *slot = value;
                self.dirty = true;
            }
    }

    /// Set an LED by name.
    pub fn set(&mut self, name: &str, value: LedValue) -> bool {
        let Some(&mapping) = self.led_map.get(name) else {
            return false;
        };
        match (mapping, value) {
            (LedMapping::Single { offset, .. }, LedValue::Off) => {
                self.write_byte(offset, 0);
            }
            (LedMapping::Single { offset, levels }, LedValue::Backlight) => {
                self.write_byte(offset, levels.backlight);
            }
            (LedMapping::Single { offset, levels }, LedValue::Dim) => {
                self.write_byte(offset, levels.dim);
            }
            (LedMapping::Single { offset, levels }, LedValue::Bright) => {
                self.write_byte(offset, levels.bright);
            }
            (LedMapping::Single { offset, .. }, LedValue::Single(b)) => {
                self.write_byte(offset, b);
            }
            (LedMapping::Single { offset, .. }, LedValue::Rgb { r, g, b }) => {
                // Single LED can only take brightness, use max channel
                self.write_byte(offset, r.max(g).max(b));
            }
            (LedMapping::Rgb { r, g, b }, LedValue::Off) => {
                self.write_byte(r, 0);
                self.write_byte(g, 0);
                self.write_byte(b, 0);
            }
            (LedMapping::Rgb { r: ro, g: go, b: bo }, LedValue::Backlight) => {
                self.write_byte(ro, 18);
                self.write_byte(go, 18);
                self.write_byte(bo, 18);
            }
            (LedMapping::Rgb { r: ro, g: go, b: bo }, LedValue::Dim) => {
                self.write_byte(ro, 64);
                self.write_byte(go, 64);
                self.write_byte(bo, 64);
            }
            (LedMapping::Rgb { r: ro, g: go, b: bo }, LedValue::Bright) => {
                self.write_byte(ro, 255);
                self.write_byte(go, 255);
                self.write_byte(bo, 255);
            }
            (LedMapping::Rgb { r: ro, g: go, b: bo }, LedValue::Rgb { r, g, b }) => {
                self.write_byte(ro, r);
                self.write_byte(go, g);
                self.write_byte(bo, b);
            }
            (LedMapping::Rgb { r: ro, g: go, b: bo }, LedValue::Single(brightness)) => {
                self.write_byte(ro, brightness);
                self.write_byte(go, brightness);
                self.write_byte(bo, brightness);
            }
            (LedMapping::Indexed { offset, .. }, LedValue::Off) => {
                self.write_byte(offset, 0);
            }
            (LedMapping::Indexed { offset, levels }, LedValue::Backlight) => {
                self.write_byte(offset, levels.backlight);
            }
            (LedMapping::Indexed { offset, levels }, LedValue::Dim) => {
                self.write_byte(offset, levels.dim);
            }
            (LedMapping::Indexed { offset, levels }, LedValue::Bright) => {
                self.write_byte(offset, levels.bright);
            }
            (LedMapping::Indexed { offset, .. }, LedValue::Single(b)) => {
                // Direct raw palette/intensity byte
                self.write_byte(offset, b);
            }
            (LedMapping::Indexed { offset, .. }, LedValue::Rgb { r, g, b }) => {
                // Convert RGB to NI packed palette byte
                self.write_byte(offset, LedValue::to_ni_palette_byte(r, g, b));
            }
            (LedMapping::Strip { .. }, _) => {
                // Strips are set via set_strip()
                return false;
            }
        }
        true
    }

    /// Set a strip LED array by name.
    pub fn set_strip(&mut self, name: &str, values: &[u8]) -> bool {
        let Some(&LedMapping::Strip { offset, count }) = self.led_map.get(name) else {
            return false;
        };
        let n = values.len().min(count);
        for (i, &val) in values[..n].iter().enumerate() {
            self.write_byte(offset + i, val);
        }
        true
    }

    /// Returns true if the LED buffer has changed since last flush.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Build the wire-format buffer (prefix bytes + LED data) and clear dirty flag.
    pub fn flush(&mut self) -> Option<Vec<u8>> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        let mut wire = Vec::with_capacity(self.prefix.len() + self.buffer_size);
        wire.extend_from_slice(&self.prefix);
        wire.extend_from_slice(&self.buffer);
        Some(wire)
    }

    /// Mark the whole buffer as unsent, so the next [`flush`](Self::flush)
    /// writes every LED again. Used when something else may have changed the
    /// device's LEDs.
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Clear all LEDs to off.
    pub fn clear(&mut self) {
        self.buffer.fill(0);
        self.dirty = true;
    }

    /// The LED group ID (e.g. "left_deck", "right_deck", "mixer").
    pub fn group_id(&self) -> &str {
        &self.group_id
    }

    /// The USB endpoint address for LED writes.
    pub fn endpoint(&self) -> u8 {
        self.endpoint_address
    }

    /// The transfer type of the LED OUT endpoint.
    pub fn transfer_type(&self) -> TransferType {
        self.transfer_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::loader::DescriptorRegistry;

    fn builder() -> LedBuilder {
        let desc: LedLayoutDesc = serde_json::from_str(
            r#"{ "id": "bank", "interface": "control", "buffer_size": 2, "prefix": ["0x0c", "0x00"],
                 "items": [ { "type": "single", "name": "play", "offset": 0 },
                            { "type": "single", "name": "rec", "offset": 1, "default": 9 } ] }"#,
        )
        .unwrap();
        let iface: InterfaceDesc = serde_json::from_str(
            r#"{ "id": "control", "number": 0,
                 "endpoints": { "out": { "address": "0x01", "type": "bulk" } } }"#,
        )
        .unwrap();
        LedBuilder::new(&desc, &iface)
    }

    #[test]
    fn defaults_flush_once_with_prefix() {
        let mut lb = builder();
        assert_eq!(lb.flush(), Some(vec![0x0c, 0x00, 0, 9]));
        assert_eq!(lb.flush(), None);
        assert_eq!(lb.transfer_type(), TransferType::Bulk);
    }

    #[test]
    fn single_values_reach_the_wire_unmasked() {
        // Every u8 (notably 128, which the MK1 backlight treats as off) is written verbatim.
        for v in 0..=255u8 {
            let mut lb = builder();
            lb.set("play", LedValue::Single(v));
            assert_eq!(lb.flush(), Some(vec![0x0c, 0x00, v, 9]), "value {v}");
        }
    }

    #[test]
    fn invalidate_resends_the_current_state() {
        let mut lb = builder();
        lb.set("play", LedValue::Single(7));
        assert_eq!(lb.flush(), Some(vec![0x0c, 0x00, 7, 9]));
        assert_eq!(lb.flush(), None);
        lb.invalidate();
        assert_eq!(lb.flush(), Some(vec![0x0c, 0x00, 7, 9]));
    }

    #[test]
    fn led_builder_protocol_dim_and_bright() {
        let iface: InterfaceDesc = serde_json::from_str(
            r#"{ "id": "control", "number": 0, "endpoints": { "out": { "address": "0x01", "type": "interrupt" } } }"#,
        )
        .unwrap();

        // 1. Default (Nhl2)
        let desc_nhl2: LedLayoutDesc = serde_json::from_str(
            r#"{ "id": "buttons", "interface": "control", "buffer_size": 2,
                 "items": [ { "type": "single", "name": "play", "offset": 0 } ] }"#,
        )
        .unwrap();
        let mut lb_nhl2 = LedBuilder::new(&desc_nhl2, &iface);
        assert_eq!(lb_nhl2.backlight_byte(), 251);
        assert_eq!(lb_nhl2.dim_byte(), 228);
        assert_eq!(lb_nhl2.bright_byte(), 158);
        lb_nhl2.set("play", LedValue::Backlight);
        assert_eq!(lb_nhl2.flush(), Some(vec![251, 0]));
        lb_nhl2.set("play", LedValue::Dim);
        assert_eq!(lb_nhl2.flush(), Some(vec![228, 0]));
        lb_nhl2.set("play", LedValue::Bright);
        assert_eq!(lb_nhl2.flush(), Some(vec![158, 0]));

        // 2. Linear 7-bit (Maschine MK3 / Jam)
        let desc_7bit: LedLayoutDesc = serde_json::from_str(
            r#"{ "id": "buttons", "interface": "control", "buffer_size": 2, "protocol": "linear_7bit",
                 "items": [ { "type": "single", "name": "play", "offset": 0 } ] }"#,
        )
        .unwrap();
        let mut lb_7bit = LedBuilder::new(&desc_7bit, &iface);
        assert_eq!(lb_7bit.backlight_byte(), 0x09);
        assert_eq!(lb_7bit.dim_byte(), 0x22);
        assert_eq!(lb_7bit.bright_byte(), 0x7F);
        lb_7bit.set("play", LedValue::Backlight);
        assert_eq!(lb_7bit.flush(), Some(vec![0x09, 0]));
        lb_7bit.set("play", LedValue::Dim);
        assert_eq!(lb_7bit.flush(), Some(vec![0x22, 0]));
        lb_7bit.set("play", LedValue::Bright);
        assert_eq!(lb_7bit.flush(), Some(vec![0x7F, 0]));

        // 3. Linear 8-bit (Maschine Studio / Mk2)
        let desc_8bit: LedLayoutDesc = serde_json::from_str(
            r#"{ "id": "buttons", "interface": "control", "buffer_size": 2, "protocol": "linear_8bit",
                 "items": [ { "type": "single", "name": "play", "offset": 0 } ] }"#,
        )
        .unwrap();
        let mut lb_8bit = LedBuilder::new(&desc_8bit, &iface);
        assert_eq!(lb_8bit.backlight_byte(), 18);
        assert_eq!(lb_8bit.dim_byte(), 68);
        assert_eq!(lb_8bit.bright_byte(), 255);
        lb_8bit.set("play", LedValue::Backlight);
        assert_eq!(lb_8bit.flush(), Some(vec![18, 0]));
        lb_8bit.set("play", LedValue::Dim);
        assert_eq!(lb_8bit.flush(), Some(vec![68, 0]));
        lb_8bit.set("play", LedValue::Bright);
        assert_eq!(lb_8bit.flush(), Some(vec![255, 0]));

        // 4. Explicit override bytes
        let desc_custom: LedLayoutDesc = serde_json::from_str(
            r#"{ "id": "buttons", "interface": "control", "buffer_size": 2, "backlight_value": 15, "dim_value": 50, "bright_value": 200,
                 "items": [ { "type": "single", "name": "play", "offset": 0 } ] }"#,
        )
        .unwrap();
        let mut lb_custom = LedBuilder::new(&desc_custom, &iface);
        assert_eq!(lb_custom.backlight_byte(), 15);
        assert_eq!(lb_custom.dim_byte(), 50);
        assert_eq!(lb_custom.bright_byte(), 200);
        lb_custom.set("play", LedValue::Backlight);
        assert_eq!(lb_custom.flush(), Some(vec![15, 0]));
        lb_custom.set("play", LedValue::Dim);
        assert_eq!(lb_custom.flush(), Some(vec![50, 0]));
        lb_custom.set("play", LedValue::Bright);
        assert_eq!(lb_custom.flush(), Some(vec![200, 0]));
    }

    #[test]
    fn mikro_mk3_led_report() {
        let mut r = DescriptorRegistry::new();
        r.load_builtins().unwrap();
        let d = r.find(0x17cc, 0x1700).unwrap();
        assert_eq!(d.leds.len(), 1);
        let mut b = LedBuilder::new(&d.leds[0], &d.interfaces[0]);
        b.set("maschine", LedValue::Single(124));
        b.set("mute", LedValue::Single(127));
        for pad in 0..16 {
            b.set(&format!("pad_{}", pad + 1), LedValue::Single(4 + pad as u8));
        }
        b.set_strip("touchstrip_leds", &[71; 25]);
        let out = b.flush().unwrap();
        assert_eq!(out.len(), 81);
        assert_eq!(out[0], 0x80);
        assert_eq!(out[1], 124);
        assert_eq!(out[39], 127);
        for pad in 0..16 {
            assert_eq!(out[40 + (3 - pad / 4) * 4 + pad % 4], 4 + pad as u8);
        }
        assert_eq!(&out[56..], &[71; 25]);
    }

    #[test]
    fn single_led_uses_max_rgb_channel() {
        let mut r = DescriptorRegistry::new();
        r.load_builtins().unwrap();
        let d = r.find(0x17cc, 0x1700).unwrap();
        let mut b = LedBuilder::new(&d.leds[0], &d.interfaces[0]);
        b.set("maschine", LedValue::Rgb { r: 0, g: 200, b: 50 });
        let out = b.flush().unwrap();
        assert_eq!(out[1], 200);
        b.set("maschine", LedValue::Rgb { r: 10, g: 20, b: 255 });
        let out = b.flush().unwrap();
        assert_eq!(out[1], 255);
    }

    #[test]
    fn per_led_level_overrides() {
        let desc: LedLayoutDesc = serde_json::from_str(
            r#"{ "id": "buttons", "interface": "control", "buffer_size": 3, "protocol": "linear_7bit",
                 "items": [
                   { "type": "indexed", "name": "sampling", "offset": 0,
                     "backlight_value": 69, "dim_value": 70, "bright_value": 71 },
                   { "type": "indexed", "name": "group_a", "offset": 1 },
                   { "type": "single", "name": "play", "offset": 2, "dim_value": 50 } ] }"#,
        )
        .unwrap();
        let interface: InterfaceDesc = serde_json::from_str(
            r#"{ "id": "control", "number": 0, "endpoints": {} }"#,
        )
        .unwrap();
        let mut lb = LedBuilder::new(&desc, &interface);
        for (level, expected) in [
            (LedValue::Backlight, [69, 68, 9]),
            (LedValue::Dim, [70, 69, 50]),
            (LedValue::Bright, [71, 71, 127]),
        ] {
            for name in ["sampling", "group_a", "play"] {
                lb.set(name, level);
            }
            assert_eq!(lb.buffer, expected, "{level:?}");
        }
    }
}
