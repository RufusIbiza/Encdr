//! NI Maschine Mk1 interactive Vegas LED & screen demo with hardware telemetry.
//!
//! Features:
//! - Brightness wave across the 16 pads; struck pads light fully.
//! - Chasing brightness animation across all button LEDs.
//! - Both 255×64 grayscale screens: a live position bar for every knob
//!   (screen encoders 1-4 left, 5-8 and volume/tempo/swing right) and a 4×4
//!   pad pressure grid.
//! - Full console telemetry reporting every button press, pad hit, and knob turn.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin mk1_vegas

use encdr::{Encdr, EncdrConfig, Event, LedValue, PixelFormat};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const WIDTH: usize = 255;
const HEIGHT: usize = 64;

/// Brightest LED value NI's own software uses on the Mk1.
const LED_MAX: f32 = 92.0;

const BUTTON_LEDS: [&str; 41] = [
    "mute",
    "solo",
    "select",
    "duplicate",
    "navigate",
    "pad_mode",
    "pattern",
    "scene",
    "shift",
    "erase",
    "grid",
    "step_right",
    "rec",
    "play",
    "step_left",
    "restart",
    "group_h",
    "group_g",
    "group_f",
    "group_e",
    "group_d",
    "group_c",
    "group_b",
    "group_a",
    "auto_write",
    "snap",
    "arrow_right",
    "arrow_left",
    "sampling",
    "browser",
    "step",
    "control",
    "top_8",
    "top_7",
    "top_6",
    "top_5",
    "top_4",
    "top_3",
    "top_2",
    "top_1",
    "note_repeat",
];

struct Canvas {
    pixels: Vec<u8>,
}

