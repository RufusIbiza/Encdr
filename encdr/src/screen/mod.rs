pub mod gpu;
pub mod gpu_pipeline;
pub mod protocol;

use std::sync::Arc;

use crate::core::descriptor::{PixelFormat, ScreenDesc};

pub use gpu::GpuContext;
pub use gpu_pipeline::GpuConvertPipeline;

/// Manages the screen pipeline for a single screen: format conversion,
/// frame diffing, and partial blit extraction.
pub struct ScreenManager {
    width: u16,
    height: u16,
    pixel_format: PixelFormat,
    /// Previous frame in device-native format (for diffing)
    prev_frame: Vec<u8>,
    /// Frame counter for periodic keyframes
    frame_count: u64,
    /// GPU compute pipeline for format conversion (lazily created)
    gpu_pipeline: Option<GpuConvertPipeline>,
}

/// Result of submitting a frame: the bytes to send over USB (if any).
pub struct BlitResult {
    pub data: Vec<u8>,
    pub is_partial: bool,
}

impl ScreenManager {
    pub fn new(desc: &ScreenDesc, gpu: Option<Arc<GpuContext>>) -> Self {
        let byte_size = desc.byte_size();
        let gpu_pipeline = gpu.map(GpuConvertPipeline::new);
        Self {
            width: desc.width,
            height: desc.height,
            pixel_format: desc.pixel_format,
            prev_frame: vec![0u8; byte_size],
            frame_count: 0,
            gpu_pipeline,
        }
    }

    /// Submit a new frame. Returns the USB transfer buffer if the frame
    /// has changed (or it's time for a keyframe), or None if identical.
    pub fn submit(
        &mut self,
        pixels: &[u8],
        input_format: PixelFormat,
        screen_desc: &ScreenDesc,
    ) -> Option<Vec<u8>> {
        // Step 1: Format conversion (if needed)
        let native_pixels = if input_format == self.pixel_format {
            pixels.to_vec()
        } else if let Some(ref mut gpu) = self.gpu_pipeline {
            // Use GPU pipeline for RGBA→BGR565 conversion
            match (input_format, self.pixel_format) {
                (PixelFormat::Rgba8888, PixelFormat::Bgr565Be) => {
                    futures_lite::future::block_on(gpu.convert_rgba_to_bgr565(pixels))
                }
                _ => convert_format(pixels, input_format, self.pixel_format, self.width, self.height),
            }
        } else {
            convert_format(pixels, input_format, self.pixel_format, self.width, self.height)
        };

        self.frame_count += 1;

        // Step 2: Determine if we need a keyframe (full blit every ~60 frames / ~2s at 30fps)
        let force_full = self.frame_count % 60 == 0;

        // Step 3: Frame diff
        if !force_full {
            if let Some(partial_blit) = &screen_desc.partial_blit {
                if partial_blit.supported {
                    let dirty = compute_dirty_rect(
                        &self.prev_frame,
                        &native_pixels,
                        self.width,
                        self.height,
                        self.pixel_format.bytes_per_pixel(),
                        partial_blit.x_align,
                        partial_blit.y_align,
                    );

                    // Update prev frame
                    self.prev_frame.copy_from_slice(&native_pixels);

                    match dirty {
                        DirtyRect::Clean => return None,
                        DirtyRect::Partial { x, y, w, h } => {
                            let region_pixels = extract_region(
                                &native_pixels,
                                self.width,
                                self.pixel_format.bytes_per_pixel(),
                                x,
                                y,
                                w,
                                h,
                            );
                            return Some(protocol::build_partial_blit(
                                screen_desc,
                                x,
                                y,
                                w,
                                h,
                                &region_pixels,
                            ));
                        }
                        DirtyRect::Full => {
                            // Fall through to full blit
                        }
                    }
                }
            }
        }

        // Full blit
        self.prev_frame.copy_from_slice(&native_pixels);
        Some(protocol::build_full_blit(screen_desc, &native_pixels))
    }
}

// ── Format conversion ──────────────────────────────────────────────────────

fn convert_format(
    pixels: &[u8],
    from: PixelFormat,
    to: PixelFormat,
    width: u16,
    height: u16,
) -> Vec<u8> {
    match (from, to) {
        (PixelFormat::Rgba8888, PixelFormat::Bgr565Be) => {
            rgba8_to_bgr565_be(pixels, width as usize, height as usize)
        }
        (PixelFormat::Rgb888, PixelFormat::Bgr565Be) => {
            rgb8_to_bgr565_be(pixels, width as usize, height as usize)
        }
        _ => {
            tracing::warn!("Unsupported format conversion: {:?} -> {:?}", from, to);
            pixels.to_vec()
        }
    }
}

/// Convert RGBA8888 to BGR565 big-endian (CPU fallback).
pub(crate) fn rgba8_to_bgr565_be(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let pixel_count = width * height;
    let mut out = Vec::with_capacity(pixel_count * 2);

    for i in 0..pixel_count {
        let offset = i * 4;
        if offset + 2 >= rgba.len() {
            out.extend_from_slice(&[0, 0]);
            continue;
        }
        let r = rgba[offset];
        let g = rgba[offset + 1];
        let b = rgba[offset + 2];

        let r5 = (r as u16 >> 3) & 0x1f;
        let g6 = (g as u16 >> 2) & 0x3f;
        let b5 = (b as u16 >> 3) & 0x1f;
        let bgr565 = (r5 << 11) | (g6 << 5) | b5;

        // Big-endian
        out.push((bgr565 >> 8) as u8);
        out.push((bgr565 & 0xff) as u8);
    }

    out
}

