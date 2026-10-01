//! NI Maschine Mikro Mk1 interactive Vegas LED demo & telemetry.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin mikro_mk1_vegas

use std::time::{Duration, Instant};
use encdr::{Encdr, EncdrConfig, Event, LedValue};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".parse().unwrap()))
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     NI Maschine Mikro Mk1 — Vegas Mode & Telemetry           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1110).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Maschine Mikro Mk1 (0x17cc:0x1110) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Hit pads, turn the master encoder, or press buttons to interact.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();
    let mut pressed_pads = [false; 16];

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    println!("[{:8.3}s] [BUTTON] {} -> {}", elapsed, name.to_uppercase(), if pressed { "\x1b[1;32mPRESSED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" });
                }
                Event::Grid { name, pressure, .. } => {
                    if let Some(idx_str) = name.strip_prefix("pad_") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if (1..=16).contains(&idx) {
                                pressed_pads[idx - 1] = pressure > 0.0;
                            }
                        }
                    }
                    if pressure > 0.0 {
                        println!("[{:8.3}s] [PAD] {} pressure = {:4.0}%", elapsed, name.to_uppercase(), pressure * 100.0);
                    }
                }
                Event::Encoder { name, delta, .. } => {
                    println!("[{:8.3}s] [ENCODER] {} delta: {:+2}", elapsed, name.to_uppercase(), delta);
                }
                Event::DeviceDisconnected { .. } => return,
                _ => {}
            }
        }

        if last_led_update.elapsed() >= Duration::from_millis(30) {
            last_led_update = Instant::now();

            // Animate chasing light wave across all 16 pads
            for i in 1..=16 {
                let pad_name = format!("pad_{}", i);
                if pressed_pads[i - 1] {
                    encdr.set_led(device_id, &pad_name, LedValue::Single(127));
                } else {
                    let phase = elapsed * 5.0 + (i as f32) * 0.4;
                    let brightness = ((phase.sin() * 0.5 + 0.5) * 127.0) as u8;
                    encdr.set_led(device_id, &pad_name, LedValue::Single(brightness));
                }
            }

            // Pulse transport/mode LEDs
            let pulse = ((elapsed * 4.0).sin() * 63.0 + 64.0) as u8;
            for btn in &["f1", "f2", "f3", "group", "browse", "sampling", "play", "rec", "restart", "erase"] {
                encdr.set_led(device_id, btn, LedValue::Single(pulse));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
