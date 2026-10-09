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

enum LedMapping {
    Single { offset: usize },
    Rgb { r: usize, g: usize, b: usize },
    Strip { offset: usize, count: usize },
    Indexed { offset: usize },
}

impl LedBuilder {
    pub fn new(desc: &LedLayoutDesc, interface: &InterfaceDesc) -> Self {
        let mut led_map = HashMap::new();
        let mut buffer = vec![0u8; desc.buffer_size];

        for item in &desc.items {
            match item {
                LedItemDesc::Single(s) => {
                    if let Some(value) = s.default {
                        if let Some(slot) = buffer.get_mut(s.offset) {
                            *slot = value;
                        }
                    }
                    led_map.insert(
                        s.name.clone(),
                        LedMapping::Single { offset: s.offset },
                    );
                }
                LedItemDesc::Indexed(i) => {
                    led_map.insert(
                        i.name.clone(),
                        LedMapping::Indexed { offset: i.offset },
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
            dim_byte,
            bright_byte,
            led_map,
            buffer,
            dirty,
        }
    }

    /// Returns the resolved byte value for `LedValue::Dim` in this group.
    pub fn dim_byte(&self) -> u8 {
        self.dim_byte
    }

    /// Returns the resolved byte value for `LedValue::Bright` in this group.
    pub fn bright_byte(&self) -> u8 {
        self.bright_byte
    }

    /// Set an LED by name.
    pub fn set(&mut self, name: &str, value: LedValue) -> bool {
        let Some(mapping) = self.led_map.get(name) else {
            return false;
        };
        match (mapping, value) {
            (LedMapping::Single { offset }, LedValue::Off) => {
                if *offset < self.buffer.len() {
                    self.buffer[*offset] = 0;
                    self.dirty = true;
                }
            }
            (LedMapping::Single { offset }, LedValue::Dim) => {
                if *offset < self.buffer.len() {
                    self.buffer[*offset] = self.dim_byte;
                    self.dirty = true;
                }
            }
            (LedMapping::Single { offset }, LedValue::Bright) => {
                if *offset < self.buffer.len() {
                    self.buffer[*offset] = self.bright_byte;
                    self.dirty = true;
                }
            }
            (LedMapping::Single { offset }, LedValue::Single(b)) => {
                if *offset < self.buffer.len() {
                    self.buffer[*offset] = b;
                    self.dirty = true;
                }
            }
            (LedMapping::Single { offset }, LedValue::Rgb { r, .. }) => {
                // Single LED can only take brightness, use max channel
                if *offset < self.buffer.len() {
                    self.buffer[*offset] = r;
                    self.dirty = true;
                }
            }
            (LedMapping::Rgb { r, g, b }, LedValue::Off) => {
                if *r < self.buffer.len() {
                    self.buffer[*r] = 0;
                }
                if *g < self.buffer.len() {
                    self.buffer[*g] = 0;
                }
                if *b < self.buffer.len() {
                    self.buffer[*b] = 0;
                }
                self.dirty = true;
            }
            (LedMapping::Rgb { r: ro, g: go, b: bo }, LedValue::Dim) => {
                let val = 64;
                if *ro < self.buffer.len() {
                    self.buffer[*ro] = val;
                }
                if *go < self.buffer.len() {
                    self.buffer[*go] = val;
                }
                if *bo < self.buffer.len() {
                    self.buffer[*bo] = val;
                }
                self.dirty = true;
            }
            (LedMapping::Rgb { r: ro, g: go, b: bo }, LedValue::Bright) => {
                let val = 255;
                if *ro < self.buffer.len() {
                    self.buffer[*ro] = val;
                }
                if *go < self.buffer.len() {
                    self.buffer[*go] = val;
                }
                if *bo < self.buffer.len() {
                    self.buffer[*bo] = val;
                }
                self.dirty = true;
            }
            (LedMapping::Rgb { r: ro, g: go, b: bo }, LedValue::Rgb { r, g, b }) => {
                if *ro < self.buffer.len() {
                    self.buffer[*ro] = r;
                }
                if *go < self.buffer.len() {
                    self.buffer[*go] = g;
                }
                if *bo < self.buffer.len() {
                    self.buffer[*bo] = b;
                }
                self.dirty = true;
            }
            (LedMapping::Rgb { r: ro, g: go, b: bo }, LedValue::Single(brightness)) => {
                if *ro < self.buffer.len() {
                    self.buffer[*ro] = brightness;
                }
                if *go < self.buffer.len() {
                    self.buffer[*go] = brightness;
                }
                if *bo < self.buffer.len() {
                    self.buffer[*bo] = brightness;
                }
                self.dirty = true;
            }
            (LedMapping::Indexed { offset }, LedValue::Off) => {
                if *offset < self.buffer.len() {
                    self.buffer[*offset] = 0;
                    self.dirty = true;
                }
            }
            (LedMapping::Indexed { offset }, LedValue::Dim) => {
                if *offset < self.buffer.len() {
                    // White (index 17 in 1-based palette), intensity 1 (dim)
                    self.buffer[*offset] = (17 << 2) | 1;
                    self.dirty = true;
                }
            }
            (LedMapping::Indexed { offset }, LedValue::Bright) => {
                if *offset < self.buffer.len() {
                    // White (index 17 in 1-based palette), intensity 3 (bright)
                    self.buffer[*offset] = (17 << 2) | 3;
                    self.dirty = true;
                }
            }
            (LedMapping::Indexed { offset }, LedValue::Single(b)) => {
                // Direct raw palette/intensity byte
                if *offset < self.buffer.len() {
                    self.buffer[*offset] = b;
                    self.dirty = true;
                }
            }
            (LedMapping::Indexed { offset }, LedValue::Rgb { r, g, b }) => {
                // Convert RGB to NI packed palette byte
                if *offset < self.buffer.len() {
                    self.buffer[*offset] = LedValue::to_ni_palette_byte(r, g, b);
                    self.dirty = true;
                }
            }
            (LedMapping::Strip { .. }, _) => {
                // Strips are set via set_strip()
            }
        }
        true
    }


    /// Set a strip LED array by name.
    pub fn set_strip(&mut self, name: &str, values: &[u8]) -> bool {
        let Some(mapping) = self.led_map.get(name) else {
            return false;
        };
        if let LedMapping::Strip { offset, count } = mapping {
            let n = values.len().min(*count);
            for i in 0..n {
                let idx = offset + i;
                if idx < self.buffer.len() {
                    self.buffer[idx] = values[i];
                }
            }
            self.dirty = true;
            true
        } else {
            false
        }
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
        assert_eq!(lb_nhl2.dim_byte(), 228);
        assert_eq!(lb_nhl2.bright_byte(), 158);
        lb_nhl2.set("play", LedValue::Dim);
        assert_eq!(lb_nhl2.flush(), Some(vec![228, 0]));
        lb_nhl2.set("play", LedValue::Bright);
        assert_eq!(lb_nhl2.flush(), Some(vec![158, 0]));

        // 2. Linear 7-bit (Maschine Jam)
        let desc_7bit: LedLayoutDesc = serde_json::from_str(
            r#"{ "id": "buttons", "interface": "control", "buffer_size": 2, "protocol": "linear_7bit",
                 "items": [ { "type": "single", "name": "play", "offset": 0 } ] }"#,
        )
        .unwrap();
        let mut lb_7bit = LedBuilder::new(&desc_7bit, &iface);
        assert_eq!(lb_7bit.dim_byte(), 38);
        assert_eq!(lb_7bit.bright_byte(), 127);
        lb_7bit.set("play", LedValue::Dim);
        assert_eq!(lb_7bit.flush(), Some(vec![38, 0]));
        lb_7bit.set("play", LedValue::Bright);
        assert_eq!(lb_7bit.flush(), Some(vec![127, 0]));

        // 3. Linear 8-bit (Maschine Studio / Mk2)
        let desc_8bit: LedLayoutDesc = serde_json::from_str(
            r#"{ "id": "buttons", "interface": "control", "buffer_size": 2, "protocol": "linear_8bit",
                 "items": [ { "type": "single", "name": "play", "offset": 0 } ] }"#,
        )
        .unwrap();
        let mut lb_8bit = LedBuilder::new(&desc_8bit, &iface);
        assert_eq!(lb_8bit.dim_byte(), 76);
        assert_eq!(lb_8bit.bright_byte(), 255);
        lb_8bit.set("play", LedValue::Dim);
        assert_eq!(lb_8bit.flush(), Some(vec![76, 0]));
        lb_8bit.set("play", LedValue::Bright);
        assert_eq!(lb_8bit.flush(), Some(vec![255, 0]));

        // 4. Explicit override bytes
        let desc_custom: LedLayoutDesc = serde_json::from_str(
            r#"{ "id": "buttons", "interface": "control", "buffer_size": 2, "dim_value": 50, "bright_value": 200,
                 "items": [ { "type": "single", "name": "play", "offset": 0 } ] }"#,
        )
        .unwrap();
        let mut lb_custom = LedBuilder::new(&desc_custom, &iface);
        assert_eq!(lb_custom.dim_byte(), 50);
        assert_eq!(lb_custom.bright_byte(), 200);
        lb_custom.set("play", LedValue::Dim);
        assert_eq!(lb_custom.flush(), Some(vec![50, 0]));
        lb_custom.set("play", LedValue::Bright);
        assert_eq!(lb_custom.flush(), Some(vec![200, 0]));
    }
}
