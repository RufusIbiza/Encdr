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
        Some(ScreenProtocol::NiMikroMk2) | Some(ScreenProtocol::NiColorRle { .. }) | None => Vec::new(),
    }
}

/// Split a frame produced by [`build_full_blit`] into the USB transfers the
/// screen expects. Without a protocol the whole blit is one transfer.
pub fn frame_transfers(desc: &ScreenDesc, blit: Vec<u8>) -> Vec<Vec<u8>> {
    match desc.protocol {
        Some(ScreenProtocol::NiSt7529 { display }) => st7529_frame(display, desc, &blit),
        Some(ScreenProtocol::NiMikroMk2) => mikro_mk2_frame(desc, &blit),
        Some(ScreenProtocol::NiColorRle { .. }) | None => vec![blit],
    }
}

// ── NI Maschine Mikro Mk2 128x64 Monochrome Display ──────────────────────────
//
// The device expects four 265-byte page transfers for its 128x64 monochrome LCD:
// Report ID 0xE0, followed by an 8-byte LE header:
//   x = 0 (u16 LE), y_page in {0, 2, 4, 6} (u16 LE), width = 128 (u16 LE), height_pages = 2 (u16 LE),
// and 256 page-formatted data bytes (two pages of 128 vertical 8-pixel column bytes).

