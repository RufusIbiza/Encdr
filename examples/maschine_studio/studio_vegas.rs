//! NI Maschine Studio interactive Vegas LED demo & hardware telemetry.
//!
//! Features:
//! - Rainbow wave across all 16 RGB pads and Group A-H buttons.
//! - Rotating chase pattern around the 32-segment Jogwheel LED ring.
//! - Bouncing stereo VU meter bars on the master meters (left & right 16-segment strips).
//! - Chasing brightness animations across editing, navigation, and transport button LEDs.
//! - Live console telemetry for jogwheel spin, level encoder, capacitive touch, buttons, and pads.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin studio_vegas

use std::time::{Duration, Instant};
use encdr::{Encdr, EncdrConfig, Event, LedValue};

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let hp = (h % 360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8)
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".parse().unwrap()))
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       NI Maschine Studio — Vegas Mode & Telemetry            ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1300).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Maschine Studio (0x17cc:0x1300) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Spin jogwheel, strike pads, or press buttons. Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();
    let mut pressed_pads = [false; 16];

    let buttons = [
        "channel", "plugin", "arrange", "mix", "browse", "sampling", "all", "auto",
        "tap", "snap", "macro", "note_repeat", "scene", "pattern", "pad_mode", "navigate",
        "duplicate", "select", "solo", "mute", "loop", "metro", "grid", "play", "rec", "erase",
        "copy", "paste", "note", "nudge", "undo", "redo", "quantize", "clear", "back",
        "nav_left", "nav_right", "enter", "in_1", "in_2", "in_3", "in_4", "master", "group", "sound", "cue",
        "top_1", "top_2", "top_3", "top_4", "top_5", "top_6", "top_7", "top_8"
    ];

    let group_buttons = [
        "group_a", "group_b", "group_c", "group_d",
        "group_e", "group_f", "group_g", "group_h"
    ];

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    if let Some(idx_str) = name.strip_prefix("pad_") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if (1..=16).contains(&idx) {
                                pressed_pads[idx - 1] = pressed;
                            }
                        }
                    }
                    println!("[{:8.3}s] [BUTTON] {} -> {}", elapsed, name.to_uppercase(), if pressed { "\x1b[1;32mPRESSED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" });
                }
                Event::Grid { name, index, pressure, .. } => {
                    if (1..=16).contains(&index) {
                        pressed_pads[(index - 1) as usize] = pressure > 0.0;
                    }
                    println!("[{:8.3}s] [GRID] {} (#{index:02}) pressure: {:5.1}%", elapsed, name.to_uppercase(), pressure * 100.0);
                }
                Event::Encoder { name, delta, .. } => {
                    println!("[{:8.3}s] [ENCODER] {} delta: {:+2}", elapsed, name.to_uppercase(), delta);
                }
                Event::EncoderFine { name, delta, .. } => {
                    println!("[{:8.3}s] [ENCODER FINE] {} delta: {:+.3}", elapsed, name.to_uppercase(), delta);
                }
                Event::Touch { name, touched, .. } => {
                    println!("[{:8.3}s] [CAP-TOUCH] {} -> {}", elapsed, name.to_uppercase(), if touched { "\x1b[1;34mTOUCHED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" });
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

            // 1. Rainbow waves on 16 Pads (indexed palette single byte)
            for i in 1..=16 {
                let pad_name = format!("pad_{}", i);
                if pressed_pads[i - 1] {
                    encdr.set_led(device_id, &pad_name, LedValue::Single(0x47)); // Bright white
                } else {
                    let color_index = (((elapsed * 12.0) as usize + i * 4) % 64 + 4) as u8;
                    encdr.set_led(device_id, &pad_name, LedValue::Single(color_index));
                }
            }

            // 2. Color cycling on Group buttons A-H
            for (idx, &group) in group_buttons.iter().enumerate() {
                let hue = (elapsed * 45.0 + (idx as f32) * 45.0) % 360.0;
                let (r, g, b) = hsv_to_rgb(hue, 1.0, 0.7);
                encdr.set_led(device_id, group, LedValue::Rgb { r, g, b });
            }

            // 3. Rotating spinner on 32-segment Jogwheel LED ring
            let mut ring = [0u8; 32];
            let pos = ((elapsed * 24.0) as usize) % 32;
            for i in 0..6 {
                let idx = (pos + i) % 32;
                ring[idx] = (127 - (5 - i) * 20) as u8;
            }
            encdr.set_led_strip(device_id, "jogwheel_ring", &ring);

            // 4. Stereo VU meters bouncing sine waves
            let left_level = (((elapsed * 3.5).sin() * 0.5 + 0.5) * 16.0) as usize;
            let right_level = (((elapsed * 4.0 + 1.0).sin() * 0.5 + 0.5) * 16.0) as usize;
            let mut left_meter = [0u8; 16];
            let mut right_meter = [0u8; 16];
            for i in 0..left_level.min(16) {
                left_meter[i] = 127;
            }
            for i in 0..right_level.min(16) {
                right_meter[i] = 127;
            }
            encdr.set_led_strip(device_id, "meter_left", &left_meter);
            encdr.set_led_strip(device_id, "meter_right", &right_meter);

            // 5. Pulse monochrome buttons
            for (idx, &btn) in buttons.iter().enumerate() {
                let wave = ((elapsed * 4.0 + (idx as f32) * 0.2).sin() * 63.0 + 64.0) as u8;
                encdr.set_led(device_id, btn, LedValue::Single(wave));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