impl Canvas {
    fn new() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT * 4],
        }
    }

    fn clear(&mut self) {
        self.pixels.fill(0);
    }

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, level: u8) {
        for py in y..(y + h).min(HEIGHT) {
            for px in x..(x + w).min(WIDTH) {
                let i = (py * WIDTH + px) * 4;
                self.pixels[i..i + 4].copy_from_slice(&[level, level, level, 255]);
            }
        }
    }

    fn outline(&mut self, x: usize, y: usize, w: usize, h: usize, level: u8) {
        self.fill_rect(x, y, w, 1, level);
        self.fill_rect(x, y + h - 1, w, 1, level);
        self.fill_rect(x, y, 1, h, level);
        self.fill_rect(x + w - 1, y, 1, h, level);
    }

    /// Vertical bar showing a knob's position within its current turn.
    fn knob_bar(&mut self, x: usize, turns: f32, highlight: bool) {
        let frac = turns.rem_euclid(1.0);
        let h = HEIGHT - 4;
        let filled = (frac * h as f32) as usize;
        self.outline(x, 2, 18, h, if highlight { 255 } else { 120 });
        self.fill_rect(
            x + 2,
            2 + h - filled,
            14,
            filled,
            if highlight { 255 } else { 180 },
        );
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".parse().unwrap()),
        )
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         NI Maschine Mk1 — Vegas Mode & Telemetry             ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids
        .into_iter()
        .find(|&id| {
            encdr
                .device_descriptor(id)
                .map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x0808)
                .unwrap_or(false)
        })
        .unwrap_or_else(|| {
            eprintln!("❌ NI Maschine Mk1 (0x17cc:0x0808) not found.");
            std::process::exit(1);
        });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(
        " Connected: {} (VID:PID 0x{:04x}:0x{:04x})",
        desc.name, desc.vendor_id.0, desc.product_id.0
    );
    println!(" Controls available: {}\n", desc.control_count());
    println!("Strike pads, turn knobs, or press buttons. Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();
    let mut last_frame = Instant::now();
    let mut pad_pressure = [0.0f32; 16];
    let mut knobs: HashMap<&'static str, f32> = HashMap::new();
    let mut last_knob: Option<(&'static str, Instant)> = None;
    let mut left = Canvas::new();
    let mut right = Canvas::new();

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    println!(
                        "[{:8.3}s] [BUTTON] {} -> {}",
                        elapsed,
                        name.to_uppercase(),
                        if pressed {
                            "\x1b[1;32mPRESSED\x1b[0m"
                        } else {
                            "\x1b[2mRELEASED\x1b[0m"
                        }
                    );
                }
                Event::Grid { name, pressure, .. } => {
                    if let Some(n) = name
                        .strip_prefix("pad_")
                        .and_then(|n| n.parse::<usize>().ok())
                    {
                        pad_pressure[n - 1] = pressure;
                    }
                    println!(
                        "[{:8.3}s] [GRID] {} pressure: {:5.1}%",
                        elapsed,
                        name.to_uppercase(),
                        pressure * 100.0
                    );
                }
                Event::EncoderFine { name, delta, .. } => {
                    *knobs.entry(name).or_default() += delta;
                    last_knob = Some((name, Instant::now()));
                    println!(
                        "[{:8.3}s] [KNOB] {} delta: {:+.3} turns",
                        elapsed,
                        name.to_uppercase(),
                        delta
                    );
                }
                Event::DeviceDisconnected { .. } => {
                    eprintln!("Device disconnected.");
                    return;
                }
                _ => {}
            }
        }

        if last_led_update.elapsed() >= Duration::from_millis(30) {
            last_led_update = Instant::now();

            for i in 1..=16 {
                let level = if pad_pressure[i - 1] > 0.0 {
                    LED_MAX
                } else {
                    let row = (i - 1) / 4;
                    let col = (i - 1) % 4;
                    ((elapsed * 3.0 + (row + col) as f32 * 0.6).sin() * 0.5 + 0.5) * LED_MAX * 0.6
                };
                encdr.set_led(
                    device_id,
                    &format!("pad_{i}"),
                    LedValue::Single(level as u8),
                );
            }

            for (idx, &btn) in BUTTON_LEDS.iter().enumerate() {
                let wave = ((elapsed * 4.0 + idx as f32 * 0.25).sin() * 0.5 + 0.5) * LED_MAX;
                encdr.set_led(device_id, btn, LedValue::Single(wave as u8));
            }
        }

        if last_frame.elapsed() >= Duration::from_millis(33) {
            last_frame = Instant::now();
            let active = last_knob
                .filter(|(_, t)| t.elapsed() < Duration::from_millis(400))
                .map(|(n, _)| n);
            let pos = |name: &str| knobs.get(name).copied().unwrap_or(0.0);

            // Left screen: screen encoders 1-4, then the pad grid.
            left.clear();
            for k in 0..4 {
                let name = [
                    "screen_encoder_1",
                    "screen_encoder_2",
                    "screen_encoder_3",
                    "screen_encoder_4",
                ][k];
                left.knob_bar(4 + k * 26, pos(name), active == Some(name));
            }
            for p in 0..16 {
                // Pad 13 is top-left, pad 1 bottom-left.
                let (row, col) = (3 - p / 4, p % 4);
                let (x, y) = (130 + col * 31, 2 + row * 15);
                let level = (pad_pressure[p] * 255.0) as u8;
                left.outline(x, y, 29, 14, 90);
                left.fill_rect(x + 1, y + 1, 27, 12, level);
            }
            // Right screen: screen encoders 5-8, then volume / tempo / swing.
            right.clear();
            let right_knobs = [
                "screen_encoder_5",
                "screen_encoder_6",
                "screen_encoder_7",
                "screen_encoder_8",
                "volume",
                "tempo",
                "swing",
            ];
            for (k, name) in right_knobs.iter().enumerate() {
                let x = if k < 4 {
                    4 + k * 26
                } else {
                    130 + (k - 4) * 30
                };
                right.knob_bar(x, pos(name), active == Some(*name));
            }

            encdr.submit_screen_with_format(device_id, "left", &left.pixels, PixelFormat::Rgba8888);
            encdr.submit_screen_with_format(
                device_id,
                "right",
                &right.pixels,
                PixelFormat::Rgba8888,
            );
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
