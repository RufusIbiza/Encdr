use crate::core::descriptor::ScreenDesc;

/// Build a full-frame blit transfer buffer: header + pixel data + footer.
pub fn build_full_blit(desc: &ScreenDesc, native_pixels: &[u8]) -> Vec<u8> {
    let header = parse_byte_list(&desc.full_blit.header);
    let footer = parse_byte_list(&desc.full_blit.footer);

    let mut buf = Vec::with_capacity(header.len() + native_pixels.len() + footer.len());
    buf.extend_from_slice(&header);
    buf.extend_from_slice(native_pixels);
    buf.extend_from_slice(&footer);
    buf
}

/// Build a partial-blit transfer buffer for a sub-rectangle.
pub fn build_partial_blit(
    desc: &ScreenDesc,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    region_pixels: &[u8],
) -> Vec<u8> {
    let partial = desc.partial_blit.as_ref().expect("partial_blit not supported");

    // Compute template substitution values
    let num_pixels = region_pixels.len() / desc.pixel_format.bytes_per_pixel();
    let px_half = (num_pixels / 2) as u16;

    let header = expand_template(
        &partial.header_template,
        x,
        y,
        w,
        h,
        px_half,
    );
    let footer = parse_byte_list(&partial.footer);

    let mut buf = Vec::with_capacity(header.len() + region_pixels.len() + footer.len());
    buf.extend_from_slice(&header);
    buf.extend_from_slice(region_pixels);
    buf.extend_from_slice(&footer);
    buf
}

/// Parse a comma-separated hex byte list like "0x84,0x00,0x03".
fn parse_byte_list(s: &str) -> Vec<u8> {
    s.split(',')
        .filter_map(|token| {
            let token = token.trim();
            if token.is_empty() {
                return None;
            }
            if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
                u8::from_str_radix(hex, 16).ok()
            } else {
                token.parse::<u8>().ok()
            }
        })
        .collect()
}

/// Expand a header template, replacing coordinate placeholders.
fn expand_template(
    template: &str,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    px_half: u16,
) -> Vec<u8> {
    let mut result = Vec::new();

    for token in template.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        match token {
            "{x_hi}" => result.push((x >> 8) as u8),
            "{x_lo}" => result.push((x & 0xff) as u8),
            "{y_hi}" => result.push((y >> 8) as u8),
            "{y_lo}" => result.push((y & 0xff) as u8),
            "{w_hi}" => result.push((w >> 8) as u8),
            "{w_lo}" => result.push((w & 0xff) as u8),
            "{h_hi}" => result.push((h >> 8) as u8),
            "{h_lo}" => result.push((h & 0xff) as u8),
            "{px_half_hi}" => result.push((px_half >> 8) as u8),
            "{px_half_lo}" => result.push((px_half & 0xff) as u8),
            other => {
                // Parse as hex byte
                if let Some(hex) =
                    other.strip_prefix("0x").or_else(|| other.strip_prefix("0X"))
                {
                    if let Ok(b) = u8::from_str_radix(hex, 16) {
                        result.push(b);
                    }
                } else if let Ok(b) = other.parse::<u8>() {
                    result.push(b);
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_list() {
        let bytes = parse_byte_list("0x84,0x00,0x03,0xFF");
        assert_eq!(bytes, vec![0x84, 0x00, 0x03, 0xFF]);
    }

    #[test]
    fn template_expansion() {
        let template = "0x84,0x00,{x_hi},{x_lo},{y_hi},{y_lo},{w_hi},{w_lo},{h_hi},{h_lo}";
        let result = expand_template(template, 100, 50, 200, 100, 0);
        assert_eq!(
            result,
            vec![
                0x84, 0x00,
                0x00, 100,   // x=100
                0x00, 50,    // y=50
                0x00, 200,   // w=200 (0x00, 0xC8)
                0x00, 100,   // h=100
            ]
        );
    }
}