/// Convert RGB888 to BGR565 big-endian.
fn rgb8_to_bgr565_be(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    let pixel_count = width * height;
    let mut out = Vec::with_capacity(pixel_count * 2);

    for i in 0..pixel_count {
        let offset = i * 3;
        if offset + 2 >= rgb.len() {
            out.extend_from_slice(&[0, 0]);
            continue;
        }
        let r = rgb[offset];
        let g = rgb[offset + 1];
        let b = rgb[offset + 2];

        let r5 = (r as u16 >> 3) & 0x1f;
        let g6 = (g as u16 >> 2) & 0x3f;
        let b5 = (b as u16 >> 3) & 0x1f;
        let bgr565 = (r5 << 11) | (g6 << 5) | b5;

        out.push((bgr565 >> 8) as u8);
        out.push((bgr565 & 0xff) as u8);
    }

    out
}

// ── Frame diffing ──────────────────────────────────────────────────────────

enum DirtyRect {
    Clean,
    Partial { x: u16, y: u16, w: u16, h: u16 },
    Full,
}

/// Compare two frames and compute the dirty bounding box, aligned to device
/// requirements. Returns Clean if identical, Full if >50% dirty, or Partial.
fn compute_dirty_rect(
    prev: &[u8],
    curr: &[u8],
    width: u16,
    height: u16,
    bpp: usize,
    x_align: u16,
    y_align: u16,
) -> DirtyRect {
    if prev.len() != curr.len() {
        return DirtyRect::Full;
    }

    let stride = width as usize * bpp;
    let mut min_x = width as usize;
    let mut max_x: usize = 0;
    let mut min_y = height as usize;
    let mut max_y: usize = 0;

    for y in 0..height as usize {
        let row_start = y * stride;
        let row_end = row_start + stride;
        if row_end > prev.len() {
            break;
        }
        if prev[row_start..row_end] == curr[row_start..row_end] {
            continue;
        }
        // Row is dirty — find the dirty pixel range
        min_y = min_y.min(y);
        max_y = max_y.max(y);

        for x in 0..width as usize {
            let px_start = row_start + x * bpp;
            let px_end = px_start + bpp;
            if prev[px_start..px_end] != curr[px_start..px_end] {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
            }
        }
    }

    if max_x < min_x || max_y < min_y {
        return DirtyRect::Clean;
    }

    // Align the bounding box
    let aligned_x = (min_x as u16 / x_align) * x_align;
    let aligned_y = (min_y as u16 / y_align) * y_align;
    let aligned_right = ((max_x as u16 + x_align) / x_align) * x_align;
    let aligned_bottom = ((max_y as u16 + y_align) / y_align) * y_align;
    let aligned_w = (aligned_right - aligned_x).min(width - aligned_x);
    let aligned_h = (aligned_bottom - aligned_y).min(height - aligned_y);

    // If the dirty region is more than 50% of the screen, do a full blit
    let dirty_pixels = aligned_w as usize * aligned_h as usize;
    let total_pixels = width as usize * height as usize;
    if dirty_pixels > total_pixels / 2 {
        return DirtyRect::Full;
    }

    DirtyRect::Partial {
        x: aligned_x,
        y: aligned_y,
        w: aligned_w,
        h: aligned_h,
    }
}

/// Extract a rectangular region from a framebuffer.
fn extract_region(
    pixels: &[u8],
    width: u16,
    bpp: usize,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
) -> Vec<u8> {
    let stride = width as usize * bpp;
    let region_stride = w as usize * bpp;
    let mut out = Vec::with_capacity(region_stride * h as usize);

    for row in y..(y + h) {
        let src_start = row as usize * stride + x as usize * bpp;
        let src_end = src_start + region_stride;
        if src_end <= pixels.len() {
            out.extend_from_slice(&pixels[src_start..src_end]);
        } else {
            out.extend(std::iter::repeat(0u8).take(region_stride));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_to_bgr565_basic() {
        // Pure red (255, 0, 0) should give r5=31, g6=0, b5=0 → (31<<11) = 0xF800
        // Big-endian: 0xF8, 0x00
        let rgba = [255, 0, 0, 255];
        let result = rgba8_to_bgr565_be(&rgba, 1, 1);
        assert_eq!(result, vec![0xF8, 0x00]);

        // Pure green (0, 255, 0) → g6=63 → (63<<5) = 0x07E0
        let rgba = [0, 255, 0, 255];
        let result = rgba8_to_bgr565_be(&rgba, 1, 1);
        assert_eq!(result, vec![0x07, 0xE0]);

        // Pure blue (0, 0, 255) → b5=31 → 0x001F
        let rgba = [0, 0, 255, 255];
        let result = rgba8_to_bgr565_be(&rgba, 1, 1);
        assert_eq!(result, vec![0x00, 0x1F]);
    }

    #[test]
    fn dirty_rect_clean() {
        let frame = vec![0u8; 100];
        let result = compute_dirty_rect(&frame, &frame, 10, 5, 2, 4, 2);
        assert!(matches!(result, DirtyRect::Clean));
    }

    #[test]
    fn dirty_rect_partial() {
        let prev = vec![0u8; 200]; // 10x10, 2 bpp
        let mut curr = prev.clone();
        // Dirty pixel at (5, 3)
        curr[3 * 20 + 5 * 2] = 0xFF;
        let result = compute_dirty_rect(&prev, &curr, 10, 10, 2, 4, 2);
        match result {
            DirtyRect::Partial { x, y, w, h } => {
                assert_eq!(x % 4, 0); // x aligned
                assert_eq!(y % 2, 0); // y aligned
                assert!(x <= 5);
                assert!(y <= 3);
                assert!(x + w > 5);
                assert!(y + h > 3);
            }
            _ => panic!("Expected partial dirty rect"),
        }
    }
}
