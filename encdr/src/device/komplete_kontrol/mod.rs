//! Native Instruments Komplete Kontrol S-Series Mk3 support.
//!
//! Exposes:
//! - [`daw`]: Complete direct DAW Remote MIDI CC & SysEx protocol.
//! - [`odr`]: On-Device Rendering (ODR) MessagePack-RPC stack, models, and asset pipeline.

pub mod daw;
pub mod odr;

pub use daw::{KkMk3DawController, KkMk3DawEvent};
pub use odr::{
    sym, sym_name, try_sym, ArpConfig, BrowserFilter, BrowserModel, BrowserSoundItem, ChordConfig,
    DeviceSettings, FileAsset, FilterType, KkMk3OdrEvent, KkMk3Page, LayoutMode, MixerModel,
    MixerTrack, OdrRpcFramer, OdrStreamDecoder, ParameterItem, ParameterStyle, PedalType,
    PluginChainItem, PluginChainModel, PluginData, RgbColor, ScaleConfig, SmartPlayData,
    ViewAddress, WaveformType, WidgetDisplayType, SYMBOLS,
};
