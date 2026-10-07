//! MessagePack-RPC packet framing and encoders for Komplete Kontrol Mk3 ODR.

use serde::Serialize;
use crate::core::error::{EncdrError, Result};
use super::models::{
    BrowserModel, DeviceSettings, FileAsset, KkMk3Page, MixerModel, PluginChainModel, PluginData, SmartPlayData,
    ViewAddress,
};

/// RPC message type constants in the MessagePack-RPC standard specification.
pub mod msg_type {
    pub const REQUEST: u32 = 0;
    pub const RESPONSE: u32 = 1;
    pub const NOTIFICATION: u32 = 2;
}

/// Known RPC method names on the Komplete Kontrol Mk3 ODR service.
pub mod rpc_methods {
    pub const SET_PLUGIN_DATA: &str = "client_instance_parameter_page_model_set_plugin_data";
    pub const UPDATE_PARAM_VALUE: &str = "client_instance_parameter_page_model_update_parameter_value";
    pub const SET_HOST_VIEWSTATE: &str = "client_instance_parameter_page_model_set_host_owned_viewstate";
    pub const SET_DEVICE_VIEWSTATE: &str = "client_instance_parameter_page_model_set_device_owned_viewstate";
    pub const SET_LIGHTGUIDE_LEDS: &str = "client_lightguide_set_leds";
    pub const SET_MIXER_METERS: &str = "client_instance_mixer_set_meters";
    pub const SET_MIXER_TRACK_DATA: &str = "client_instance_mixer_set_track_data";
    pub const SET_PLUGIN_CHAIN: &str = "client_plugin_chain_set_data";
    pub const SET_PLUGIN_CHAIN_INDEX: &str = "client_plugin_chain_set_index";
    pub const SET_SMARTPLAY_DATA: &str = "client_smartplay_set_data";
    pub const SET_BROWSER_DATA: &str = "client_browser_set_data_model";
    pub const SET_DEVICE_SETTINGS: &str = "device_setting";
    pub const SET_PAGE: &str = "client_set_page";
    pub const SET_PROJECT_TREE: &str = "client_host_set_project_tree";
    pub const SET_SELECTED_TRACK: &str = "client_host_set_selected_track";
    pub const REGISTER_ASSET: &str = "asset_device_model_register_asset";
}

/// MessagePack-RPC framer for ODR communications.
#[derive(Debug, Default, Clone)]
pub struct OdrRpcFramer {
    next_msg_id: u32,
}

impl OdrRpcFramer {
    pub fn new() -> Self {
        Self { next_msg_id: 1 }
    }

    /// Build a one-way notification message `[2, method, [params...]]`.
    pub fn build_notification<P: Serialize>(&self, method: &str, params: P) -> Result<Vec<u8>> {
        let envelope = (msg_type::NOTIFICATION, method, params);
        rmp_serde::to_vec_named(&envelope).map_err(|e| EncdrError::Protocol(format!("MsgPack serialize error: {}", e)))
    }

    /// Build a request message with an auto-incrementing message ID: `[0, msg_id, method, [params...]]`.
    pub fn build_request<P: Serialize>(&mut self, method: &str, params: P) -> Result<(u32, Vec<u8>)> {
        let msg_id = self.next_msg_id;
        self.next_msg_id = self.next_msg_id.wrapping_add(1);
        let envelope = (msg_type::REQUEST, msg_id, method, params);
        let bytes = rmp_serde::to_vec_named(&envelope)
            .map_err(|e| EncdrError::Protocol(format!("MsgPack serialize error: {}", e)))?;
        Ok((msg_id, bytes))
    }

    // ── High-Level Helper Encoders ───────────────────────────────────────

    /// Build notification to update the active plugin/instrument model and top header banner.
    pub fn build_set_plugin_data(&self, plugin_data: &PluginData) -> Result<Vec<u8>> {
        self.build_notification(rpc_methods::SET_PLUGIN_DATA, (plugin_data,))
    }

