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

        // Step 2: Determine if we need a keyframe (full blit on frame 1 and every ~60 frames / ~2s at 30fps)
        let force_full = self.frame_count == 1 || self.frame_count % 60 == 0;

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

                    match dirty {
                        DirtyRect::Clean => return None,
                        DirtyRect::Partial { x, y, w, h } => {
                            self.prev_frame.copy_from_slice(&native_pixels);
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
        if self.prev_frame == native_pixels && !force_full {
            return None;
        }
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
        (PixelFormat::Rgba8888, PixelFormat::Mono) => {
            rgba8_to_mono(pixels, width as usize, height as usize)
        }
        (PixelFormat::Rgb888, PixelFormat::Mono) => {
            rgb8_to_mono(pixels, width as usize, height as usize)
        }
        (PixelFormat::Rgba8888, PixelFormat::St7529Gray5) => {
            to_st7529_gray5(pixels, 4, width as usize, height as usize)
        }
        (PixelFormat::Rgb888, PixelFormat::St7529Gray5) => {
            to_st7529_gray5(pixels, 3, width as usize, height as usize)
        }
        (PixelFormat::Mono, PixelFormat::St7529Gray5) => {
            mono_to_st7529_gray5(pixels, width as usize, height as usize)
        }
        (PixelFormat::St7529Gray5, PixelFormat::Mono) => {
            st7529_gray5_to_mono(pixels, width as usize, height as usize)
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

/// Convert RGBA8888 to 1bpp monochrome bitmap (MSB-first row-major).
pub(crate) fn rgba8_to_mono(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let stride = (width + 7) / 8;
    let mut out = vec![0u8; stride * height];

    for y in 0..height {
        for x in 0..width {
            let offset = (y * width + x) * 4;
            if offset + 2 < rgba.len() {
                let r = rgba[offset] as u32;
                let g = rgba[offset + 1] as u32;
                let b = rgba[offset + 2] as u32;
                let lum = (r * 299 + g * 587 + b * 114) / 1000;
                if lum > 128 {
                    let byte_idx = y * stride + (x / 8);
                    let bit_mask = 0x80 >> (x % 8);
                    out[byte_idx] |= bit_mask;
                }
            }
        }
    }

    out
}

/// Convert RGB888 to 1bpp monochrome bitmap (MSB-first row-major).
pub(crate) fn rgb8_to_mono(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    let stride = (width + 7) / 8;
    let mut out = vec![0u8; stride * height];

    for y in 0..height {
        for x in 0..width {
            let offset = (y * width + x) * 3;
            if offset + 2 < rgb.len() {
                let r = rgb[offset] as u32;
                let g = rgb[offset + 1] as u32;
                let b = rgb[offset + 2] as u32;
                let lum = (r * 299 + g * 587 + b * 114) / 1000;
                if lum > 128 {
                    let byte_idx = y * stride + (x / 8);
                    let bit_mask = 0x80 >> (x % 8);
                    out[byte_idx] |= bit_mask;
                }
            }
        }
    }

    out
}

/// Convert RGB(A) to ST7529 5-bit grayscale, 3 pixels per 2 bytes:
/// `[p0:5 p1_hi:3] [p1_lo:2 _:1 p2:5]`. Levels are stored inverted (0 = lit,
/// 31 = black), so unused trailing pixels in the last group are black.
pub(crate) fn to_st7529_gray5(src: &[u8], channels: usize, width: usize, height: usize) -> Vec<u8> {
    let groups = width.div_ceil(3);
    let mut out = vec![0u8; groups * 2 * height];

    for y in 0..height {
        for g in 0..groups {
            let mut px = [0x1Fu8; 3];
            for (i, level) in px.iter_mut().enumerate() {
                let x = g * 3 + i;
                let offset = (y * width + x) * channels;
                if x < width && offset + 2 < src.len() {
                    let lum = (src[offset] as u32 * 299
                        + src[offset + 1] as u32 * 587
                        + src[offset + 2] as u32 * 114)
                        / 1000;
                    *level = 0x1F - ((lum * 31 + 127) / 255) as u8;
                }
            }
            let i = (y * groups + g) * 2;
            out[i] = px[0] << 3 | px[1] >> 2;
            out[i + 1] = (px[1] & 0x03) << 6 | px[2];
        }
    }

    out
}

/// Convert 1bpp monochrome bitmap (MSB-first row-major) to ST7529 5-bit grayscale
/// (3 pixels packed into 2 bytes, 0 = lit, 31 = black).
///
/// Handles both native 255-wide and standard 256-wide monochrome framebuffers
/// (such as those from Maschine Mk2 or X1 Mk3) by matching the scanline stride.
pub(crate) fn mono_to_st7529_gray5(src: &[u8], width: usize, height: usize) -> Vec<u8> {
    let groups = width.div_ceil(3);
    let mut out = vec![0u8; groups * 2 * height];
    let src_stride = if height > 0 && src.len() >= height {
        src.len() / height
    } else {
        (width + 7) / 8
    };

    for y in 0..height {
        let row_offset = y * src_stride;
        for g in 0..groups {
            let mut px = [0x1Fu8; 3];
            for (i, level) in px.iter_mut().enumerate() {
                let x = g * 3 + i;
                if x < width {
                    let byte_idx = row_offset + (x / 8);
                    if byte_idx < src.len() {
                        let bit_mask = 0x80 >> (x % 8);
                        if (src[byte_idx] & bit_mask) != 0 {
                            *level = 0x00; // Lit (white)
                        } else {
                            *level = 0x1F; // Unlit (black)
                        }
                    }
                }
            }
            let idx = (y * groups + g) * 2;
            out[idx] = px[0] << 3 | px[1] >> 2;
            out[idx + 1] = (px[1] & 0x03) << 6 | px[2];
        }
    }

    out
}

/// Convert ST7529 5-bit grayscale to 1bpp monochrome bitmap (MSB-first row-major).
///
/// Thresholds at mid-level (16): pixels with inverted level < 16 are treated as lit.
pub(crate) fn st7529_gray5_to_mono(src: &[u8], width: usize, height: usize) -> Vec<u8> {
    let stride = (width + 7) / 8;
    let mut out = vec![0u8; stride * height];
    let groups = width.div_ceil(3);

    for y in 0..height {
        let row_offset = y * stride;
        for g in 0..groups {
            let idx = (y * groups + g) * 2;
            if idx + 1 >= src.len() {
                break;
            }
            let b0 = src[idx];
            let b1 = src[idx + 1];
            let px = [
                b0 >> 3,
                ((b0 & 0x07) << 2) | (b1 >> 6),
                b1 & 0x1F,
            ];
            for (i, &level) in px.iter().enumerate() {
                let x = g * 3 + i;
                if x < width && level < 16 {
                    let byte_idx = row_offset + (x / 8);
                    let bit_mask = 0x80 >> (x % 8);
                    out[byte_idx] |= bit_mask;
                }
            }
        }
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

/// Split a horizontally combined double-width image buffer into left and right screen buffers.
///
/// * `pixels`: Contiguous row-major pixel buffer of dimension `(left_width + right_width) * height * bpp`.
/// * `left_width`: Width of the left screen in pixels.
/// * `right_width`: Width of the right screen in pixels.
/// * `height`: Height of the screens in pixels.
/// * `bpp`: Bytes per pixel (e.g. 4 for RGBA8888, 2 for BGR565/RGB565).
///
/// Returns `Some((left_pixels, right_pixels))` or `None` if `pixels.len() != (left_width + right_width) * height * bpp`.
pub fn split_horizontal(
    pixels: &[u8],
    left_width: usize,
    right_width: usize,
    height: usize,
    bpp: usize,
) -> Option<(Vec<u8>, Vec<u8>)> {
    if bpp == 0 || height == 0 || left_width == 0 || right_width == 0 {
        return None;
    }
    let row_stride = (left_width + right_width) * bpp;
    let expected_len = row_stride * height;
    if pixels.len() != expected_len {
        return None;
    }

    let left_row_bytes = left_width * bpp;
    let right_row_bytes = right_width * bpp;

    let mut left = Vec::with_capacity(left_row_bytes * height);
    let mut right = Vec::with_capacity(right_row_bytes * height);

    for row in 0..height {
        let row_start = row * row_stride;
        let left_end = row_start + left_row_bytes;
        let right_end = left_end + right_row_bytes;

        left.extend_from_slice(&pixels[row_start..left_end]);
        right.extend_from_slice(&pixels[left_end..right_end]);
    }

    Some((left, right))
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

    #[test]
    fn rgba_to_mono_basic() {
        // White pixel (255, 255, 255) -> bit 1 at MSB (0x80)
        let rgba = [255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255,
                    0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255];
        let result = rgba8_to_mono(&rgba, 8, 1);
        assert_eq!(result, vec![0x80]);
    }

    #[test]
    fn rgba_to_st7529_gray5_packing() {
        // 4 pixels: white, black, mid-gray, white. The 4th starts a second
        // 3-pixel group whose two missing pixels pad as black (0x1F).
        let rgba = [
            255, 255, 255, 255, 0, 0, 0, 255, 128, 128, 128, 255, 255, 255, 255, 255,
        ];
        let out = to_st7529_gray5(&rgba, 4, 4, 1);
        // Inverted levels: white 0, black 31, gray 31 - 16 = 15.
        assert_eq!(out, vec![0 << 3 | 31 >> 2, (31 & 3) << 6 | 15, 0 << 3 | 31 >> 2, (31 & 3) << 6 | 31]);
    }

    #[test]
    fn mono_to_st7529_gray5_packing_and_round_trip() {
        // 4 pixels: lit, unlit, unlit, lit (MSB 0b1001_0000 = 0x90).
        let mono = [0x90];
        let gray5 = mono_to_st7529_gray5(&mono, 4, 1);
        // Inverted: lit 0, unlit 31.
        // Group 0: p0=0, p1=31, p2=31. Group 1: p0=0, p1=31(pad), p2=31(pad).
        assert_eq!(
            gray5,
            vec![
                0 << 3 | 31 >> 2,
                (31 & 3) << 6 | 31,
                0 << 3 | 31 >> 2,
                (31 & 3) << 6 | 31,
            ]
        );
        // Round trip back to mono
        let back = st7529_gray5_to_mono(&gray5, 4, 1);
        assert_eq!(back, vec![0x90]);
    }

    #[test]
    fn mono_to_st7529_mk2_stride_compatibility() {
        // Simulate a 256x64 Mk2 framebuffer (32 bytes per row * 64 rows = 2048 bytes).
        let mut mk2_frame = vec![0u8; 32 * 64];
        // Turn on pixel (0, 0) and pixel (0, 1) in row 0 and row 1.
        mk2_frame[0] = 0x80;
        mk2_frame[32] = 0x80;

        let st7529_frame = mono_to_st7529_gray5(&mk2_frame, 255, 64);
        // Total bytes should match 85 groups * 2 bytes * 64 rows = 10,880.
        assert_eq!(st7529_frame.len(), 85 * 2 * 64);

        // Row 0 first triad: p0 lit (0), p1 unlit (31), p2 unlit (31)
        assert_eq!(st7529_frame[0], 0 << 3 | 31 >> 2);
        assert_eq!(st7529_frame[1], (31 & 3) << 6 | 31);

        // Row 1 first triad at offset 170: p0 lit (0), p1 unlit (31), p2 unlit (31)
        assert_eq!(st7529_frame[170], 0 << 3 | 31 >> 2);
        assert_eq!(st7529_frame[171], (31 & 3) << 6 | 31);
    }

    #[test]
    fn test_screen_manager_lifecycle() {
        let desc = crate::core::descriptor::ScreenDesc {
            name: "test".to_string(),
            interface: "screen".to_string(),
            width: 10,
            height: 10,
            pixel_format: PixelFormat::Bgr565Be,
            full_blit: crate::core::descriptor::ScreenBlitDesc {
                header: "0x01".to_string(),
                footer: "0x02".to_string(),
            },
            partial_blit: Some(crate::core::descriptor::PartialBlitDesc {
                supported: true,
                x_align: 2,
                y_align: 2,
                header_template: "0x03".to_string(),
                footer: "0x04".to_string(),
            }),
            protocol: None,
        };

        let mut sm = ScreenManager::new(&desc, None);
        let frame_black = vec![0u8; 10 * 10 * 2];
        let mut frame_red = vec![0xF8, 0x00].repeat(10 * 10);

        // Frame 1: forced full
        let res1 = sm.submit(&frame_black, PixelFormat::Bgr565Be, &desc);
        assert!(res1.is_some(), "Frame 1 must always be submitted");

        // Frame 2: identical black -> Clean -> None
        let res2 = sm.submit(&frame_black, PixelFormat::Bgr565Be, &desc);
        assert!(res2.is_none(), "Identical frame should return None");

        // Frame 3: 100% changed to red -> DirtyRect::Full -> must return Some
        let res3 = sm.submit(&frame_red, PixelFormat::Bgr565Be, &desc);
        assert!(res3.is_some(), "Full screen change on non-keyframe must not be dropped");

        // Frame 4: 1 pixel changed -> Partial -> must return Some
        frame_red[0] = 0x00;
        let res4 = sm.submit(&frame_red, PixelFormat::Bgr565Be, &desc);
        assert!(res4.is_some(), "Partial change should return Some");
    }

    #[test]
    fn test_split_horizontal_success() {
        // 2x2 left screen, 3x2 right screen, 2 bpp (e.g. RGB565)
        // Combined width: 5, height: 2, row_stride: 10 bytes, total 20 bytes.
        // Row 0: L0 L1 R0 R1 R2
        // Row 1: L2 L3 R3 R4 R5
        let pixels: Vec<u8> = vec![
            // Row 0:
            1, 1, 2, 2, 10, 10, 20, 20, 30, 30,
            // Row 1:
            3, 3, 4, 4, 40, 40, 50, 50, 60, 60,
        ];

        let (left, right) = split_horizontal(&pixels, 2, 3, 2, 2).expect("split should succeed");

        assert_eq!(left, vec![
            1, 1, 2, 2,
            3, 3, 4, 4,
        ]);

        assert_eq!(right, vec![
            10, 10, 20, 20, 30, 30,
            40, 40, 50, 50, 60, 60,
        ]);
    }

    #[test]
    fn test_split_horizontal_invalid_length() {
        let pixels = vec![0u8; 19]; // expected 20
        assert!(split_horizontal(&pixels, 2, 3, 2, 2).is_none());
    }

    #[test]
    fn test_split_horizontal_zero_dimensions() {
        assert!(split_horizontal(&[], 0, 3, 2, 2).is_none());
        assert!(split_horizontal(&[], 2, 0, 2, 2).is_none());
        assert!(split_horizontal(&[], 2, 3, 0, 2).is_none());
        assert!(split_horizontal(&[], 2, 3, 2, 0).is_none());
    }
}