fn mikro_mk2_frame(desc: &ScreenDesc, blit: &[u8]) -> Vec<Vec<u8>> {
    let width = desc.width as usize;
    let height = desc.height as usize;
    let expected_mono_len = (width * height) / 8;

    let mono = if blit.len() > expected_mono_len {
        &blit[blit.len() - expected_mono_len..]
    } else {
        blit
    };

    let stride = (width + 7) / 8;
    let mut transfers = Vec::with_capacity(4);

    for y_page in [0u16, 2u16, 4u16, 6u16] {
        let mut chunk = Vec::with_capacity(9 + 256);
        chunk.push(0xE0); // Report ID 0xE0
        chunk.extend_from_slice(&0u16.to_le_bytes()); // x = 0
        chunk.extend_from_slice(&y_page.to_le_bytes()); // y_page = 0, 2, 4, 6
        chunk.extend_from_slice(&(width as u16).to_le_bytes()); // width = 128
        chunk.extend_from_slice(&2u16.to_le_bytes()); // height_pages = 2

        for p_rel in 0..2 {
            let y_base = (y_page as usize + p_rel) * 8;
            for x in 0..width {
                let mut col_byte = 0u8;
                for bit in 0..8 {
                    let y = y_base + bit;
                    if y < height {
                        let byte_idx = y * stride + (x / 8);
                        if byte_idx < mono.len() && (mono[byte_idx] & (0x80 >> (x % 8))) != 0 {
                            col_byte |= 1 << bit;
                        }
                    }
                }
                chunk.push(col_byte);
            }
        }
        transfers.push(chunk);
    }

    transfers
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

fn st7529_frame(display: u8, desc: &ScreenDesc, pixels: &[u8]) -> Vec<Vec<u8>> {
    let last_line = (desc.height - 1) as u8;
    let last_column = (desc.width.div_ceil(3) - 1) as u8;
    let mut transfers = vec![
        st7529_command(display, &[0x75, 0x00, last_line]),   // LASET
        st7529_command(display, &[0x15, 0x00, last_column]), // CASET
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
    if let Some(ScreenProtocol::NiColorRle { display }) = desc.protocol {
        return encode_ni_color_rle(display, 0, 0, desc.width, desc.height, native_pixels);
    }

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
    if let Some(ScreenProtocol::NiColorRle { display }) = desc.protocol {
        return encode_ni_color_rle(display, x, y, w, h, region_pixels);
    }

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

// ── NI Color Screen Command Stream (Maschine MK3, Studio, KK Mk2, Traktor) ────
//
// 16-byte header: `[0x84, 0x00, display, 0x60, 0, 0, 0, 0, x_be, y_be, w_be, h_be]`
// followed by 4-byte stream opcodes:
// - `0x00 nn nn nn`: raw transmit `nn` pixel pairs
// - `0x01 nn nn nn [pair]`: repeat 1 pixel pair `nn` times (RLE)
// - `0x02 nn nn nn`: skip `nn` pixel pairs (leaves screen RAM unchanged)
// - `0x03, 0x00, 0x00, 0x00`: blit / swap buffer
// - `0x40, 0x00, 0x00, 0x00`: end of transfer

/// Encodes a rectangle of 16-bit BGR565-BE pixels into the Native Instruments hardware
/// command stream format with Run-Length Encoding (RLE).
pub fn encode_ni_color_rle(
    display: u8,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    bgr565_be_pixels: &[u8],
) -> Vec<u8> {
    let mut f = Vec::with_capacity(16 + bgr565_be_pixels.len() + 8);
    f.extend_from_slice(&[0x84, 0x00, display, 0x60, 0, 0, 0, 0]);
    f.extend_from_slice(&x.to_be_bytes());
    f.extend_from_slice(&y.to_be_bytes());
    f.extend_from_slice(&w.to_be_bytes());
    f.extend_from_slice(&h.to_be_bytes());

    let pair_count = bgr565_be_pixels.len() / 4;
    let count_cmd = |buf: &mut Vec<u8>, cmd: u8, n: usize| {
        buf.push(cmd);
        buf.push((n >> 16) as u8);
        buf.push((n >> 8) as u8);
        buf.push(n as u8);
    };

    let mut i = 0;
    while i < pair_count {
        let p_i = &bgr565_be_pixels[i * 4..(i + 1) * 4];
        let mut run = 1;
        while i + run < pair_count && &bgr565_be_pixels[(i + run) * 4..(i + run + 1) * 4] == p_i {
            run += 1;
        }

        if run >= 4 {
            count_cmd(&mut f, 0x01, run);
            f.extend_from_slice(p_i);
            i += run;
            continue;
        }

        // Scan raw pixels until next run of at least 4 identical pairs
        let start = i;
        while i < pair_count {
            if i + 4 <= pair_count {
                let candidate = &bgr565_be_pixels[i * 4..(i + 1) * 4];
                if (1..4).all(|k| &bgr565_be_pixels[(i + k) * 4..(i + k + 1) * 4] == candidate) {
                    break;
                }
            }
            i += 1;
        }

        let raw_count = i - start;
        if raw_count > 0 {
            count_cmd(&mut f, 0x00, raw_count);
            f.extend_from_slice(&bgr565_be_pixels[start * 4..i * 4]);
        }
    }

    f.extend_from_slice(&[0x03, 0, 0, 0, 0x40, 0, 0, 0]);
    f
}

/// Decoder for NI color screen command streams, useful for testing and verifying streams.
#[derive(Default)]
pub struct NiColorDecoder {
    pub screens: [Vec<u8>; 2],
    pub width: usize,
    pub height: usize,
    buf: Vec<u8>,
}

impl NiColorDecoder {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            screens: [vec![0u8; width * height * 2], vec![0u8; width * height * 2]],
            width,
            height,
            buf: Vec::new(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<usize> {
        self.buf.extend_from_slice(bytes);
        let mut blitted = Vec::new();
        loop {
            match self.parse_transfer() {
                Some(Ok((used, display_blitted))) => {
                    self.buf.drain(..used);
                    if let Some(d) = display_blitted {
                        blitted.push(d);
                    }
                }
                Some(Err(())) => {
                    let next = self.buf[1..]
                        .windows(2)
                        .position(|w| w == [0x84, 0x00])
                        .map_or(self.buf.len(), |i| i + 1);
                    self.buf.drain(..next);
                }
                None => break,
            }
        }
        blitted
    }

    fn parse_transfer(&mut self) -> Option<Result<(usize, Option<usize>), ()>> {
        let b = &self.buf;
        if b.len() < 16 {
            return None;
        }
        if b[0] != 0x84 || b[1] != 0 || b[2] > 1 {
            return Some(Err(()));
        }
        let display = b[2] as usize;
        let u16_at = |i: usize| u16::from_be_bytes([b[i], b[i + 1]]) as usize;
        let (x, y, w, h) = (u16_at(8), u16_at(10), u16_at(12), u16_at(14));
        if w == 0 || x + w > self.width || y + h > self.height {
            return Some(Err(()));
        }

        let mut i = 16;
        let mut pixels: Vec<Option<[u8; 2]>> = Vec::new();
        let mut blit = None;

        loop {
            let cmd = b.get(i..i + 4)?;
            let count = ((cmd[1] as usize) << 16) | ((cmd[2] as usize) << 8) | (cmd[3] as usize);
            i += 4;
            match cmd[0] {
                0x00 => {
                    let data = b.get(i..i + count * 4)?;
                    for chunk in data.chunks_exact(2) {
                        pixels.push(Some([chunk[0], chunk[1]]));
                    }
                    i += count * 4;
                }
                0x01 => {
                    let pair_bytes = b.get(i..i + 4)?;
                    let p0 = [pair_bytes[0], pair_bytes[1]];
                    let p1 = [pair_bytes[2], pair_bytes[3]];
                    for _ in 0..count {
                        pixels.push(Some(p0));
                        pixels.push(Some(p1));
                    }
                    i += 4;
                }
                0x02 => {
                    for _ in 0..count * 2 {
                        pixels.push(None);
                    }
                }
                0x03 => blit = Some(display),
                0x40 => break,
                _ => return Some(Err(())),
            }
        }

        let target = &mut self.screens[display];
        let mut px_idx = 0;
        for row in 0..h {
            for col in 0..w {
                if px_idx >= pixels.len() {
                    break;
                }
                if let Some(p) = pixels[px_idx] {
                    let dst_offset = ((y + row) * self.width + (x + col)) * 2;
                    target[dst_offset] = p[0];
                    target[dst_offset + 1] = p[1];
                }
                px_idx += 1;
            }
        }

        Some(Ok((i, blit)))
    }
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
        let transfers = frame_transfers(&desc, build_full_blit(&desc, &frame));

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
    fn mikro_mk2_frame_chunking() {
        let desc = ScreenDesc {
            name: "screen".to_string(),
            interface: "control".to_string(),
            width: 128,
            height: 64,
            pixel_format: crate::core::descriptor::PixelFormat::Mono,
            full_blit: Default::default(),
            partial_blit: None,
            protocol: Some(ScreenProtocol::NiMikroMk2),
        };

        // 1024 bytes row-major mono (128x64 / 8)
        let mut pixels = vec![0u8; 1024];
        // Turn on top-left pixel (x=0, y=0) -> bit 7 of byte 0 in row-major
        pixels[0] = 0x80;

        let transfers = frame_transfers(&desc, pixels);
        assert_eq!(transfers.len(), 4);

        // Every transfer must be 265 bytes (1 report ID + 8 header + 256 data)
        for (i, t) in transfers.iter().enumerate() {
            assert_eq!(t.len(), 265, "transfer {i} length mismatch");
            assert_eq!(t[0], 0xE0, "transfer {i} report ID mismatch");
            assert_eq!(&t[1..3], &[0x00, 0x00], "transfer {i} x mismatch");
            assert_eq!(&t[5..7], &[0x80, 0x00], "transfer {i} width mismatch");
            assert_eq!(&t[7..9], &[0x02, 0x00], "transfer {i} height_pages mismatch");
        }

        // Check y_page values
        assert_eq!(&transfers[0][3..5], &[0x00, 0x00]); // y_page = 0
        assert_eq!(&transfers[1][3..5], &[0x02, 0x00]); // y_page = 2
        assert_eq!(&transfers[2][3..5], &[0x04, 0x00]); // y_page = 4
        assert_eq!(&transfers[3][3..5], &[0x06, 0x00]); // y_page = 6

        // In transfer 0, top-left pixel (x=0, y=0) should map to bit 0 of the first data byte (offset 9)
        assert_eq!(transfers[0][9], 0x01);
    }

    #[test]
    fn ni_color_rle_compression_and_decode() {
        let w = 8u16;
        let h = 8u16;
        // 64 pixels * 2 bytes = 128 bytes = 32 pixel pairs
        // Make 10 pairs of red, then 22 pairs of blue
        let red_pair = [0xF8, 0x00, 0xF8, 0x00];
        let blue_pair = [0x00, 0x1F, 0x00, 0x1F];
        let mut pixels = Vec::new();
        for _ in 0..10 {
            pixels.extend_from_slice(&red_pair);
        }
        for _ in 0..22 {
            pixels.extend_from_slice(&blue_pair);
        }

        let encoded = encode_ni_color_rle(0, 0, 0, w, h, &pixels);

        // Header: 16 bytes
        // Run 1: 4 bytes opcode + 4 bytes pair = 8 bytes
        // Run 2: 4 bytes opcode + 4 bytes pair = 8 bytes
        // Footer: 4 bytes (blit) + 4 bytes (end) = 8 bytes
        // Total expected size: 16 + 8 + 8 + 8 = 40 bytes (vs 128 raw)
        assert_eq!(encoded.len(), 40);

        let mut decoder = NiColorDecoder::new(w as usize, h as usize);
        let blits = decoder.feed(&encoded);
        assert_eq!(blits, vec![0]);
        assert_eq!(decoder.screens[0], pixels);
    }

    #[test]
    fn ni_color_rle_raw_and_partial_blit() {
        let full_w = 16usize;
        let full_h = 16usize;
        let mut decoder = NiColorDecoder::new(full_w, full_h);

        // Partial blit on display 1: x=4, y=2, w=4, h=2 (8 pixels = 4 pairs, all unique)
        let sub_w = 4u16;
        let sub_h = 2u16;
        let mut sub_pixels = Vec::new();
        for i in 0..8u8 {
            sub_pixels.push(i);
            sub_pixels.push(i + 10);
        }

        let encoded = encode_ni_color_rle(1, 4, 2, sub_w, sub_h, &sub_pixels);
        let blits = decoder.feed(&encoded);
        assert_eq!(blits, vec![1]);

        // Verify the sub-rectangle in screen 1 was updated
        for row in 0..sub_h as usize {
            for col in 0..sub_w as usize {
                let src_idx = (row * (sub_w as usize) + col) * 2;
                let dst_idx = ((2 + row) * full_w + (4 + col)) * 2;
                assert_eq!(
                    &decoder.screens[1][dst_idx..dst_idx + 2],
                    &sub_pixels[src_idx..src_idx + 2],
                    "Mismatch at sub rect ({col}, {row})"
                );
            }
        }

        // Verify outside pixels remain 0
        assert_eq!(decoder.screens[1][0..2], [0, 0]);
    }
}
