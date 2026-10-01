//! NI Maschine Mikro Mk3 interactive Vegas LED demo & telemetry.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin mikro_mk3_vegas

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
    println!("║     NI Maschine Mikro Mk3 — Vegas Mode & Telemetry           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1700).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Maschine Mikro Mk3 (0x17cc:0x1700) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Slide touchstrip, hit pads, or turn the encoder to interact.\n");

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
                Event::Slider { name, value, .. } => {
                    println!("[{:8.3}s] [TOUCHSTRIP] {} = {:5.1}%", elapsed, name.to_uppercase(), value * 100.0);
                }
                Event::Touch { name, touched, .. } => {
                    println!("[{:8.3}s] [TOUCH] {} -> {}", elapsed, name.to_uppercase(), if touched { "\x1b[1;36mCONTACT\x1b[0m" } else { "\x1b[2mRELEASE\x1b[0m" });
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

            // 1. Animate 16 RGB pads
            for i in 1..=16 {
                let pad_name = format!("pad_{}", i);
                if pressed_pads[i - 1] {
                    encdr.set_led(device_id, &pad_name, LedValue::Single(0x47)); // White
                } else {
                    let hue = (elapsed * 60.0 + (i as f32) * 22.5) % 360.0;
                    let (r, g, b) = hsv_to_rgb(hue, 1.0, 0.8);
                    encdr.set_led(device_id, &pad_name, LedValue::Rgb { r, g, b });
                }
            }

            // 2. Animate 25-segment Smart Strip
            let mut strip_leds = [0u8; 25];
            let pos = ((elapsed * 3.0).sin() * 0.5 + 0.5) * 24.0;
            for i in 0..25 {
                let dist = (i as f32 - pos).abs();
                if dist < 2.0 {
                    strip_leds[i] = ((1.0 - dist / 2.0) * 127.0) as u8;
                }
            }
            encdr.set_led_strip(device_id, "touchstrip_leds", &strip_leds);

            // 3. Pulse mode buttons
            let pulse = ((elapsed * 4.0).sin() * 63.0 + 64.0) as u8;
            for btn in &["masch_group", "auto", "lock", "sampling", "pad_mode", "keyboard", "chords", "step", "play", "rec"] {
                encdr.set_led(device_id, btn, LedValue::Single(pulse));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
