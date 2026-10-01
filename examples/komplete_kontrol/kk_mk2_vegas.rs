//! NI Komplete Kontrol Mk2 (S49 / S61 / S88) interactive Vegas LED demo & telemetry.
//!
//! Features:
//! - Sweeping RGB Light Guide rainbow wave across the entire keybed.
//! - Chasing brightness animations across function, preset, display, and transport button LEDs.
//! - Live console telemetry reporting all 4-D encoder gestures, knobs, and button presses.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin kk_mk2_vegas

use std::time::{Duration, Instant};
use encdr::{Encdr, EncdrConfig, Event, LedValue};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".parse().unwrap()))
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     NI Komplete Kontrol Mk2 — Vegas Mode & Telemetry         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| {
            d.vendor_id.0 == 0x17cc && (d.product_id.0 == 0x1610 || d.product_id.0 == 0x1620 || d.product_id.0 == 0x1630)
        }).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Komplete Kontrol S-Series Mk2 (0x17cc:0x1610/0x1620/0x1630) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Rotate encoders, press buttons, or use the 4D encoder. Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();

    let buttons = [
        "top_1", "top_2", "top_3", "top_4", "top_5", "top_6", "top_7", "top_8",
        "shift", "page_left", "page_right", "mute", "solo",
        "play", "rec", "stop", "preset_up", "preset_down", "plugin", "midi"
    ];

    let key_count = match desc.product_id.0 {
        0x1620 => 61,
        0x1630 => 88,
        _ => 49,
    };

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    println!("[{:8.3}s] [BUTTON] {} -> {}", elapsed, name.to_uppercase(), if pressed { "\x1b[1;32mPRESSED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" });
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

            // 1. Light Guide animation
            let mut lg_pattern = vec![0u8; key_count];
            for (idx, slot) in lg_pattern.iter_mut().enumerate() {
                let wave = ((elapsed * 4.0 + (idx as f32) * 0.3).sin() * 63.0 + 64.0) as u8;
                *slot = wave;
            }
            encdr.set_led_strip(device_id, "light_guide", &lg_pattern);

            // 2. Pulse function and transport buttons
            for (idx, &btn) in buttons.iter().enumerate() {
                let wave = ((elapsed * 4.0 + (idx as f32) * 0.25).sin() * 63.0 + 64.0) as u8;
                encdr.set_led(device_id, btn, LedValue::Single(wave));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
