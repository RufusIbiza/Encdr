use std::io::Cursor;
use once_cell::sync::Lazy;

/// Bundled default screensaver PNG image (480x272).
pub const DEFAULT_SCREENSAVER_PNG: &[u8] = include_bytes!("../../assets/screensaver.png");

/// Decodes raw PNG image bytes into (width, height, rgba_pixels).
pub fn decode_png_to_rgba(png_bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let decoder = png::Decoder::new(Cursor::new(png_bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("PNG decode info failed: {}", e))?;

    let info = reader.info();
    let width = info.width;
    let height = info.height;
    let color_type = info.color_type;

    let mut src_buf = vec![0u8; reader.output_buffer_size()];
    reader
        .next_frame(&mut src_buf)
        .map_err(|e| format!("PNG frame read failed: {}", e))?;

    let rgba = match color_type {
        png::ColorType::Rgba => src_buf,
        png::ColorType::Rgb => {
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for chunk in src_buf.chunks_exact(3) {
                rgba.push(chunk[0]);
                rgba.push(chunk[1]);
                rgba.push(chunk[2]);
                rgba.push(255);
            }
            rgba
        }
        png::ColorType::Grayscale => {
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for &b in &src_buf {
                rgba.push(b);
                rgba.push(b);
                rgba.push(b);
                rgba.push(255);
            }
            rgba
        }
        png::ColorType::GrayscaleAlpha => {
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for chunk in src_buf.chunks_exact(2) {
                rgba.push(chunk[0]);
                rgba.push(chunk[0]);
                rgba.push(chunk[0]);
                rgba.push(chunk[1]);
            }
            rgba
        }
        other => return Err(format!("Unsupported PNG color type: {:?}", other)),
    };

    Ok((width, height, rgba))
}

/// Global lazy-decoded default screensaver RGBA buffer.
pub fn default_screensaver_rgba() -> &'static (u32, u32, Vec<u8>) {
    static SCREENSAVER: Lazy<(u32, u32, Vec<u8>)> = Lazy::new(|| {
        decode_png_to_rgba(DEFAULT_SCREENSAVER_PNG)
            .expect("Failed to decode bundled default screensaver PNG")
    });
    &SCREENSAVER
}

/// Renders a screensaver RGBA frame tailored to the target display dimensions.
///
/// If dimensions match the native 480x272 image, returns the decoded buffer directly.
/// Otherwise, resamples (nearest-neighbor) to fit the target screen geometry.
pub fn render_screensaver_frame(target_w: u16, target_h: u16) -> Vec<u8> {
    let (src_w, src_h, ref src_rgba) = *default_screensaver_rgba();
    let target_w = target_w as usize;
    let target_h = target_h as usize;

    if src_w as usize == target_w && src_h as usize == target_h {
        return src_rgba.clone();
    }

    let mut dst = vec![0u8; target_w * target_h * 4];
    for dy in 0..target_h {
        let sy = (dy * src_h as usize) / target_h;
        for dx in 0..target_w {
            let sx = (dx * src_w as usize) / target_w;
            let src_idx = (sy * src_w as usize + sx) * 4;
            let dst_idx = (dy * target_w + dx) * 4;
            dst[dst_idx..dst_idx + 4].copy_from_slice(&src_rgba[src_idx..src_idx + 4]);
        }
    }
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_screensaver_decodes() {
        let (w, h, rgba) = default_screensaver_rgba();
        assert_eq!(*w, 480);
        assert_eq!(*h, 272);
        assert_eq!(rgba.len(), 480 * 272 * 4);
    }

    #[test]
    fn test_render_screensaver_frame_resize() {
        let frame_320_240 = render_screensaver_frame(320, 240);
        assert_eq!(frame_320_240.len(), 320 * 240 * 4);
    }
}
