use std::sync::Arc;

use nusb::{DeviceInfo, MaybeFuture};

use crate::core::descriptor::DeviceDescriptor;
use crate::core::event::DeviceId;
use crate::device::loader::DescriptorRegistry;

/// Result of scanning for devices that match loaded descriptors.
pub struct DetectedDevice {
    pub device_id: DeviceId,
    pub descriptor: Arc<DeviceDescriptor>,
    pub usb_info: DeviceInfo,
}

/// Filter criteria for specifying which hardware controllers an application supports.
#[derive(Clone)]
pub enum ControllerFilter {
    /// Matches any controller supported by loaded descriptors.
    Any,
    /// Matches a controller by name or substring (e.g. `"Maschine Mk3"`, `"maschine_mk3"`, or hex `"0x17cc:0x1600"`).
    Name(String),
    /// Matches any controller in the provided list of names/substrings.
    Names(Vec<String>),
    /// Matches a specific USB Vendor ID and Product ID.
    Id(u16, u16),
    /// Matches any USB Vendor ID and Product ID in the list.
    Ids(Vec<(u16, u16)>),
    /// Custom predicate matching a [`DeviceDescriptor`].
    Custom(Arc<dyn Fn(&DeviceDescriptor) -> bool + Send + Sync>),
}

impl std::fmt::Debug for ControllerFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Any => write!(f, "Any"),
            Self::Name(n) => write!(f, "Name({:?})", n),
            Self::Names(list) => write!(f, "Names({:?})", list),
            Self::Id(vid, pid) => write!(f, "Id(0x{:04x}, 0x{:04x})", vid, pid),
            Self::Ids(list) => write!(f, "Ids({:?})", list),
            Self::Custom(_) => write!(f, "Custom(<predicate>)"),
        }
    }
}

impl ControllerFilter {
    /// Match any controller.
    pub fn any() -> Self {
        Self::Any
    }

    /// Match a controller by name or substring.
    pub fn name(s: impl Into<String>) -> Self {
        Self::Name(s.into())
    }

    /// Match any controller in the list of names.
    pub fn names<I, S>(list: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::Names(list.into_iter().map(Into::into).collect())
    }

    /// Match a controller by USB (Vendor ID, Product ID).
    pub fn id(vid: u16, pid: u16) -> Self {
        Self::Id(vid, pid)
    }

    /// Match any controller in the list of (Vendor ID, Product ID) pairs.
    pub fn ids<I>(list: I) -> Self
    where
        I: IntoIterator<Item = (u16, u16)>,
    {
        Self::Ids(list.into_iter().collect())
    }

    /// Match using a custom predicate function on the [`DeviceDescriptor`].
    pub fn custom(f: impl Fn(&DeviceDescriptor) -> bool + Send + Sync + 'static) -> Self {
        Self::Custom(Arc::new(f))
    }

    /// Whether this filter matches the given [`DeviceDescriptor`].
    pub fn matches(&self, desc: &DeviceDescriptor) -> bool {
        match self {
            Self::Any => true,
            Self::Name(name) => matches_target_name(desc, name),
            Self::Names(list) => list.iter().any(|name| matches_target_name(desc, name)),
            Self::Id(vid, pid) => desc.vendor_id.0 == *vid && desc.product_id.0 == *pid,
            Self::Ids(list) => list.iter().any(|(vid, pid)| desc.vendor_id.0 == *vid && desc.product_id.0 == *pid),
            Self::Custom(predicate) => predicate(desc),
        }
    }

    /// User-friendly display string of the supported controllers for console and log output.
    pub fn display_name(&self, registry: &DescriptorRegistry) -> String {
        match self {
            Self::Any => "any compatible controller".to_string(),
            Self::Name(name) => {
                let matching: Vec<_> = registry.all().filter(|d| matches_target_name(d, name)).collect();
                if matching.len() == 1 {
                    matching[0].name.clone()
                } else {
                    name.clone()
                }
            }
            Self::Names(list) => {
                let resolved: Vec<String> = list
                    .iter()
                    .map(|name| {
                        let matching: Vec<_> = registry.all().filter(|d| matches_target_name(d, name)).collect();
                        if matching.len() == 1 {
                            matching[0].name.clone()
                        } else {
                            name.clone()
                        }
                    })
                    .collect();
                resolved.join(", ")
            }
            Self::Id(vid, pid) => {
                if let Some(desc) = registry.find(*vid, *pid) {
                    format!("{} (0x{:04x}:0x{:04x})", desc.name, vid, pid)
                } else {
                    format!("0x{:04x}:0x{:04x}", vid, pid)
                }
            }
            Self::Ids(list) => {
                let names: Vec<String> = list
                    .iter()
                    .map(|(vid, pid)| {
                        if let Some(desc) = registry.find(*vid, *pid) {
                            format!("{} (0x{:04x}:0x{:04x})", desc.name, vid, pid)
                        } else {
                            format!("0x{:04x}:0x{:04x}", vid, pid)
                        }
                    })
                    .collect();
                names.join(", ")
            }
            Self::Custom(_) => "custom controller filter".to_string(),
        }
    }
}

fn matches_target_name(desc: &DeviceDescriptor, target: &str) -> bool {
    let target = target.trim();
    if target.is_empty() {
        return false;
    }

    // Check if target is formatted as hex VID:PID (e.g. "0x17cc:0x1600" or "17cc:1600")
    if let Some((vid_str, pid_str)) = target.split_once(':') {
        let vid_clean = vid_str.trim().trim_start_matches("0x").trim_start_matches("0X");
        let pid_clean = pid_str.trim().trim_start_matches("0x").trim_start_matches("0X");
        if let (Ok(vid), Ok(pid)) = (u16::from_str_radix(vid_clean, 16), u16::from_str_radix(pid_clean, 16)) {
            return desc.vendor_id.0 == vid && desc.product_id.0 == pid;
        }
    }

    // Direct case-insensitive substring
    if desc.name.to_lowercase().contains(&target.to_lowercase()) {
        return true;
    }

    // Normalized alphanumeric match (stripping spaces, underscores, dashes)
    let normalize = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect()
    };
    let norm_desc = normalize(&desc.name);
    let norm_target = normalize(target);
    norm_desc.contains(&norm_target)
}

