use std::collections::HashMap;

use crate::core::descriptor::*;
use crate::core::led::LedValue;

/// Builds USB LED output buffers from named LED values, driven by the descriptor.
pub struct LedBuilder {
    group_id: String,
    buffer_size: usize,
    prefix_byte: u8,
    endpoint_address: u8,
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
}

impl LedBuilder {
    pub fn new(desc: &LedLayoutDesc, interface: &InterfaceDesc) -> Self {
        let mut led_map = HashMap::new();

        for item in &desc.items {
            match item {
                LedItemDesc::Single(s) => {
                    led_map.insert(
                        s.name.clone(),
                        LedMapping::Single { offset: s.offset },
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

        let endpoint_address = interface
            .endpoints
            .out
            .as_ref()
            .map(|ep| ep.address.0 as u8)
            .unwrap_or(0x01);

        Self {
            group_id: desc.id.clone(),
            buffer_size: desc.buffer_size,
            prefix_byte: desc.prefix_byte.0 as u8,
            endpoint_address,
            led_map,
            buffer: vec![0u8; desc.buffer_size],
            dirty: false,
        }
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

    /// Build the wire-format buffer (prefix byte + LED data) and clear dirty flag.
    pub fn flush(&mut self) -> Option<Vec<u8>> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        let mut wire = Vec::with_capacity(1 + self.buffer_size);
        wire.push(self.prefix_byte);
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
}
