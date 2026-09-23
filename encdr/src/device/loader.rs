use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::core::descriptor::DeviceDescriptor;
use crate::core::error::{EncdrError, Result};

/// Registry of loaded device descriptors, keyed by (vendor_id, product_id).
#[derive(Debug, Default)]
pub struct DescriptorRegistry {
    descriptors: HashMap<(u16, u16), Arc<DeviceDescriptor>>,
    /// Interned strings: control names → &'static str.
    /// We leak these because descriptors live for the program lifetime.
    interned: HashMap<String, &'static str>,
}

impl DescriptorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load all built-in descriptors shipped with encdr.
    pub fn load_builtins(&mut self) -> Result<()> {
        // Embedded at compile time
        let d2_json = include_str!("../../descriptors/ni_kontrol_d2.json");
        self.load_json(d2_json)?;
        let mk2_json = include_str!("../../descriptors/ni_maschine_mk2.json");
        self.load_json(mk2_json)?;
        let mk3_json = include_str!("../../descriptors/ni_maschine_mk3.json");
        self.load_json(mk3_json)?;
        let s8_json = include_str!("../../descriptors/ni_kontrol_s8.json");
        self.load_json(s8_json)?;
        let s49_json = include_str!("../../descriptors/ni_komplete_kontrol_s49_mk2.json");
        self.load_json(s49_json)?;
        let s61_json = include_str!("../../descriptors/ni_komplete_kontrol_s61_mk2.json");
        self.load_json(s61_json)?;
        let s88_json = include_str!("../../descriptors/ni_komplete_kontrol_s88_mk2.json");
        self.load_json(s88_json)?;
        Ok(())
    }


    /// Load all .json files from a directory.
    pub fn load_dir(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if !path.is_dir() {
            return Err(EncdrError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Not a directory: {}", path.display()),
            )));
        }
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "json") {
                let contents = std::fs::read_to_string(&p)?;
                self.load_json(&contents)?;
            }
        }
        Ok(())
    }

    /// Load a single JSON descriptor string.
    pub fn load_json(&mut self, json: &str) -> Result<Arc<DeviceDescriptor>> {
        let desc: DeviceDescriptor = serde_json::from_str(json)?;
        let key = (desc.vendor_id.0, desc.product_id.0);
        let arc = Arc::new(desc);
        self.descriptors.insert(key, arc.clone());
        Ok(arc)
    }

    /// Look up a descriptor by USB vendor/product ID.
    pub fn find(&self, vendor_id: u16, product_id: u16) -> Option<&Arc<DeviceDescriptor>> {
        self.descriptors.get(&(vendor_id, product_id))
    }

    /// Get all loaded descriptors.
    pub fn all(&self) -> impl Iterator<Item = &Arc<DeviceDescriptor>> {
        self.descriptors.values()
    }

    /// Get all known (vendor_id, product_id) pairs for hotplug matching.
    pub fn known_ids(&self) -> Vec<(u16, u16)> {
        self.descriptors.keys().copied().collect()
    }

    /// Intern a string, returning a &'static str. If the string was already
    /// interned, returns the same pointer. The leaked memory is tiny (a few KB
    /// for all control names) and lives for the program lifetime.
    pub fn intern(&mut self, s: &str) -> &'static str {
        if let Some(&interned) = self.interned.get(s) {
            return interned;
        }
        let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
        self.interned.insert(s.to_owned(), leaked);
        leaked
    }

    /// Intern all control names from a descriptor, returning a map of
    /// original name → &'static str for use in event emission.
    pub fn intern_descriptor_names(
        &mut self,
        desc: &DeviceDescriptor,
    ) -> HashMap<String, &'static str> {
        let mut map = HashMap::new();
        for packet in &desc.input_packets {
            for item in &packet.items {
                let name = item.name();
                let interned = self.intern(name);
                map.insert(name.to_owned(), interned);
            }
        }
        for leds in &desc.leds {
            for item in &leds.items {
                let name = item.name();
                let interned = self.intern(name);
                map.insert(name.to_owned(), interned);
            }
        }
        for screen in &desc.screens {
            let interned = self.intern(&screen.name);
            map.insert(screen.name.clone(), interned);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::descriptor::*;

    #[test]
    fn load_builtin_d2() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x1400).expect("D2 not found");
        assert_eq!(desc.name, "NI Kontrol D2");
        assert_eq!(desc.interfaces.len(), 2);
        assert_eq!(desc.input_packets.len(), 2);
        assert!(!desc.leds.is_empty());
        assert_eq!(desc.screens.len(), 1);
        assert_eq!(desc.screens[0].width, 480);
        assert_eq!(desc.screens[0].height, 272);
        assert!(desc.quirks.dual_handle);
    }

    #[test]
    fn load_builtin_mk3() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x1600).expect("Maschine Mk3 not found");
        assert_eq!(desc.name, "NI Maschine Mk3");

        let pad_leds = desc.leds.iter().find(|l| l.id == "pad_leds").expect("pad_leds layout missing");
        assert_eq!(pad_leds.prefix_byte.0, 0x81);

        // Verify pad 13 is offset 25 and pad 1 is offset 37
        let p13 = pad_leds.items.iter().find(|i| i.name() == "pad_13").expect("pad_13 missing");
        if let crate::core::descriptor::LedItemDesc::Indexed(s) = p13 {
            assert_eq!(s.offset, 25);
        } else {
            panic!("Expected pad_13 to be Indexed");
        }

        let p1 = pad_leds.items.iter().find(|i| i.name() == "pad_1").expect("pad_1 missing");
        if let crate::core::descriptor::LedItemDesc::Indexed(s) = p1 {
            assert_eq!(s.offset, 37);
        } else {
            panic!("Expected pad_1 to be Indexed");
        }
    }

    #[test]
    fn load_builtin_kk_mk2() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let s49 = reg.find(0x17cc, 0x1610).expect("S49 Mk2 not found");
        assert_eq!(s49.name, "NI Komplete Kontrol S49 Mk2");
        assert_eq!(s49.screens.len(), 2);
        assert_eq!(s49.screens[0].width, 480);
        assert_eq!(s49.screens[0].height, 272);

        let s61 = reg.find(0x17cc, 0x1620).expect("S61 Mk2 not found");
        assert_eq!(s61.name, "NI Komplete Kontrol S61 Mk2");

        let s88 = reg.find(0x17cc, 0x1630).expect("S88 Mk2 not found");
        assert_eq!(s88.name, "NI Komplete Kontrol S88 Mk2");
    }

    #[test]
    fn load_builtin_s8_mixer() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let s8 = reg.find(0x17cc, 0x1370).expect("S8 not found");
        assert_eq!(s8.name, "NI Kontrol S8");

        // Verify feature_report_leds quirk
        let feature_leds = s8.quirks.feature_report_leds.as_ref().expect("feature_report_leds quirk missing");
        assert_eq!(feature_leds.report_id.0, 0xF4);
        assert_eq!(feature_leds.interface, 5);
        assert_eq!(feature_leds.payload_length, 33);

        // Verify cue and filter LEDs
        let cue_a = feature_leds.items.get("mixer_cue_a").expect("mixer_cue_a missing");
        assert_eq!(cue_a.command.0, 0x26);
        assert_eq!(cue_a.mask.0, 0x01);

        let filter_b = feature_leds.items.get("mixer_filter_on_b").expect("mixer_filter_on_b missing");
        assert_eq!(filter_b.command.0, 0x25);
        assert_eq!(filter_b.mask.0, 0x02);

        let find_input = |target_name: &str| s8.all_inputs().find(|i| i.name() == target_name);

        assert!(find_input("mixer_gain_a").is_some());
        assert!(find_input("mixer_eq_hi_a").is_some());
        assert!(find_input("mixer_eq_mid_a").is_some());
        assert!(find_input("mixer_eq_low_a").is_some());
        assert!(find_input("mixer_filter_a").is_some());

        assert!(find_input("mixer_gain_b").is_some());
        assert!(find_input("mixer_eq_low_b").is_some());

        // Verify 1-byte Low EQ on Ch C and D
        if let Some(InputItemDesc::Slider(low_c)) = find_input("mixer_eq_low_c") {
            assert_eq!(low_c.byte, Some(96));
            assert_eq!(low_c.bits, 4);
            assert_eq!(low_c.max_value, Some(15));
        } else {
            panic!("mixer_eq_low_c not found or not Slider");
        }

        if let Some(InputItemDesc::Slider(low_d)) = find_input("mixer_eq_low_d") {
            assert_eq!(low_d.byte, Some(106));
            assert_eq!(low_d.bits, 4);
            assert_eq!(low_d.max_value, Some(15));
        } else {
            panic!("mixer_eq_low_d not found or not Slider");
        }

        // Verify tempo controls
        assert!(find_input("mixer_tempo").is_some());
        if let Some(InputItemDesc::Encoder(tempo_enc)) = find_input("tempo_encoder") {
            assert_eq!(tempo_enc.byte, 3);
            assert_eq!(tempo_enc.bits, 4);
            assert_eq!(tempo_enc.encoding, EncoderEncoding::Wrap16);
        } else {
            panic!("tempo_encoder not found or not Encoder");
        }
    }

    #[test]
    fn s8_mixer_parsing_events() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let s8 = reg.find(0x17cc, 0x1370).expect("S8 not found").clone();
        let names = reg.intern_descriptor_names(&s8);
        let mut parser = crate::device::parser::PacketParser::new(crate::core::event::DeviceId(1), &s8, names);

        let mut events = Vec::new();

        // 1. Simulate sliders packet (109 bytes, report ID 0x02)
        let mut slider_buf = vec![0u8; 109];
        slider_buf[0] = 0x02;
        // Set mixer_gain_a (bytes 69, 70) to raw value 2048 (0x0800: low=0x00, hi=0x08)
        slider_buf[69] = 0x00;
        slider_buf[70] = 0x08;
        // Set mixer_eq_low_c (byte 96) to raw value 15 (max)
        slider_buf[96] = 15;

        parser.parse(&slider_buf, &mut events);

        let gain_a_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Slider { name, .. } if *name == "mixer_gain_a"));
        assert!(gain_a_event.is_some(), "Expected mixer_gain_a slider event");
        if let Some(crate::core::event::Event::Slider { value, .. }) = gain_a_event {
            assert!((*value - 0.5).abs() < 0.01, "Expected value ~0.5, got {}", value);
        }

        let low_c_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Slider { name, .. } if *name == "mixer_eq_low_c"));
        assert!(low_c_event.is_some(), "Expected mixer_eq_low_c slider event");
        if let Some(crate::core::event::Event::Slider { value, .. }) = low_c_event {
            assert_eq!(*value, 1.0);
        }

        // 2. Simulate buttons packet (41 bytes, report ID 0x01)
        events.clear();
        let mut btn_buf = vec![0u8; 41];
        btn_buf[0] = 0x01;
        // Set mixer_tempo (byte 23 mask 0x04)
        btn_buf[23] = 0x04;
        // Initial tempo encoder reading
        btn_buf[3] = 0;
        parser.parse(&btn_buf, &mut events);

        let tempo_btn = events.iter().find(|e| matches!(e, crate::core::event::Event::Button { name, pressed, .. } if *name == "mixer_tempo" && *pressed));
        assert!(tempo_btn.is_some(), "Expected mixer_tempo pressed event");

        // Now advance tempo encoder from 0 to 1
        events.clear();
        btn_buf[3] = 1;
        parser.parse(&btn_buf, &mut events);

        let tempo_enc = events.iter().find(|e| matches!(e, crate::core::event::Event::Encoder { name, delta, .. } if *name == "tempo_encoder" && *delta == 1));
        assert!(tempo_enc.is_some(), "Expected tempo_encoder delta 1 event");
    }

    #[test]
    fn intern_strings() {
        let mut reg = DescriptorRegistry::new();
        let a = reg.intern("play");
        let b = reg.intern("play");
        assert!(std::ptr::eq(a, b));
    }
}


