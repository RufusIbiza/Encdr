pub mod gpu;
pub mod gpu_pipeline;
pub mod protocol;

use std::ops::Range;
use std::sync::Arc;

use crate::core::descriptor::{PixelFormat, ScreenDesc, ScreenProtocol};

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

    /// Submit a new frame. Returns the USB transfers to send if the frame
    /// has changed (or it's time for a keyframe), or None if identical.
    pub fn submit(
        &mut self,
        pixels: &[u8],
        input_format: PixelFormat,
        screen_desc: &ScreenDesc,
    ) -> Option<Vec<Vec<u8>>> {
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
        //
        // Row-addressable controllers: send only the band of rows that
        // changed. Their USB bridge is slow (ST7529: ~310 KiB/s), and transfer
        // time scales with bytes sent.
        if let Some(ScreenProtocol::NiSt7529 { .. }) = screen_desc.protocol {
            let rows = if force_full {
                0..self.height
            } else {
                changed_rows(&self.prev_frame, &native_pixels, self.height)?
            };
            let stride = native_pixels.len() / self.height as usize;
            let band = native_pixels[rows.start as usize * stride..rows.end as usize * stride].to_vec();
            self.prev_frame.copy_from_slice(&native_pixels);
            return Some(protocol::frame_transfers(screen_desc, band, rows));
        }

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
                            return Some(vec![protocol::build_partial_blit(
                                screen_desc,
                                x,
                                y,
                                w,
                                h,
                                &region_pixels,
                            )]);
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
        Some(protocol::frame_transfers(
            screen_desc,
            protocol::build_full_blit(screen_desc, &native_pixels),
            0..self.height,
        ))
    }
}

/// The smallest band of whole rows that differs between two frames of
/// `height` equal-stride rows, or None if they're identical.
fn changed_rows(prev: &[u8], curr: &[u8], height: u16) -> Option<Range<u16>> {
    let stride = curr.len() / height as usize;
    let differs = |y: &u16| {
        let row = *y as usize * stride..(*y as usize + 1) * stride;
        prev[row.clone()] != curr[row]
    };
    let first = (0..height).find(differs)?;
    let last = (0..height).rev().find(differs)?;
    Some(first..last + 1)
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
    fn changed_rows_finds_band() {
        let prev = vec![0u8; 4 * 8]; // 8 rows, 4-byte stride
        let mut curr = prev.clone();
        assert_eq!(changed_rows(&prev, &curr, 8), None);
        curr[2 * 4 + 1] = 1; // row 2
        curr[5 * 4 + 3] = 1; // row 5
        assert_eq!(changed_rows(&prev, &curr, 8), Some(2..6));
    }

    /// The last 8 native bytes of rows 10..=12 of an RGBA frame.
    fn band_tail(rgba: &[u8]) -> Vec<u8> {
        let native = to_st7529_gray5(rgba, 4, 255, 64);
        native[13 * 170 - 8..13 * 170].to_vec()
    }

    #[test]
    fn st7529_sends_only_changed_rows() {
        let desc: ScreenDesc = serde_json::from_str(
            r#"{ "name": "left", "interface": "display", "width": 255, "height": 64,
                 "pixel_format": "st7529_gray5", "protocol": { "type": "ni_st7529", "display": 0 } }"#,
        )
        .unwrap();
        let mut sm = ScreenManager::new(&desc, None);
        let mut frame = vec![0u8; 255 * 64 * 4];

        // Frame 1 is a keyframe: every row.
        let first = sm.submit(&frame, PixelFormat::Rgba8888, &desc).unwrap();
        assert_eq!(first[0], vec![0x00, 0x00, 0x03, 0x75, 0x00, 0x3F]);

        // Identical frame: nothing to send.
        assert!(sm.submit(&frame, PixelFormat::Rgba8888, &desc).is_none());

        // Light pixels on rows 10 and 12: LASET 10..=12, 3 rows x 170 bytes.
        for y in [10, 12] {
            let i = (y * 255 + 40) * 4;
            frame[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
        let band = sm.submit(&frame, PixelFormat::Rgba8888, &desc).unwrap();
        assert_eq!(band[0], vec![0x00, 0x00, 0x03, 0x75, 10, 12]);
        // LASET, CASET, then 510 data bytes: RAMWR + 502, continuation of 8.
        assert_eq!(band.len(), 4);
        assert_eq!(band[2][..4], [0x00, 0x01, 0xF7, 0x5C]);
        assert_eq!(band[3], [&[0x01, 0x00, 0x08][..], &band_tail(&frame)].concat());
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
}
