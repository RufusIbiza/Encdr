//! NI Traktor Kontrol X1 Mk1 interactive Vegas LED demo & telemetry.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin x1_mk1_vegas

use std::time::{Duration, Instant};
use encdr::{Encdr, EncdrConfig, Event, LedValue};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".parse().unwrap()))
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     NI Traktor Kontrol X1 Mk1 — Vegas Mode & Telemetry       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && (d.product_id.0 == 0x2305 || d.product_id.0 == 0x1000)).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Traktor Kontrol X1 Mk1 (0x17cc:0x2305 / 0x1000) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Turn knobs/encoders or press buttons to interact. Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();

    const BUTTON_LEDS: &[&str] = &[
        "fx1_on", "fx1_1", "fx1_2", "fx1_3",
        "fx2_on", "fx2_1", "fx2_2", "fx2_3",
        "hotcue_left_1", "hotcue_left_2", "hotcue_left_3", "hotcue_left_4",
        "hotcue_right_1", "hotcue_right_2", "hotcue_right_3", "hotcue_right_4",
        "in_left", "out_left", "in_right", "out_right",
        "play_left", "cue_left", "sync_left", "shift",
        "play_right", "cue_right", "sync_right",
    ];

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    println!("[{:8.3}s] [BUTTON] {} -> {}", elapsed, name.to_uppercase(), if pressed { "\x1b[1;32mPRESSED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" });
                }
                Event::Slider { name, value, .. } => {
                    println!("[{:8.3}s] [KNOB] {} = {:5.1}%", elapsed, name.to_uppercase(), value * 100.0);
                }
                Event::Encoder { name, delta, .. } => {
                    println!("[{:8.3}s] [ENCODER] {} delta: {:+2}", elapsed, name.to_uppercase(), delta);
                }
                Event::DeviceDisconnected { .. } => return,
                _ => {}
            }
        }

        if last_led_update.elapsed() >= Duration::from_millis(33) {
            last_led_update = Instant::now();

            // Animate chasing LED pattern across all buttons
            for (idx, btn) in BUTTON_LEDS.iter().enumerate() {
                let phase = elapsed * 5.0 + (idx as f32) * 0.3;
                let brightness = ((phase.sin() * 0.5 + 0.5) * 127.0) as u8;
                encdr.set_led(device_id, btn, LedValue::Single(brightness));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