    /// Build notification to update a single parameter's value smoothly in real-time.
    pub fn build_update_parameter_value(&self, param_index: u32, value: f32) -> Result<Vec<u8>> {
        self.build_notification(rpc_methods::UPDATE_PARAM_VALUE, (param_index, value))
    }

    /// Build notification to update the RGB Light Guide LEDs across the keybed.
    ///
    /// Takes a slice of RGB tuples `(r, g, b)` for each key.
    pub fn build_set_lightguide(&self, rgb_keys: &[(u8, u8, u8)]) -> Result<Vec<u8>> {
        let mut leds_flat = Vec::with_capacity(rgb_keys.len() * 3);
        for &(r, g, b) in rgb_keys {
            leds_flat.push(r);
            leds_flat.push(g);
            leds_flat.push(b);
        }
        self.build_notification(rpc_methods::SET_LIGHTGUIDE_LEDS, (leds_flat,))
    }

    /// Build notification to update the active serial plugin chain.
    pub fn build_set_plugin_chain(&self, chain: &PluginChainModel) -> Result<Vec<u8>> {
        self.build_notification(rpc_methods::SET_PLUGIN_CHAIN, (chain,))
    }

    /// Build notification to change the selected plugin index within the active chain.
    pub fn build_set_plugin_chain_index(&self, index: u32) -> Result<Vec<u8>> {
        self.build_notification(rpc_methods::SET_PLUGIN_CHAIN_INDEX, (index,))
    }

    /// Build notification to update the full ODR mixer state model.
    pub fn build_set_mixer_track_data(&self, mixer: &MixerModel) -> Result<Vec<u8>> {
        self.build_notification(rpc_methods::SET_MIXER_TRACK_DATA, (mixer,))
    }

    /// Build notification to update stereo VU meters in the ODR mixer view.
    pub fn build_set_mixer_meters(&self, left_meters: &[f32], right_meters: &[f32]) -> Result<Vec<u8>> {
        let pairs: Vec<(f32, f32)> = left_meters.iter().copied().zip(right_meters.iter().copied()).collect();
        self.build_notification(rpc_methods::SET_MIXER_METERS, (pairs,))
    }

    /// Build notification to update the on-device Smart Play engine (scales, chords, arpeggiator).
    pub fn build_set_smartplay_data(&self, smartplay: &SmartPlayData) -> Result<Vec<u8>> {
        self.build_notification(rpc_methods::SET_SMARTPLAY_DATA, (smartplay,))
    }

    /// Build notification to populate the on-device Sound / Preset Browser.
    pub fn build_set_browser_model(&self, browser: &BrowserModel) -> Result<Vec<u8>> {
        self.build_notification(rpc_methods::SET_BROWSER_DATA, (browser,))
    }

    /// Build notification to update hardware device preferences (brightness, velocity, standby).
    pub fn build_set_device_settings(&self, settings: &DeviceSettings) -> Result<Vec<u8>> {
        self.build_notification(rpc_methods::SET_DEVICE_SETTINGS, (settings,))
    }

    /// Build notification to switch the active screen page / view template.
    pub fn build_set_page(&self, page: KkMk3Page) -> Result<Vec<u8>> {
        self.build_notification(rpc_methods::SET_PAGE, (page,))
    }

    /// Build notification to navigate the hardware viewstate to a target section or group.
    pub fn build_navigate_view(&self, address: &ViewAddress) -> Result<Vec<u8>> {
        self.build_notification(rpc_methods::SET_HOST_VIEWSTATE, (address,))
    }

