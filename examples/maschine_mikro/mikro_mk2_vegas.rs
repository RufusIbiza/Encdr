//! NI Maschine Mikro Mk2 interactive Vegas LED demo & telemetry.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin mikro_mk2_vegas

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
    println!("║     NI Maschine Mikro Mk2 — Vegas Mode & Telemetry           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1200).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Maschine Mikro Mk2 (0x17cc:0x1200) not found.");
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

            // Animate 16 RGB pads with sweeping rainbow spectrum
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

            // Pulse transport/mode LEDs
            let pulse = ((elapsed * 4.0).sin() * 63.0 + 64.0) as u8;
            for btn in &["f1", "f2", "f3", "group", "browse", "sampling", "play", "rec", "restart", "erase"] {
                encdr.set_led(device_id, btn, LedValue::Single(pulse));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
