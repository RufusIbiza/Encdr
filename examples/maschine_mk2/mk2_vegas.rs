//! NI Maschine Mk2 interactive Vegas LED demo & hardware telemetry.
//!
//! Features:
//! - Sweeping rainbow wave across all 16 RGB pads.
//! - Color cycling on Group A-H RGB buttons.
//! - Chasing brightness animations across monochrome mode, function, and transport LEDs.
//! - Full console telemetry reporting every button press, pad hit, and encoder rotation in real time.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin mk2_vegas

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
    println!("║         NI Maschine Mk2 — Vegas Mode & Telemetry             ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1140).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Maschine Mk2 (0x17cc:0x1140) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Strike pads, rotate encoders, or press buttons. Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();
    let mut pressed_pads = [false; 16];

    let mono_buttons = [
        "control", "step", "browser", "sampling", "arrow_left", "arrow_right", "all", "auto",
        "top_1", "top_2", "top_3", "top_4", "top_5", "top_6", "top_7", "top_8",
        "note_repeat", "enter", "nav_right", "nav_left", "tempo", "swing", "volume",
        "mute", "solo", "select", "duplicate", "navigate", "pad_mode", "pattern", "scene",
        "restart", "step_left", "step_right", "grid", "play", "rec", "shift", "erase"
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
                Event::DeviceDisconnected { .. } => {
                    eprintln!("Device disconnected.");
                    return;
                }
                _ => {}
            }
        }

        if last_led_update.elapsed() >= Duration::from_millis(30) {
            last_led_update = Instant::now();

            // 1. Rainbow waves on 16 RGB Pads
            for i in 1..=16 {
                let pad_name = format!("pad_{}", i);
                if pressed_pads[i - 1] {
                    encdr.set_led(device_id, &pad_name, LedValue::Rgb { r: 255, g: 255, b: 255 });
                } else {
                    let hue = (elapsed * 60.0 + (i as f32) * 22.5) % 360.0;
                    let (r, g, b) = hsv_to_rgb(hue, 1.0, 0.8);
                    encdr.set_led(device_id, &pad_name, LedValue::Rgb { r, g, b });
                }
            }

            // 2. Color cycling on Group buttons A-H
            for (idx, &group) in group_buttons.iter().enumerate() {
                let hue = (elapsed * 45.0 + (idx as f32) * 45.0) % 360.0;
                let (r, g, b) = hsv_to_rgb(hue, 1.0, 0.7);
                encdr.set_led(device_id, group, LedValue::Rgb { r, g, b });
            }

            // 3. Chasing wave on monochrome buttons
            for (idx, &btn) in mono_buttons.iter().enumerate() {
                let wave = ((elapsed * 4.0 + (idx as f32) * 0.25).sin() * 63.0 + 64.0) as u8;
                encdr.set_led(device_id, btn, LedValue::Single(wave));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