    /// Build request to register a graphic asset (PNG or JPEG) into the keyboard's flash cache.
    pub fn build_register_asset(&mut self, asset: &FileAsset) -> Result<(u32, Vec<u8>)> {
        let payload = (&asset.identifier, &asset.data);
        self.build_request(rpc_methods::REGISTER_ASSET, payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::models::{ParameterItem, PluginChainItem, RgbColor};

    #[test]
    fn test_rpc_notification_framing() {
        let framer = OdrRpcFramer::new();
        let bytes = framer.build_notification("test_method", ("hello", 123)).unwrap();

        // Decode as generic msgpack array
        let decoded: (u32, String, (String, i32)) = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(decoded.0, msg_type::NOTIFICATION);
        assert_eq!(decoded.1, "test_method");
        assert_eq!((decoded.2).0, "hello");
        assert_eq!((decoded.2).1, 123);
    }

    #[test]
    fn test_set_plugin_data_serialization() {
        let framer = OdrRpcFramer::new();
        let mut plugin = PluginData::new("Analog Synth", RgbColor::new(0, 180, 255))
            .with_background("my_custom_header");
        plugin.add_parameter(ParameterItem::knob("Cutoff", 0.75, "3.2 kHz", "Filter"));
        plugin.add_parameter(ParameterItem::toggle("Reso Boost", true, "Filter"));

        let bytes = framer.build_set_plugin_data(&plugin).unwrap();
        assert!(!bytes.is_empty());

        // Verify notification type and method name in msgpack
        let decoded: (u32, String, (PluginData,)) = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(decoded.0, msg_type::NOTIFICATION);
        assert_eq!(decoded.1, rpc_methods::SET_PLUGIN_DATA);
        assert_eq!((decoded.2).0.name, "Analog Synth");
        assert_eq!((decoded.2).0.background, Some("my_custom_header".to_string()));
        assert_eq!((decoded.2).0.parameters.len(), 2);
    }

    #[test]
    fn test_asset_registration_request() {
        let mut framer = OdrRpcFramer::new();
        let fake_png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let asset = FileAsset::new("header_logo", fake_png.clone());

        let (msg_id, bytes) = framer.build_register_asset(&asset).unwrap();
        assert_eq!(msg_id, 1);

        let decoded: (u32, u32, String, (String, Vec<u8>)) = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(decoded.0, msg_type::REQUEST);
        assert_eq!(decoded.1, 1);
        assert_eq!(decoded.2, rpc_methods::REGISTER_ASSET);
        assert_eq!((decoded.3).0, "header_logo");
        assert_eq!((decoded.3).1, fake_png);
    }

    #[test]
    fn test_plugin_chain_serialization() {
        let framer = OdrRpcFramer::new();
        let mut chain = PluginChainModel::new();
        chain.add_plugin(PluginChainItem::new("Massive X", RgbColor::new(255, 100, 0)));
        chain.add_plugin(PluginChainItem::new("Raum", RgbColor::new(0, 180, 255)).bypassed(true));
        chain.set_current_index(1);

        let bytes = framer.build_set_plugin_chain(&chain).unwrap();
        assert!(!bytes.is_empty());

        let decoded: (u32, String, (PluginChainModel,)) = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(decoded.0, msg_type::NOTIFICATION);
        assert_eq!(decoded.1, rpc_methods::SET_PLUGIN_CHAIN);
        assert_eq!((decoded.2).0.plugins.len(), 2);
        assert_eq!((decoded.2).0.current_index, 1);
    }

    #[test]
    fn test_mixer_and_smartplay_serialization() {
        let framer = OdrRpcFramer::new();

        // Mixer
        let mut mixer = MixerModel::new();
        mixer.add_track(super::super::models::MixerTrack::new("Kick", RgbColor::new(255, 0, 0)));
        let bytes = framer.build_set_mixer_track_data(&mixer).unwrap();
        assert!(!bytes.is_empty());

        // SmartPlay
        let smartplay = SmartPlayData::default();
        let bytes_sp = framer.build_set_smartplay_data(&smartplay).unwrap();
        assert!(!bytes_sp.is_empty());

        // Browser
        let mut browser = BrowserModel::default();
        browser.filters.push(super::super::models::BrowserFilter::new("Instrument", vec!["Synth".into(), "Bass".into()]));
        let bytes_br = framer.build_set_browser_model(&browser).unwrap();
        assert!(!bytes_br.is_empty());

        // Device settings
        let settings = DeviceSettings::default();
        let bytes_set = framer.build_set_device_settings(&settings).unwrap();
        assert!(!bytes_set.is_empty());
    }
}