impl From<&str> for ControllerFilter {
    fn from(s: &str) -> Self {
        Self::Name(s.to_string())
    }
}

impl From<String> for ControllerFilter {
    fn from(s: String) -> Self {
        Self::Name(s)
    }
}

impl From<&[&str]> for ControllerFilter {
    fn from(list: &[&str]) -> Self {
        Self::Names(list.iter().map(|s| s.to_string()).collect())
    }
}

impl<const N: usize> From<[&str; N]> for ControllerFilter {
    fn from(arr: [&str; N]) -> Self {
        Self::Names(arr.iter().map(|s| s.to_string()).collect())
    }
}

impl<const N: usize> From<&[&str; N]> for ControllerFilter {
    fn from(arr: &[&str; N]) -> Self {
        Self::Names(arr.iter().map(|s| s.to_string()).collect())
    }
}

impl From<&[String]> for ControllerFilter {
    fn from(list: &[String]) -> Self {
        Self::Names(list.to_vec())
    }
}

impl From<Vec<String>> for ControllerFilter {
    fn from(v: Vec<String>) -> Self {
        Self::Names(v)
    }
}

impl From<Vec<&str>> for ControllerFilter {
    fn from(v: Vec<&str>) -> Self {
        Self::Names(v.into_iter().map(str::to_string).collect())
    }
}

impl From<(u16, u16)> for ControllerFilter {
    fn from((vid, pid): (u16, u16)) -> Self {
        Self::Id(vid, pid)
    }
}

impl From<&[(u16, u16)]> for ControllerFilter {
    fn from(list: &[(u16, u16)]) -> Self {
        Self::Ids(list.to_vec())
    }
}

impl<const N: usize> From<[(u16, u16); N]> for ControllerFilter {
    fn from(arr: [(u16, u16); N]) -> Self {
        Self::Ids(arr.to_vec())
    }
}

impl<const N: usize> From<&[(u16, u16); N]> for ControllerFilter {
    fn from(arr: &[(u16, u16); N]) -> Self {
        Self::Ids(arr.to_vec())
    }
}

impl From<Vec<(u16, u16)>> for ControllerFilter {
    fn from(v: Vec<(u16, u16)>) -> Self {
        Self::Ids(v)
    }
}

/// Scan all currently connected USB devices and return those matching
/// any loaded descriptor.
pub fn scan_devices(registry: &DescriptorRegistry) -> Vec<DetectedDevice> {
    let mut found = Vec::new();

    let devices = match nusb::list_devices().wait() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Failed to list USB devices: {}", e);
            return found;
        }
    };

    for info in devices {
        let vid = info.vendor_id();
        let pid = info.product_id();

        if let Some(desc) = registry.find(vid, pid) {
            #[cfg(target_os = "linux")]
            let bus = info.busnum();
            #[cfg(not(target_os = "linux"))]
            let bus = info.bus_id().parse::<u8>().unwrap_or(0);

            let device_id = DeviceId::from_usb(
                bus,
                info.device_address(),
                vid,
                pid,
            );
            found.push(DetectedDevice {
                device_id,
                descriptor: desc.clone(),
                usb_info: info,
            });
        }
    }

    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_filter_matching() {
        let mut registry = DescriptorRegistry::new();
        registry.load_builtins().unwrap();

        let mk3 = registry.find(0x17cc, 0x1600).unwrap();
        let s4_mk3 = registry.find(0x17cc, 0x1720).unwrap();

        // Name match (case-insensitive substring)
        let filter_name: ControllerFilter = "maschine mk3".into();
        assert!(filter_name.matches(mk3));
        assert!(!filter_name.matches(s4_mk3));

        // Normalized name match (with underscore)
        let filter_underscore: ControllerFilter = "maschine_mk3".into();
        assert!(filter_underscore.matches(mk3));

        // Hex VID:PID string match
        let filter_hex: ControllerFilter = "0x17cc:0x1600".into();
        assert!(filter_hex.matches(mk3));
        assert!(!filter_hex.matches(s4_mk3));

        // List of names
        let filter_names: ControllerFilter = ["Maschine Mk3", "Kontrol S4 Mk3"].into();
        assert!(filter_names.matches(mk3));
        assert!(filter_names.matches(s4_mk3));

        // Tuple ID match
        let filter_id: ControllerFilter = (0x17cc, 0x1720).into();
        assert!(!filter_id.matches(mk3));
        assert!(filter_id.matches(s4_mk3));

        // List of IDs
        let filter_ids: ControllerFilter = [(0x17cc, 0x1600), (0x17cc, 0x1720)].into();
        assert!(filter_ids.matches(mk3));
        assert!(filter_ids.matches(s4_mk3));

        // Any
        let filter_any = ControllerFilter::Any;
        assert!(filter_any.matches(mk3));
        assert!(filter_any.matches(s4_mk3));

        // Custom predicate
        let filter_custom = ControllerFilter::custom(|d| d.screens.len() >= 2);
        assert!(filter_custom.matches(mk3));

        // Display name
        assert_eq!(filter_id.display_name(&registry), "NI Kontrol S4 Mk3 (0x17cc:0x1720)");
        assert_eq!(filter_name.display_name(&registry), "NI Maschine Mk3");
    }
}
