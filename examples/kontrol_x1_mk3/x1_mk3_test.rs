//! NI Traktor Kontrol X1 MK3 interactive hardware test.
//!
//! Demonstrates:
//! - Real-time input handling for 21 buttons, 4 encoders, and 8 analog knobs.
//! - Dynamic LED feedback (single-color buttons, RGB hotcues, and RGB underglow).
//! - Concurrent rendering to all 5 monochrome OLED screens (128x64 pixels each).
//!
//! Run with:
//!   cargo run -p encdr-examples --bin x1_mk3_test

use std::time::{Duration, Instant};

use encdr::{Encdr, EncdrConfig, Event, LedValue, PixelFormat};

// ── 5x7 Minimal Monospace Bitmap Font ───────────────────────────────────────

const FONT_5X7: [&[u8; 5]; 96] = [
    &[0x00, 0x00, 0x00, 0x00, 0x00], // Space
    &[0x00, 0x00, 0x5F, 0x00, 0x00], // !
    &[0x00, 0x07, 0x00, 0x07, 0x00], // "
    &[0x14, 0x7F, 0x14, 0x7F, 0x14], // #
    &[0x24, 0x2A, 0x7F, 0x2A, 0x12], // $
    &[0x23, 0x13, 0x08, 0x64, 0x62], // %
    &[0x36, 0x49, 0x55, 0x22, 0x50], // &
    &[0x00, 0x05, 0x03, 0x00, 0x00], // '
    &[0x00, 0x1C, 0x22, 0x41, 0x00], // (
    &[0x00, 0x41, 0x22, 0x1C, 0x00], // )
    &[0x14, 0x08, 0x3E, 0x08, 0x14], // *
    &[0x08, 0x08, 0x3E, 0x08, 0x08], // +
    &[0x00, 0x50, 0x30, 0x00, 0x00], // ,
    &[0x08, 0x08, 0x08, 0x08, 0x08], // -
    &[0x00, 0x60, 0x60, 0x00, 0x00], // .
    &[0x20, 0x10, 0x08, 0x04, 0x02], // /
    &[0x3E, 0x51, 0x49, 0x45, 0x3E], // 0
    &[0x00, 0x42, 0x7F, 0x40, 0x00], // 1
    &[0x42, 0x61, 0x51, 0x49, 0x46], // 2
    &[0x21, 0x41, 0x45, 0x4B, 0x31], // 3
    &[0x18, 0x14, 0x12, 0x7F, 0x10], // 4
    &[0x27, 0x45, 0x45, 0x45, 0x39], // 5
    &[0x3C, 0x4A, 0x49, 0x49, 0x30], // 6
    &[0x01, 0x71, 0x09, 0x05, 0x03], // 7
    &[0x36, 0x49, 0x49, 0x49, 0x36], // 8
    &[0x06, 0x49, 0x49, 0x29, 0x1E], // 9
    &[0x00, 0x36, 0x36, 0x00, 0x00], // :
    &[0x00, 0x56, 0x36, 0x00, 0x00], // ;
    &[0x08, 0x14, 0x22, 0x41, 0x00], // <
    &[0x14, 0x14, 0x14, 0x14, 0x14], // =
    &[0x00, 0x41, 0x22, 0x14, 0x08], // >
    &[0x02, 0x01, 0x51, 0x09, 0x06], // ?
    &[0x32, 0x49, 0x79, 0x41, 0x3E], // @
    &[0x7E, 0x11, 0x11, 0x11, 0x7E], // A
    &[0x7F, 0x49, 0x49, 0x49, 0x36], // B
    &[0x3E, 0x41, 0x41, 0x41, 0x22], // C
    &[0x7F, 0x41, 0x41, 0x22, 0x1C], // D
    &[0x7F, 0x49, 0x49, 0x49, 0x41], // E
    &[0x7F, 0x09, 0x09, 0x09, 0x01], // F
    &[0x3E, 0x41, 0x49, 0x49, 0x7A], // G
    &[0x7F, 0x08, 0x08, 0x08, 0x7F], // H
    &[0x00, 0x41, 0x7F, 0x41, 0x00], // I
    &[0x20, 0x40, 0x41, 0x3F, 0x01], // J
    &[0x7F, 0x08, 0x14, 0x22, 0x41], // K
    &[0x7F, 0x40, 0x40, 0x40, 0x40], // L
    &[0x7F, 0x02, 0x0C, 0x02, 0x7F], // M
    &[0x7F, 0x04, 0x08, 0x10, 0x7F], // N
    &[0x3E, 0x41, 0x41, 0x41, 0x3E], // O
    &[0x7F, 0x09, 0x09, 0x09, 0x06], // P
    &[0x3E, 0x41, 0x51, 0x21, 0x5E], // Q
    &[0x7F, 0x09, 0x19, 0x29, 0x46], // R
    &[0x46, 0x49, 0x49, 0x49, 0x31], // S
    &[0x01, 0x01, 0x7F, 0x01, 0x01], // T
    &[0x3F, 0x40, 0x40, 0x40, 0x3F], // U
    &[0x1F, 0x20, 0x40, 0x20, 0x1F], // V
    &[0x3F, 0x40, 0x38, 0x40, 0x3F], // W
    &[0x63, 0x14, 0x08, 0x14, 0x63], // X
    &[0x07, 0x08, 0x70, 0x08, 0x07], // Y
    &[0x61, 0x51, 0x49, 0x45, 0x43], // Z
    &[0x00, 0x7F, 0x41, 0x41, 0x00], // [
    &[0x02, 0x04, 0x08, 0x10, 0x20], // \
    &[0x00, 0x41, 0x41, 0x7F, 0x00], // ]
    &[0x04, 0x02, 0x01, 0x02, 0x04], // ^
    &[0x40, 0x40, 0x40, 0x40, 0x40], // _
    &[0x00, 0x01, 0x02, 0x04, 0x00], // `
    &[0x20, 0x54, 0x54, 0x54, 0x78], // a
    &[0x7F, 0x48, 0x44, 0x44, 0x38], // b
    &[0x38, 0x44, 0x44, 0x44, 0x20], // c
    &[0x38, 0x44, 0x44, 0x48, 0x7F], // d
    &[0x38, 0x54, 0x54, 0x54, 0x18], // e
    &[0x08, 0x7E, 0x09, 0x01, 0x02], // f
    &[0x0C, 0x52, 0x52, 0x52, 0x3E], // g
    &[0x7F, 0x08, 0x04, 0x04, 0x78], // h
    &[0x00, 0x44, 0x7D, 0x40, 0x00], // i
    &[0x20, 0x40, 0x44, 0x3D, 0x00], // j
    &[0x7F, 0x10, 0x28, 0x44, 0x00], // k
    &[0x00, 0x41, 0x7F, 0x40, 0x00], // l
    &[0x7C, 0x04, 0x18, 0x04, 0x78], // m
    &[0x7C, 0x08, 0x04, 0x04, 0x78], // n
    &[0x38, 0x44, 0x44, 0x44, 0x38], // o
    &[0x7C, 0x14, 0x14, 0x14, 0x08], // p
    &[0x08, 0x14, 0x14, 0x18, 0x7C], // q
    &[0x7C, 0x08, 0x04, 0x04, 0x08], // r
    &[0x48, 0x54, 0x54, 0x54, 0x20], // s
    &[0x04, 0x3F, 0x44, 0x40, 0x20], // t
    &[0x3C, 0x40, 0x40, 0x20, 0x7C], // u
    &[0x1C, 0x20, 0x40, 0x20, 0x1C], // v
    &[0x3C, 0x40, 0x30, 0x40, 0x3C], // w
    &[0x44, 0x28, 0x10, 0x28, 0x44], // x
    &[0x0C, 0x50, 0x50, 0x50, 0x3C], // y
    &[0x44, 0x64, 0x54, 0x4C, 0x44], // z
    &[0x00, 0x08, 0x36, 0x41, 0x00], // {
    &[0x00, 0x00, 0x7F, 0x00, 0x00], // |
    &[0x00, 0x41, 0x36, 0x08, 0x00], // }
    &[0x08, 0x08, 0x2A, 0x1C, 0x08], // ~
    &[0x7F, 0x7F, 0x7F, 0x7F, 0x7F], // (del / block)
];

