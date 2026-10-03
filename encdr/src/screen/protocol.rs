use std::ops::Range;
use std::time::Duration;

use crate::core::descriptor::{ScreenDesc, ScreenProtocol};

/// One transfer of a screen controller's init sequence.
pub struct InitStep {
    pub data: Vec<u8>,
    /// Pause after this transfer completes.
    pub delay: Duration,
}

/// Transfers that bring a screen controller up after connect. Empty for
/// screens that need no initialisation.
pub fn init_sequence(desc: &ScreenDesc) -> Vec<InitStep> {
    match desc.protocol {
        Some(ScreenProtocol::NiSt7529 { display }) => st7529_init(display),
        None => Vec::new(),
    }
}

/// Split a blit into the USB transfers the screen expects. `blit` holds the
/// native pixels of `rows` (a full frame from [`build_full_blit`] covers every
/// row). Without a protocol the whole blit is one transfer.
pub fn frame_transfers(desc: &ScreenDesc, blit: Vec<u8>, rows: Range<u16>) -> Vec<Vec<u8>> {
    match desc.protocol {
        Some(ScreenProtocol::NiSt7529 { display }) => st7529_frame(display, desc, &blit, rows),
        None => vec![blit],
    }
}

// ── Sitronix ST7529 behind the NI EP8 display bridge (Maschine Mk1) ─────────
//
// Every transfer is `[header, len_hi, len_lo, payload…]` where `len` counts the
// payload. `header = display << 1` marks a payload starting with a controller
// command; `header | 1` marks a pure data continuation. Sequence and framing
// follow shaduzlabs/cabl (MaschineMK1.cpp), verified on hardware.

/// Data bytes per frame transfer: 502 fits one 512-byte high-speed packet
/// together with the 3-byte header and the leading RAMWR command.
const ST7529_CHUNK: usize = 502;

fn st7529_command(display: u8, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(3 + payload.len());
    buf.push(display << 1);
    buf.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

fn st7529_init(display: u8) -> Vec<InitStep> {
    const SETTLE: Duration = Duration::from_millis(20);
    let steps: [(&[u8], Duration); 22] = [
        (&[0x30], Duration::ZERO),                   // EXT_IN: extension set 0
        (&[0xCA, 0x04, 0x0F, 0x00], SETTLE),         // DISCTRL: display control
        (&[0xBB, 0x00], Duration::ZERO),             // COMSCN: COM scan direction
        (&[0xD1], Duration::ZERO),                   // OSC_ON
        (&[0x94], Duration::ZERO),                   // SLEEP_OUT
        (&[0x81, 0x1E, 0x02], SETTLE),               // VOLCTRL: contrast
        (&[0x20, 0x08], SETTLE),                     // PWRCTRL: booster on
        (&[0x20, 0x0B], SETTLE),                     // PWRCTRL: booster + regulator + follower
        (&[0xA6], Duration::ZERO),                   // normal (non-inverted) display
        (&[0x31], Duration::ZERO),                   // EXT_OUT: extension set 1
        (&[0x32, 0x00, 0x00, 0x05], Duration::ZERO), // ANASET: analog circuit
        (&[0x34], Duration::ZERO),                   // SWINT: software initial
        (&[0x30], Duration::ZERO),                   // EXT_IN
        (&[0xBC, 0x00, 0x01, 0x02], Duration::ZERO), // DATSDR: data scan direction
        (&[0x75, 0x00, 0x3F], Duration::ZERO),       // LASET: lines 0..63
        (&[0x15, 0x00, 0x54], Duration::ZERO),       // CASET: columns 0..84 (3 px each)
        (&[0x5C], Duration::ZERO),                   // RAMWR
        (&[0x25], SETTLE),                           // NOP
        (&[0xAF], SETTLE),                           // DISPLAY_ON
        (&[0xBC, 0x02, 0x01, 0x01], Duration::ZERO), // DATSDR: 3-pixel / 2-byte mode
        (&[0xA6], Duration::ZERO),                   // normal display
        (&[0x81, 0x25, 0x02], Duration::ZERO),       // VOLCTRL: final contrast
    ];
    steps
        .iter()
        .map(|&(payload, delay)| InitStep { data: st7529_command(display, payload), delay })
        .collect()
}

fn st7529_frame(display: u8, desc: &ScreenDesc, pixels: &[u8], rows: Range<u16>) -> Vec<Vec<u8>> {
    let last_column = (desc.width.div_ceil(3) - 1) as u8;
    let mut transfers = vec![
        st7529_command(display, &[0x75, rows.start as u8, (rows.end - 1) as u8]), // LASET
        st7529_command(display, &[0x15, 0x00, last_column]),                     // CASET
    ];

    for (i, chunk) in pixels.chunks(ST7529_CHUNK).enumerate() {
        let mut buf = Vec::with_capacity(4 + chunk.len());
        if i == 0 {
            // First chunk carries the RAMWR command ahead of its data.
            buf.push(display << 1);
            buf.extend_from_slice(&(chunk.len() as u16 + 1).to_be_bytes());
            buf.push(0x5C);
        } else {
            buf.push(display << 1 | 1);
            buf.extend_from_slice(&(chunk.len() as u16).to_be_bytes());
        }
        buf.extend_from_slice(chunk);
        transfers.push(buf);
    }
    transfers
}

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
pub(crate) fn parse_byte_list(s: &str) -> Vec<u8> {
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
    fn st7529_frame_chunking() {
        let desc = ScreenDesc {
            name: "right".to_string(),
            interface: "display".to_string(),
            width: 255,
            height: 64,
            pixel_format: crate::core::descriptor::PixelFormat::St7529Gray5,
            full_blit: Default::default(),
            partial_blit: None,
            protocol: Some(ScreenProtocol::NiSt7529 { display: 1 }),
        };
        let frame = vec![0xAA; desc.byte_size()];
        let transfers = frame_transfers(&desc, build_full_blit(&desc, &frame), 0..64);

        assert_eq!(transfers[0], vec![0x02, 0x00, 0x03, 0x75, 0x00, 0x3F]);
        assert_eq!(transfers[1], vec![0x02, 0x00, 0x03, 0x15, 0x00, 0x54]);
        // RAMWR + 502 data bytes, then 20 full continuations and a 338-byte tail.
        assert_eq!(transfers[2][..4], [0x02, 0x01, 0xF7, 0x5C]);
        assert_eq!(transfers[2].len(), 4 + 502);
        assert_eq!(transfers.len(), 2 + 22);
        for t in &transfers[3..23] {
            assert_eq!(t[..3], [0x03, 0x01, 0xF6]);
            assert_eq!(t.len(), 3 + 502);
        }
        assert_eq!(transfers[23][..3], [0x03, 0x01, 0x52]);
        assert_eq!(transfers[23].len(), 3 + 338);
    }

    #[test]
    fn st7529_init_framing() {
        let desc = ScreenDesc {
            name: "left".to_string(),
            interface: "display".to_string(),
            width: 255,
            height: 64,
            pixel_format: crate::core::descriptor::PixelFormat::St7529Gray5,
            full_blit: Default::default(),
            partial_blit: None,
            protocol: Some(ScreenProtocol::NiSt7529 { display: 0 }),
        };
        let steps = init_sequence(&desc);
        assert_eq!(steps.len(), 22);
        assert_eq!(steps[0].data, vec![0x00, 0x00, 0x01, 0x30]);
        assert_eq!(steps[1].data, vec![0x00, 0x00, 0x04, 0xCA, 0x04, 0x0F, 0x00]);
        assert_eq!(steps[1].delay, Duration::from_millis(20));
    }

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
