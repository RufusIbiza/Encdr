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
        let mk1_json = include_str!("../../descriptors/ni_maschine_mk1.json");
        self.load_json(mk1_json)?;
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
        let x1_mk3_json = include_str!("../../descriptors/ni_traktor_kontrol_x1_mk3.json");
        self.load_json(x1_mk3_json)?;
        let plus_json = include_str!("../../descriptors/ni_maschine_plus.json");
        self.load_json(plus_json)?;
        let studio_json = include_str!("../../descriptors/ni_maschine_studio.json");
        self.load_json(studio_json)?;
        let s5_json = include_str!("../../descriptors/ni_kontrol_s5.json");
        self.load_json(s5_json)?;
        let s2_mk1_json = include_str!("../../descriptors/ni_kontrol_s2_mk1.json");
        self.load_json(s2_mk1_json)?;
        let s2_mk2_json = include_str!("../../descriptors/ni_kontrol_s2_mk2.json");
        self.load_json(s2_mk2_json)?;
        let s4_mk2_json = include_str!("../../descriptors/ni_kontrol_s4_mk2.json");
        self.load_json(s4_mk2_json)?;
        let s4_mk3_json = include_str!("../../descriptors/ni_kontrol_s4_mk3.json");
        self.load_json(s4_mk3_json)?;
        let jam_json = include_str!("../../descriptors/ni_maschine_jam.json");
        self.load_json(jam_json)?;
        let s49_mk3_json = include_str!("../../descriptors/ni_komplete_kontrol_s49_mk3.json");
        self.load_json(s49_mk3_json)?;
        let s61_mk3_json = include_str!("../../descriptors/ni_komplete_kontrol_s61_mk3.json");
        self.load_json(s61_mk3_json)?;
        let s88_mk3_json = include_str!("../../descriptors/ni_komplete_kontrol_s88_mk3.json");
        self.load_json(s88_mk3_json)?;
        let f1_json = include_str!("../../descriptors/ni_kontrol_f1.json");
        self.load_json(f1_json)?;
        let x1_mk1_json = include_str!("../../descriptors/ni_kontrol_x1_mk1.json");
        self.load_json(x1_mk1_json)?;
        let x1_mk2_json = include_str!("../../descriptors/ni_kontrol_x1_mk2.json");
        self.load_json(x1_mk2_json)?;
        let z1_json = include_str!("../../descriptors/ni_kontrol_z1.json");
        self.load_json(z1_json)?;
        let z2_json = include_str!("../../descriptors/ni_kontrol_z2.json");
        self.load_json(z2_json)?;
        let mikro_mk1_json = include_str!("../../descriptors/ni_maschine_mikro_mk1.json");
        self.load_json(mikro_mk1_json)?;
        let mikro_mk2_json = include_str!("../../descriptors/ni_maschine_mikro_mk2.json");
        self.load_json(mikro_mk2_json)?;
        let mikro_mk3_json = include_str!("../../descriptors/ni_maschine_mikro_mk3.json");
        self.load_json(mikro_mk3_json)?;
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
        desc.validate()
            .map_err(|e| EncdrError::Descriptor(format!("{}: {}", desc.name, e)))?;
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
        assert_eq!(pad_leds.prefix_bytes(), [0x81]);

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

    #[test]
    fn load_x1_mk3_descriptor() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x2200).expect("X1 MK3 should be registered").clone();
        assert_eq!(desc.name, "NI Traktor Kontrol X1 Mk3");
        assert_eq!(desc.screens.len(), 5);
        for screen in &desc.screens {
            assert_eq!(screen.width, 128);
            assert_eq!(screen.height, 64);
            assert_eq!(screen.pixel_format, crate::core::descriptor::PixelFormat::Mono);
            assert_eq!(screen.byte_size(), 1024);
        }

        let names = reg.intern_descriptor_names(&desc);
        let mut parser = crate::device::parser::PacketParser::new(crate::core::event::DeviceId(1), &desc, names);
        let mut events = Vec::new();

        let mut buf = vec![0u8; 25];
        buf[0] = 0x01;
        buf[4] = 0x01; // left_play (byte 4 mask 0x01)
        parser.parse(&buf, &mut events);

        let play_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Button { name, pressed, .. } if *name == "left_play" && *pressed));
        assert!(play_event.is_some(), "Expected left_play pressed event");
    }

    #[test]
    fn load_maschine_plus_descriptor() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x1820).expect("Maschine Plus should be registered").clone();
        assert_eq!(desc.name, "NI Maschine Plus");
        assert_eq!(desc.screens.len(), 2);
        for screen in &desc.screens {
            assert_eq!(screen.width, 480);
            assert_eq!(screen.height, 272);
            assert_eq!(screen.pixel_format, crate::core::descriptor::PixelFormat::Bgr565Be);
        }
        assert_eq!(desc.leds.len(), 2);
    }

    #[test]
    fn load_maschine_studio_descriptor() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x1300).expect("Maschine Studio should be registered").clone();
        assert_eq!(desc.name, "NI Maschine Studio");
        assert_eq!(desc.screens.len(), 2);
        for screen in &desc.screens {
            assert_eq!(screen.width, 480);
            assert_eq!(screen.height, 272);
            assert_eq!(screen.pixel_format, crate::core::descriptor::PixelFormat::Bgr565Be);
        }
        assert_eq!(desc.leds.len(), 4);
        assert!(desc.leds.iter().any(|l| l.id == "buttons" && l.prefix_bytes() == [0x80]));
        assert!(desc.leds.iter().any(|l| l.id == "pad_leds" && l.prefix_bytes() == [0x81]));
        assert!(desc.leds.iter().any(|l| l.id == "master_meters" && l.prefix_bytes() == [0x82]));
        assert!(desc.leds.iter().any(|l| l.id == "jogwheel_ring" && l.prefix_bytes() == [0x83]));

        let names = reg.intern_descriptor_names(&desc);
        let mut parser = crate::device::parser::PacketParser::new(crate::core::event::DeviceId(1), &desc, names);
        let mut events = Vec::new();

        let mut buf = vec![0u8; 42];
        buf[0] = 0x01;
        buf[14] = 0x10; // play (byte 14 mask 0x10)
        parser.parse(&buf, &mut events);

        let play_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Button { name, pressed, .. } if *name == "play" && *pressed));
        assert!(play_event.is_some(), "Expected play pressed event on Maschine Studio");
    }

    #[test]
    fn load_kontrol_s5_descriptor() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x1420).expect("Kontrol S5 should be registered").clone();
        assert_eq!(desc.name, "NI Kontrol S5");
        assert_eq!(desc.screens.len(), 2);
        for screen in &desc.screens {
            assert_eq!(screen.width, 480);
            assert_eq!(screen.height, 272);
            assert_eq!(screen.pixel_format, crate::core::descriptor::PixelFormat::Bgr565Be);
        }
        assert_eq!(desc.leds.len(), 3);

        let names = reg.intern_descriptor_names(&desc);
        let mut parser = crate::device::parser::PacketParser::new(crate::core::event::DeviceId(1), &desc, names);
        let mut events = Vec::new();

        let mut buf = vec![0u8; 30];
        buf[9] = 0x10; // left_play (byte 9 mask 0x10)
        parser.parse(&buf, &mut events);

        let play_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Button { name, pressed, .. } if *name == "left_play" && *pressed));
        assert!(play_event.is_some(), "Expected left_play pressed event on S5");
    }

    #[test]
    fn load_kontrol_s2_mk1_descriptor() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x1101).expect("Kontrol S2 MK1 should be registered").clone();
        assert_eq!(desc.name, "NI Kontrol S2 Mk1");
        assert_eq!(desc.input_packets.len(), 2);
        assert_eq!(desc.leds.len(), 1);

        let names = reg.intern_descriptor_names(&desc);
        let mut parser = crate::device::parser::PacketParser::new(crate::core::event::DeviceId(1), &desc, names);
        let mut events = Vec::new();

        let mut buf = vec![0u8; 16];
        buf[13] = 0x10; // left_play (byte 13 mask 0x10)
        parser.parse(&buf, &mut events);

        let play_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Button { name, pressed, .. } if *name == "left_play" && *pressed));
        assert!(play_event.is_some(), "Expected left_play pressed event on S2 MK1");
    }

    #[test]
    fn load_kontrol_s2_mk2_descriptor() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x1320).expect("Kontrol S2 MK2 should be registered").clone();
        assert_eq!(desc.name, "NI Kontrol S2 Mk2");
        assert_eq!(desc.input_packets.len(), 2);
        assert_eq!(desc.leds.len(), 2);

        let names = reg.intern_descriptor_names(&desc);
        let mut parser = crate::device::parser::PacketParser::new(crate::core::event::DeviceId(1), &desc, names);
        let mut events = Vec::new();

        let mut buf = vec![0u8; 17];
        buf[11] = 0x01; // left_play (byte 11 mask 0x01)
        parser.parse(&buf, &mut events);

        let play_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Button { name, pressed, .. } if *name == "left_play" && *pressed));
        assert!(play_event.is_some(), "Expected left_play pressed event on S2 MK2");
    }

    #[test]
    fn load_kontrol_s4_mk2_descriptor() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x1310).expect("Kontrol S4 MK2 should be registered").clone();
        assert_eq!(desc.name, "NI Kontrol S4 Mk2");
        assert_eq!(desc.input_packets.len(), 2);
        assert_eq!(desc.leds.len(), 3);
        assert!(desc.leds.iter().any(|g| g.id == "loop_displays" && g.prefix_bytes() == [0xd5]));

        let names = reg.intern_descriptor_names(&desc);
        let mut parser = crate::device::parser::PacketParser::new(crate::core::event::DeviceId(1), &desc, names);
        let mut events = Vec::new();

        let mut buf = vec![0u8; 20];
        buf[13] = 0x01; // left_play (byte 13 mask 0x01)
        parser.parse(&buf, &mut events);

        let play_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Button { name, pressed, .. } if *name == "left_play" && *pressed));
        assert!(play_event.is_some(), "Expected left_play pressed event on S4 MK2");
    }

    #[test]
    fn load_kontrol_s4_mk3_descriptor() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x1720).expect("Kontrol S4 MK3 should be registered").clone();
        assert_eq!(desc.name, "NI Kontrol S4 Mk3");
        assert_eq!(desc.screens.len(), 2);
        for screen in &desc.screens {
            assert_eq!(screen.width, 320);
            assert_eq!(screen.height, 240);
            assert_eq!(screen.pixel_format, crate::core::descriptor::PixelFormat::Bgr565Be);
        }
        assert_eq!(desc.leds.len(), 4);
        assert!(desc.leds.iter().any(|l| l.id == "motor_command" && l.prefix_bytes() == [0x31]));
        assert!(desc.leds.iter().any(|l| l.id == "wheel_leds" && l.prefix_bytes() == [0x32]));

        let names = reg.intern_descriptor_names(&desc);
        let mut parser = crate::device::parser::PacketParser::new(crate::core::event::DeviceId(1), &desc, names);
        let mut events = Vec::new();

        let mut buf = vec![0u8; 25];
        buf[4] = 0x01; // left_play (byte 4 mask 0x01)
        parser.parse(&buf, &mut events);

        let play_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Button { name, pressed, .. } if *name == "left_play" && *pressed));
        assert!(play_event.is_some(), "Expected left_play pressed event on S4 MK3");
    }

    #[test]
    fn load_maschine_jam_descriptor() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x1500).expect("Maschine Jam should be registered").clone();
        assert_eq!(desc.name, "NI Maschine Jam");
        assert_eq!(desc.input_packets.len(), 2);
        assert_eq!(desc.leds.len(), 3);
        assert!(desc.leds.iter().any(|l| l.id == "button_leds" && l.prefix_bytes() == [0x80]));
        assert!(desc.leds.iter().any(|l| l.id == "matrix_and_groups" && l.prefix_bytes() == [0x81]));
        assert!(desc.leds.iter().any(|l| l.id == "touchstrip_meters" && l.prefix_bytes() == [0x82]));

        let names = reg.intern_descriptor_names(&desc);
        let mut parser = crate::device::parser::PacketParser::new(crate::core::event::DeviceId(1), &desc, names);
        let mut events = Vec::new();

        // 1. Simulate button packet (17 bytes): press "play" (byte 15, mask 0x04)
        let mut btn_buf = vec![0u8; 17];
        btn_buf[15] = 0x04;
        parser.parse(&btn_buf, &mut events);
        let play_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Button { name, pressed, .. } if *name == "play" && *pressed));
        assert!(play_event.is_some(), "Expected play button pressed event on Jam");

        // 2. Simulate matrix button: row 1, col 1 (matrix_1_1: byte 4, mask 0x04)
        events.clear();
        btn_buf[4] = 0x04;
        parser.parse(&btn_buf, &mut events);
        let matrix_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Button { name, pressed, .. } if *name == "matrix_1_1" && *pressed));
        assert!(matrix_event.is_some(), "Expected matrix_1_1 button pressed event on Jam");

        // 3. Simulate touchstrip packet (49 bytes): touchstrip 1 touched with position 512
        events.clear();
        let mut ts_buf = vec![0u8; 49];
        ts_buf[0] = 0x02;
        // strip 1 offset = 1: position at bytes 3, 4 (little-endian 512 = 0x0200)
        ts_buf[3] = 0x00;
        ts_buf[4] = 0x02;
        parser.parse(&ts_buf, &mut events);
        let slider_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Slider { name, .. } if *name == "touchstrip_1"));
        assert!(slider_event.is_some(), "Expected touchstrip_1 slider event on Jam");
        if let Some(crate::core::event::Event::Slider { value, .. }) = slider_event {
            assert!((*value - 0.5).abs() < 0.01, "Expected normalized value ~0.5, got {}", value);
        }
        let touch_event = events.iter().find(|e| matches!(e, crate::core::event::Event::Touch { name, touched, .. } if *name == "touchstrip_1_touch" && *touched));
        assert!(touch_event.is_some(), "Expected touchstrip_1_touch event on Jam");
    }

    #[test]
    fn load_builtin_kk_mk3() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let s49 = reg.find(0x17cc, 0x2100).expect("S49 Mk3 not found");
        assert_eq!(s49.name, "NI Komplete Kontrol S49 Mk3");
        assert_eq!(s49.input_packets.len(), 3);
        assert_eq!(s49.leds.len(), 3);
        let s49_lg = s49.leds.iter().find(|l| l.id == "light_guide").expect("S49 Light Guide missing");
        assert_eq!(s49_lg.buffer_size, 49);

        let s61 = reg.find(0x17cc, 0x2110).expect("S61 Mk3 not found");
        assert_eq!(s61.name, "NI Komplete Kontrol S61 Mk3");
        let s61_lg = s61.leds.iter().find(|l| l.id == "light_guide").expect("S61 Light Guide missing");
        assert_eq!(s61_lg.buffer_size, 61);

        let s88 = reg.find(0x17cc, 0x2120).expect("S88 Mk3 not found");
        assert_eq!(s88.name, "NI Komplete Kontrol S88 Mk3");
        let s88_lg = s88.leds.iter().find(|l| l.id == "light_guide").expect("S88 Light Guide missing");
        assert_eq!(s88_lg.buffer_size, 88);
    }

    #[test]
    fn load_f1_x1_z1_z2_mikro_descriptors() {
        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        // 1. Traktor Kontrol F1
        let f1 = reg.find(0x17cc, 0x1120).expect("F1 not found");
        assert_eq!(f1.name, "NI Kontrol F1");
        assert_eq!(f1.input_packets.len(), 1);
        assert_eq!(f1.leds.len(), 1);

        // 2. Traktor Kontrol X1 Mk1
        let x1_mk1 = reg.find(0x17cc, 0x2305).expect("X1 Mk1 not found");
        assert_eq!(x1_mk1.name, "NI Kontrol X1 Mk1");
        assert_eq!(x1_mk1.input_packets.len(), 1);

        // 3. Traktor Kontrol X1 Mk2
        let x1_mk2 = reg.find(0x17cc, 0x1220).expect("X1 Mk2 not found");
        assert_eq!(x1_mk2.name, "NI Kontrol X1 Mk2");
        assert_eq!(x1_mk2.input_packets.len(), 1);

        // 4. Traktor Kontrol Z1
        let z1 = reg.find(0x17cc, 0x1210).expect("Z1 not found");
        assert_eq!(z1.name, "NI Kontrol Z1");
        assert_eq!(z1.input_packets.len(), 1);

        // 5. Traktor Kontrol Z2
        let z2 = reg.find(0x17cc, 0x1230).expect("Z2 not found");
        assert_eq!(z2.name, "NI Kontrol Z2");
        assert_eq!(z2.input_packets.len(), 1);

        // 6. Maschine Mikro Mk1
        let mm1 = reg.find(0x17cc, 0x1110).expect("Mikro Mk1 not found");
        assert_eq!(mm1.name, "NI Maschine Mikro Mk1");
        assert_eq!(mm1.screens.len(), 1);

        // 7. Maschine Mikro Mk2
        let mm2 = reg.find(0x17cc, 0x1200).expect("Mikro Mk2 not found");
        assert_eq!(mm2.name, "NI Maschine Mikro Mk2");
        assert_eq!(mm2.screens.len(), 1);

        // 8. Maschine Mikro Mk3
        let mm3 = reg.find(0x17cc, 0x1700).expect("Mikro Mk3 not found");
        assert_eq!(mm3.name, "NI Maschine Mikro Mk3");
        assert_eq!(mm3.screens.len(), 1);
        assert_eq!(mm3.input_packets.len(), 3);
        assert_eq!(mm3.leds.len(), 2);
    }

    #[test]
    fn load_maschine_mk1_descriptor() {
        use crate::core::event::Event;

        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();

        let desc = reg.find(0x17cc, 0x0808).expect("Maschine Mk1 should be registered").clone();
        assert_eq!(desc.name, "NI Maschine Mk1");
        assert_eq!(desc.screens.len(), 2);
        for screen in &desc.screens {
            assert_eq!(screen.pixel_format, crate::core::descriptor::PixelFormat::St7529Gray5);
            assert_eq!(screen.byte_size(), 10880);
        }
        assert_eq!(
            desc.leds.iter().map(|l| l.prefix_bytes()).collect::<Vec<_>>(),
            vec![vec![0x0c, 0x00], vec![0x0c, 0x1e]]
        );

        let names = reg.intern_descriptor_names(&desc);
        let mut parser = crate::device::parser::PacketParser::new(crate::core::event::DeviceId(1), &desc, names);
        let mut events = Vec::new();

        // Button report captured from hardware: Mute pressed. The 0x40 in
        // byte 6 is an undocumented toggle bit and must not produce events.
        parser.parse_from("control", &[0x04, 0x01, 0, 0, 0, 0, 0x40, 0x85], &mut events);
        assert!(
            matches!(events.as_slice(), [Event::Button { name: "mute", pressed: true, .. }]),
            "{events:?}"
        );

        // Knob report captured at rest sets the baseline; then the `a` tap of
        // screen_encoder_1 (pair at bytes 21..22) rises: 197 -> 225.
        let mut knobs = vec![
            0x02, 0xa0, 0x15, 0xb7, 0xcc, 0x03, 0x71, 0x85, 0x03, 0x1f, 0xa2, 0x85,
            0x00, 0x3b, 0x3d, 0x5d, 0x17, 0x19, 0x5f, 0x0c, 0x86, 0xf7, 0x65,
        ];
        knobs.resize(33, 0);
        events.clear();
        parser.parse_from("control", &knobs, &mut events);
        assert!(events.is_empty());
        knobs[22] = 0x75;
        parser.parse_from("control", &knobs, &mut events);
        match events.as_slice() {
            [Event::EncoderFine { name: "screen_encoder_1", delta, .. }] => {
                assert!((*delta - 0.028).abs() < 1e-6, "{delta}")
            }
            other => panic!("unexpected events {other:?}"),
        }

        // Pad stream: words are self-identifying, so a frame starting
        // mid-cycle still maps id 12 to pad_1.
        let mut pads = Vec::new();
        for id in (8u16..16).chain(0..8) {
            let pressure = if id == 12 { 3000 } else { 0 };
            pads.extend_from_slice(&(id << 12 | pressure).to_le_bytes());
        }
        events.clear();
        parser.parse_from("pads", &pads, &mut events);
        assert!(matches!(events[0], Event::Button { name: "pad_1", pressed: true, .. }), "{events:?}");
        assert!(matches!(events[1], Event::Grid { name: "pad_1", index: 12, .. }), "{events:?}");

        // An LED ack (0x0c) on the control interface is ignored.
        events.clear();
        parser.parse_from("control", &[0x0c], &mut events);
        assert!(events.is_empty());
    }

    #[test]
    fn descriptor_validation_rejects_ambiguous_or_incomplete_fields() {
        fn load(screens: &str, leds: &str, quirks: &str) -> Result<Arc<DeviceDescriptor>> {
            DescriptorRegistry::new().load_json(&format!(
                r#"{{ "name": "Test", "manufacturer": "Test", "vendor_id": "0x1234", "product_id": "0x5678",
                      "interfaces": [ {{ "id": "control", "number": 0,
                          "endpoints": {{ "out": {{ "address": "0x01", "type": "bulk" }} }} }} ],
                      "input_packets": [], "screens": [{screens}], "leds": [{leds}], "quirks": {{ {quirks} }} }}"#
            ))
        }
        let screen = |extra: &str| {
            format!(r#"{{ "name": "main", "interface": "control", "width": 8, "height": 8, "pixel_format": "mono" {extra} }}"#)
        };
        let leds = |prefix: &str| {
            format!(r#"{{ "id": "bank", "interface": "control", "buffer_size": 1, {prefix} "items": [] }}"#)
        };

        // Valid baselines.
        assert!(load(&screen(r#", "full_blit": { "header": "0xe0", "footer": "" }"#), "", "").is_ok());
        assert!(load(&screen(r#", "protocol": { "type": "ni_st7529", "display": 0 }"#), "", "").is_ok());
        assert!(load("", &leds(r#""prefix_byte": "0x80","#), "").is_ok());
        assert!(load("", &leds(r#""prefix": ["0x0c", "0x1e"],"#), "").is_ok());
        assert!(load("", "", r#""init_writes": [ { "interface": "control", "data": ["0x0b"] } ]"#).is_ok());

        // Screen with neither framing nor protocol.
        assert!(load(&screen(""), "", "").is_err());
        // LED group with no prefix, or with both kinds.
        assert!(load("", &leds(""), "").is_err());
        assert!(load("", &leds(r#""prefix_byte": "0x80", "prefix": ["0x0c"],"#), "").is_err());
        // Bytes that don't fit in a u8.
        assert!(load("", &leds(r#""prefix": ["0x10c"],"#), "").is_err());
        assert!(load("", "", r#""init_writes": [ { "interface": "control", "data": ["0x1ff"] } ]"#).is_err());
        // Malformed hex and unknown interfaces.
        assert!(load("", "", r#""init_writes": [ { "interface": "control", "data": ["0x5O"] } ]"#).is_err());
        assert!(load("", "", r#""init_writes": [ { "interface": "nope", "data": ["0x0b"] } ]"#).is_err());
    }

    #[test]
    fn maschine_mk1_held_pad_survives_stream_pause() {
        use crate::core::event::Event;

        let mut reg = DescriptorRegistry::new();
        reg.load_builtins().unwrap();
        let desc = reg.find(0x17cc, 0x0808).unwrap().clone();
        let names = reg.intern_descriptor_names(&desc);
        let mut parser = crate::device::parser::PacketParser::new(crate::core::event::DeviceId(1), &desc, names);
        let mut events = Vec::new();

        // Press pad_1 (id 12) and keep reporting it held.
        let held = (12u16 << 12 | 2000).to_le_bytes();
        for _ in 0..4 {
            parser.parse_from("pads", &held, &mut events);
        }
        assert!(matches!(events[0], Event::Button { name: "pad_1", pressed: true, .. }));

        // A pause longer than the 150 ms isolated-tap timeout must not
        // synthesize a release for a sustained hold.
        std::thread::sleep(std::time::Duration::from_millis(160));
        events.clear();
        parser.check_pad_timeouts(&mut events);
        assert!(events.is_empty(), "{events:?}");
    }
}