// ── Screen Bitmap Canvas (128x64 monochrome) ──────────────────────────────

struct Canvas {
    pixels: [u8; 128 * 64],
}

impl Canvas {
    fn new() -> Self {
        Self {
            pixels: [0u8; 128 * 64],
        }
    }

    fn clear(&mut self) {
        self.pixels.fill(0);
    }

    fn set_pixel(&mut self, x: i32, y: i32, on: bool) {
        if x >= 0 && x < 128 && y >= 0 && y < 64 {
            self.pixels[(y as usize) * 128 + (x as usize)] = if on { 255 } else { 0 };
        }
    }

    fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, on: bool) {
        for dy in 0..h {
            for dx in 0..w {
                if dx == 0 || dx == w - 1 || dy == 0 || dy == h - 1 {
                    self.set_pixel(x + dx, y + dy, on);
                }
            }
        }
    }

    fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, on: bool) {
        for dy in 0..h {
            for dx in 0..w {
                self.set_pixel(x + dx, y + dy, on);
            }
        }
    }

    fn draw_text(&mut self, x: i32, y: i32, text: &str, scale: i32) {
        let mut cur_x = x;
        for c in text.chars() {
            let idx = (c as usize).saturating_sub(32);
            if idx < FONT_5X7.len() {
                let glyph = FONT_5X7[idx];
                for (col_idx, &col) in glyph.iter().enumerate() {
                    for row_idx in 0..7 {
                        if (col & (1 << row_idx)) != 0 {
                            for sx in 0..scale {
                                for sy in 0..scale {
                                    self.set_pixel(
                                        cur_x + (col_idx as i32) * scale + sx,
                                        y + (row_idx as i32) * scale + sy,
                                        true,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            cur_x += (5 + 1) * scale;
        }
    }

    /// Converts internal buffer into packed 1bpp monochrome format (1,024 bytes).
    fn to_mono_bytes(&self) -> Vec<u8> {
        let stride = 128 / 8; // 16 bytes per row
        let mut out = vec![0u8; stride * 64];
        for y in 0..64 {
            for x in 0..128 {
                if self.pixels[y * 128 + x] > 128 {
                    let byte_idx = y * stride + (x / 8);
                    let bit_mask = 0x80 >> (x % 8);
                    out[byte_idx] |= bit_mask;
                }
            }
        }
        out
    }
}

// ── Controller State ────────────────────────────────────────────────────────

const LOOP_SIZES: &[&str] = &["'32", "'16", "'8", "'4", "'2", "1", "2", "4", "8", "16", "32"];
const MODES: &[&str] = &["TRACK", "FX UNIT", "MIXER"];

struct X1State {
    mode_index: usize,
    left_loop_index: usize,
    right_loop_index: usize,
    knobs: [f32; 8],
    shift_active: bool,
    dirty: bool,
}

impl X1State {
    fn new() -> Self {
        Self {
            mode_index: 0,
            left_loop_index: 5,  // default "1" beat
            right_loop_index: 5, // default "1" beat
            knobs: [0.5; 8],
            shift_active: false,
            dirty: true,
        }
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".parse().unwrap()),
        )
        .init();

    println!("======================================================");
    println!("     NI Traktor Kontrol X1 MK3 Hardware Demo          ");
    println!("======================================================");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Device scan failed");

    let device_id = match ids.into_iter().find(|&id| {
        encdr
            .device_descriptor(id)
            .map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x2200)
            .unwrap_or(false)
    }) {
        Some(id) => id,
        None => {
            eprintln!("Error: NI Traktor Kontrol X1 MK3 (17cc:2200) not found.");
            eprintln!("Ensure the USB cable is plugged in and udev permissions allow access.");
            return;
        }
    };

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected to: {}", desc.name);
    println!("Detected {} screens:", desc.screens.len());
    for s in &desc.screens {
        println!(" - {} ({}x{} {:?})", s.name, s.width, s.height, s.pixel_format);
    }

    // Set initial static LED illumination:
    // Hotcues: vibrant RGB defaults
    encdr.set_led(device_id, "left_hotcue_1", LedValue::Rgb { r: 0, g: 255, b: 128 });
    encdr.set_led(device_id, "left_hotcue_2", LedValue::Rgb { r: 0, g: 180, b: 255 });
    encdr.set_led(device_id, "left_hotcue_3", LedValue::Rgb { r: 255, g: 128, b: 0 });
    encdr.set_led(device_id, "left_hotcue_4", LedValue::Rgb { r: 255, g: 0, b: 128 });

    encdr.set_led(device_id, "right_hotcue_1", LedValue::Rgb { r: 0, g: 255, b: 128 });
    encdr.set_led(device_id, "right_hotcue_2", LedValue::Rgb { r: 0, g: 180, b: 255 });
    encdr.set_led(device_id, "right_hotcue_3", LedValue::Rgb { r: 255, g: 128, b: 0 });
    encdr.set_led(device_id, "right_hotcue_4", LedValue::Rgb { r: 255, g: 0, b: 128 });

    // Underglow: ambient cyan/magenta
    encdr.set_led(device_id, "underglow_left", LedValue::Rgb { r: 0, g: 128, b: 255 });
    encdr.set_led(device_id, "underglow_right", LedValue::Rgb { r: 255, g: 0, b: 180 });

    // Backlights
    encdr.set_led(device_id, "mode", LedValue::Single(127));
    encdr.set_led(device_id, "left_sync", LedValue::Single(64));
    encdr.set_led(device_id, "right_sync", LedValue::Single(64));

    println!("Demo running! Turn knobs, press buttons, and observe the 5 OLED screens.");
    println!("Press Ctrl+C to stop.\n");

    let mut state = X1State::new();
    let mut canvas = Canvas::new();
    let events = encdr.events().clone();

    let mut last_render = Instant::now();

    loop {
        // 1. Process controller inputs
        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    println!("Button: {} -> {}", name, if pressed { "DOWN" } else { "UP" });

                    // Button LED mirroring
                    if name.contains("play") {
                        encdr.set_led(device_id, name, if pressed { LedValue::Single(127) } else { LedValue::Single(15) });
                    } else if name.contains("cue") {
                        encdr.set_led(device_id, name, if pressed { LedValue::Single(127) } else { LedValue::Single(15) });
                    } else if name.contains("sync") {
                        encdr.set_led(device_id, name, if pressed { LedValue::Single(127) } else { LedValue::Single(40) });
                    } else if name.contains("fx_") {
                        encdr.set_led(device_id, name, if pressed { LedValue::Single(127) } else { LedValue::Off });
                    } else if name == "shift" {
                        state.shift_active = pressed;
                        encdr.set_led(device_id, "shift", if pressed { LedValue::Single(127) } else { LedValue::Off });
                    } else if name == "mode" && pressed {
                        state.mode_index = (state.mode_index + 1) % MODES.len();
                        state.dirty = true;
                    }
                }
                Event::Encoder { name, delta, .. } => {
                    println!("Encoder: {} delta={}", name, delta);
                    if name == "left_loop" {
                        if delta > 0 {
                            state.left_loop_index = (state.left_loop_index + 1).min(LOOP_SIZES.len() - 1);
                        } else {
                            state.left_loop_index = state.left_loop_index.saturating_sub(1);
                        }
                        state.dirty = true;
                    } else if name == "right_loop" {
                        if delta > 0 {
                            state.right_loop_index = (state.right_loop_index + 1).min(LOOP_SIZES.len() - 1);
                        } else {
                            state.right_loop_index = state.right_loop_index.saturating_sub(1);
                        }
                        state.dirty = true;
                    }
                }
                Event::Slider { name, value, .. } => {
                    // FX Knobs
                    if let Some(idx) = name.strip_prefix("left_fx_knob_").and_then(|s| s.parse::<usize>().ok()) {
                        if idx >= 1 && idx <= 4 {
                            state.knobs[idx - 1] = value;
                            state.dirty = true;
                        }
                    } else if let Some(idx) = name.strip_prefix("right_fx_knob_").and_then(|s| s.parse::<usize>().ok()) {
                        if idx >= 1 && idx <= 4 {
                            state.knobs[idx + 3] = value;
                            state.dirty = true;
                        }
                    }
                }
                _ => {}
            }
        }

        // 2. Refresh screens at ~30 FPS if state changed
        if state.dirty || last_render.elapsed() >= Duration::from_millis(33) {
            last_render = Instant::now();

            // Screen 0: left_fx
            canvas.clear();
            canvas.draw_rect(0, 0, 128, 64, true);
            canvas.draw_text(6, 4, "LEFT FX UNIT", 1);
            for i in 0..4 {
                let y = 18 + (i as i32) * 11;
                canvas.draw_text(6, y, &format!("K{}:", i + 1), 1);
                canvas.draw_rect(28, y + 1, 92, 6, true);
                let bar_w = ((state.knobs[i] * 90.0) as i32).clamp(0, 90);
                canvas.fill_rect(29, y + 2, bar_w, 4, true);
            }
            encdr.submit_screen_with_format(device_id, "left_fx", &canvas.to_mono_bytes(), PixelFormat::Mono);

            // Screen 1: left_loop
            canvas.clear();
            canvas.draw_rect(0, 0, 128, 64, true);
            canvas.draw_text(24, 6, "DECK A LOOP", 1);
            let loop_str = LOOP_SIZES[state.left_loop_index];
            canvas.draw_text(48, 24, loop_str, 3);
            canvas.draw_text(30, 50, "BEATS", 1);
            encdr.submit_screen_with_format(device_id, "left_loop", &canvas.to_mono_bytes(), PixelFormat::Mono);

            // Screen 2: center_mode
            canvas.clear();
            canvas.draw_rect(0, 0, 128, 64, true);
            canvas.draw_text(38, 6, "- MODE -", 1);
            let mode_str = MODES[state.mode_index];
            let offset_x = (128 - (mode_str.len() as i32 * 6 * 2)) / 2;
            canvas.draw_text(offset_x, 24, mode_str, 2);
            canvas.draw_text(20, 50, "PRESS MODE TO SWAP", 1);
            encdr.submit_screen_with_format(device_id, "center_mode", &canvas.to_mono_bytes(), PixelFormat::Mono);

            // Screen 3: right_loop
            canvas.clear();
            canvas.draw_rect(0, 0, 128, 64, true);
            canvas.draw_text(24, 6, "DECK B LOOP", 1);
            let loop_str_b = LOOP_SIZES[state.right_loop_index];
            canvas.draw_text(48, 24, loop_str_b, 3);
            canvas.draw_text(30, 50, "BEATS", 1);
            encdr.submit_screen_with_format(device_id, "right_loop", &canvas.to_mono_bytes(), PixelFormat::Mono);

            // Screen 4: right_fx
            canvas.clear();
            canvas.draw_rect(0, 0, 128, 64, true);
            canvas.draw_text(6, 4, "RIGHT FX UNIT", 1);
            for i in 0..4 {
                let y = 18 + (i as i32) * 11;
                canvas.draw_text(6, y, &format!("K{}:", i + 1), 1);
                canvas.draw_rect(28, y + 1, 92, 6, true);
                let bar_w = ((state.knobs[i + 4] * 90.0) as i32).clamp(0, 90);
                canvas.fill_rect(29, y + 2, bar_w, 4, true);
            }
            encdr.submit_screen_with_format(device_id, "right_fx", &canvas.to_mono_bytes(), PixelFormat::Mono);

            state.dirty = false;
        }

        std::thread::sleep(Duration::from_millis(5));
    }
}
