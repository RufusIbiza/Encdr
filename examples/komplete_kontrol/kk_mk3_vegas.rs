//! NI Komplete Kontrol Mk3 (S49 / S61 / S88) interactive Vegas LED demo & telemetry.
//!
//! Features:
//! - Sweeping RGB Light Guide wave across the entire keybed.
//! - Live console telemetry reporting 4-D encoder gestures, encoder data, and buttons.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin kk_mk3_vegas

use std::time::{Duration, Instant};
use encdr::{Encdr, EncdrConfig, Event};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".parse().unwrap()))
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     NI Komplete Kontrol Mk3 — Vegas Mode & Telemetry         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| {
            d.vendor_id.0 == 0x17cc && (d.product_id.0 == 0x2100 || d.product_id.0 == 0x2110 || d.product_id.0 == 0x2120)
        }).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Komplete Kontrol S-Series Mk3 (0x17cc:0x2100/0x2110/0x2120) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Rotate encoders, press buttons, or use the 4D encoder. Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();

    let key_count = match desc.product_id.0 {
        0x2110 => 61,
        0x2120 => 88,
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
                Event::Slider { name, value, .. } => {
                    println!("[{:8.3}s] [TOUCHSTRIP] {} = {:5.1}%", elapsed, name.to_uppercase(), value * 100.0);
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

            // Light Guide wave animation
            let mut lg_pattern = vec![0u8; key_count];
            for (idx, slot) in lg_pattern.iter_mut().enumerate() {
                let wave = ((elapsed * 4.0 + (idx as f32) * 0.3).sin() * 63.0 + 64.0) as u8;
                *slot = wave;
            }
            encdr.set_led_strip(device_id, "light_guide", &lg_pattern);
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
