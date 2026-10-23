//! # encdr-skia
//!
//! GPU-accelerated Skia 2D screen renderer for hardware controller screens.
//!
//! Provides hardware-accelerated 2D vector graphics rendering for Native Instruments
//! controller displays (such as Maschine MK3, Studio, Traktor S8, D2, S5, and Komplete Kontrol keyboards).
//!
//! Rendering is executed on the GPU via Vulkan render targets (with automatic fallback to CPU rasterization
//! if hardware acceleration is unavailable), and the rendered frames are submitted directly to encdr's
//! pipeline for format conversion, dirty-rect diffing, and USB bulk transfer.
//!
//! ## Single Screen Example
//!
//! ```no_run
//! use encdr::{Encdr, EncdrConfig};
//! use encdr_skia::{Color, Paint, SkiaScreenView};
//!
//! let encdr = Encdr::new(EncdrConfig::default()).unwrap();
//! # let device_id = encdr.connected_devices()[0];
//! let mut screen = SkiaScreenView::new(&encdr, device_id, "screen").unwrap();
//!
//! screen.draw(&encdr, |canvas| {
//!     canvas.clear(Color::BLACK);
//!     let mut paint = Paint::default();
//!     paint.set_color(Color::RED);
//!     paint.set_anti_alias(true);
//!     canvas.draw_circle((100.0, 100.0), 40.0, &paint);
//! }).unwrap();
//! ```
//!
//! ## Dual Screen Example
//!
//! ```no_run
//! use encdr::{Encdr, EncdrConfig};
//! use encdr_skia::{Color, Paint, SkiaDualScreenView};
//!
//! let encdr = Encdr::new(EncdrConfig::default()).unwrap();
//! # let device_id = encdr.connected_devices()[0];
//! let mut dual_screen = SkiaDualScreenView::new(&encdr, device_id, "left_screen", "right_screen").unwrap();
//!
//! dual_screen.draw(&encdr, |canvas| {
//!     canvas.clear(Color::from_rgb(20, 20, 25));
//!     // Draw across the full dual display canvas (e.g. 960x272)
//! }).unwrap();
//! ```

pub mod dual_screen;
pub mod gpu;
pub mod screen;

pub use dual_screen::SkiaDualScreenView;
pub use gpu::{SkiaGpuContext, VulkanContext};
pub use screen::SkiaScreenView;

// Re-export core Skia types for convenience
pub use skia_safe::{
    AlphaType, BlendMode, Canvas, ClipOp, Color, Color4f, ColorSpace, ColorType, Font, FontStyle,
    Image, ImageInfo, Matrix, Paint, PaintStyle, Path, PathBuilder, PathDirection, Point, RRect,
    Rect, Shader, Surface, Typeface, Vector,
};
