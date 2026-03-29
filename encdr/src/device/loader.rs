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
        let mk3_json = include_str!("../../descriptors/ni_maschine_mk3.json");
        self.load_json(mk3_json)?;
        let s8_json = include_str!("../../descriptors/ni_kontrol_s8.json");
        self.load_json(s8_json)?;
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
    fn intern_strings() {
        let mut reg = DescriptorRegistry::new();
        let a = reg.intern("play");
        let b = reg.intern("play");
        assert!(std::ptr::eq(a, b));
    }
}
