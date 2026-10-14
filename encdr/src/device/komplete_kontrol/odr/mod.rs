//! On-Device Rendering (ODR) MessagePack-RPC stack for Komplete Kontrol S-Series Mk3.
//!
//! Provides:
//! - [`models`]: Typed data structures for plugins, parameters, colors, layouts, and assets.
//! - [`rpc`]: MessagePack-RPC framer and encoders for hardware bulk transport and TCP sockets.
//! - [`symbols`]: 437-symbol registry for the on-device RPC protocol.

pub mod models;
pub mod rpc;
pub mod symbols;

pub use models::{
    ArpConfig, BrowserFilter, BrowserModel, BrowserSoundItem, ChordConfig, DeviceSettings,
    FileAsset, FilterType, KkMk3Page, LayoutMode, MixerModel, MixerTrack, ParameterItem,
    ParameterStyle, PedalType, PluginChainItem, PluginChainModel, PluginData, RgbColor,
    ScaleConfig, SmartPlayData, ViewAddress, WaveformType, WidgetDisplayType,
};
pub use rpc::{KkMk3OdrEvent, OdrRpcFramer, OdrStreamDecoder};
pub use symbols::{sym, sym_name, try_sym, SYMBOLS};
