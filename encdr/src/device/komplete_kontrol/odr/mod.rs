//! On-Device Rendering (ODR) MessagePack-RPC stack for Komplete Kontrol S-Series Mk3.
//!
//! Provides:
//! - [`models`]: Typed data structures for plugins, parameters, colors, layouts, and assets.
//! - [`rpc`]: MessagePack-RPC framer and encoders for hardware bulk transport and TCP sockets.

pub mod models;
pub mod rpc;

pub use models::{FileAsset, LayoutMode, ParameterItem, PluginData, RgbColor, ViewAddress, WidgetDisplayType};
pub use rpc::OdrRpcFramer;
