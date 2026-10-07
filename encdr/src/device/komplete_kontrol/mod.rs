//! Native Instruments Komplete Kontrol S-Series Mk3 support.
//!
//! Exposes:
//! - [`daw`]: Complete direct DAW Remote MIDI CC & SysEx protocol.
//! - [`odr`]: On-Device Rendering (ODR) MessagePack-RPC stack, models, and asset pipeline.

pub mod daw;
pub mod odr;

pub use daw::{KkMk3DawController, KkMk3DawEvent};
pub use odr::{
    ArpConfig, BrowserFilter, BrowserModel, BrowserSoundItem, ChordConfig, DeviceSettings,
    FileAsset, KkMk3Page, LayoutMode, MixerModel, MixerTrack, OdrRpcFramer, ParameterItem, PluginChainItem,
    PluginChainModel, PluginData, RgbColor, ScaleConfig, SmartPlayData, ViewAddress, WidgetDisplayType,
};
