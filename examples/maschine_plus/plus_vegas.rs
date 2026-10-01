//! NI Maschine+ interactive Vegas LED demo & hardware telemetry.
//!
//! Features:
//! - Sweeping rainbow wave across all 16 RGB pads and Touchstrip.
//! - Chasing brightness animations across button LEDs and Group A-H indicators.
//! - Real-time console reporting of all 4-D encoder gestures, knobs, touch events, and pad strikes.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin plus_vegas

use std::time::{Duration, Instant};
use encdr::{Encdr, EncdrConfig, Event, LedValue};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".parse().unwrap()))
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         NI Maschine+ — Vegas Mode & Telemetry                ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1820).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Maschine+ (0x17cc:0x1820) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Interact with buttons, knobs, touchstrip, and pads. Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();
    let mut pressed_pads = [false; 16];

    let buttons = [
        "channel", "plugin", "arranger", "mixer", "browser", "sampling",
        "arrow_left", "arrow_right", "file", "settings", "auto", "macro",
        "top_1", "top_2", "top_3", "top_4", "top_5", "top_6", "top_7", "top_8",
        "volume", "swing", "note_repeat", "tempo", "lock", "pitch", "mod", "perform", "notes",
        "group_a", "group_b", "group_c", "group_d", "group_e", "group_f", "group_g", "group_h",
        "restart", "erase", "tap", "follow", "play", "rec", "stop", "shift",
        "fixed_vel", "pad_mode", "keyboard", "chords", "step", "scene", "pattern", "events",
        "variations", "duplicate", "select", "solo", "mute"
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
                Event::Slider { name, value, .. } => {
                    println!("[{:8.3}s] [TOUCHSTRIP] {} = {:5.1}%", elapsed, name.to_uppercase(), value * 100.0);
                }
                Event::Touch { name, touched, .. } => {
                    println!("[{:8.3}s] [CAP-TOUCH] {} -> {}", elapsed, name.to_uppercase(), if touched { "\x1b[1;34mTOUCHED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" });
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

            // 1. Color cycling on 16 Pads (using single byte palette / brightness)
            for i in 1..=16 {
                let pad_name = format!("pad_{}", i);
                if pressed_pads[i - 1] {
                    encdr.set_led(device_id, &pad_name, LedValue::Single(0x47)); // Bright white
                } else {
                    let color_index = (((elapsed * 12.0) as usize + i * 4) % 64 + 4) as u8;
                    encdr.set_led(device_id, &pad_name, LedValue::Single(color_index));
                }
            }

            // 2. Animate 25 touchstrip LEDs
            let mut strip_pattern = [0u8; 25];
            let center = ((elapsed * 3.0).sin() * 11.0 + 12.0) as usize;
            for (idx, slot) in strip_pattern.iter_mut().enumerate() {
                let dist = (idx as isize - center as isize).abs();
                if dist < 4 {
                    *slot = (127 - dist * 30).max(0) as u8;
                }
            }
            encdr.set_led_strip(device_id, "touchstrip", &strip_pattern);

            // 3. Pulse all button LEDs
            for (idx, &btn) in buttons.iter().enumerate() {
                let wave = ((elapsed * 4.0 + (idx as f32) * 0.2).sin() * 63.0 + 64.0) as u8;
                encdr.set_led(device_id, btn, LedValue::Single(wave));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
