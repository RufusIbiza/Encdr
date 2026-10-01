//! NI Traktor Kontrol S8 interactive Vegas LED demo & hardware telemetry.
//!
//! Features:
//! - Rainbow wave across all 16 RGB performance pads (Deck A & Deck B/C/D).
//! - Dual-color sine wave animations across both left & right 25-LED touchstrips.
//! - Chasing brightness animations across deck selection, screen buttons, and transport LEDs.
//! - Real-time console reporting of all 4 mixer channels, faders, knobs, pads, and encoders.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin s8_vegas

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
    println!("║       NI Traktor Kontrol S8 — Vegas Mode & Telemetry         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1370).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Traktor Kontrol S8 (0x17cc:0x1370) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Move faders, touch dials, or press pads. Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();
    let mut pressed_pads = [false; 16];

    let buttons = [
        "left_play", "left_cue", "left_sync_green", "left_shift", "left_flux", "left_deck_white",
        "right_play", "right_cue", "right_sync_green", "right_shift", "right_flux", "right_deck_white",
        "left_on_1", "left_on_2", "left_on_3", "left_on_4",
        "right_on_1", "right_on_2", "right_on_3", "right_on_4"
    ];

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    if let Some(idx_str) = name.strip_prefix("left_pad_") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if (1..=8).contains(&idx) {
                                pressed_pads[idx - 1] = pressed;
                            }
                        }
                    } else if let Some(idx_str) = name.strip_prefix("right_pad_") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if (1..=8).contains(&idx) {
                                pressed_pads[idx + 7] = pressed;
                            }
                        }
                    }
                    println!("[{:8.3}s] [BUTTON] {} -> {}", elapsed, name.to_uppercase(), if pressed { "\x1b[1;32mPRESSED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" });
                }
                Event::Slider { name, value, .. } => {
                    println!("[{:8.3}s] [ANALOG] {} = {:5.1}%", elapsed, name.to_uppercase(), value * 100.0);
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

            // 1. Rainbow waves on Left 8 pads & Right 8 pads
            for i in 1..=8 {
                let left_pad = format!("left_pad_{}", i);
                let right_pad = format!("right_pad_{}", i);

                if pressed_pads[i - 1] {
                    encdr.set_led(device_id, &left_pad, LedValue::Rgb { r: 255, g: 255, b: 255 });
                } else {
                    let hue = (elapsed * 60.0 + (i as f32) * 45.0) % 360.0;
                    let (r, g, b) = hsv_to_rgb(hue, 1.0, 0.8);
                    encdr.set_led(device_id, &left_pad, LedValue::Rgb { r, g, b });
                }

                if pressed_pads[i + 7] {
                    encdr.set_led(device_id, &right_pad, LedValue::Rgb { r: 255, g: 255, b: 255 });
                } else {
                    let hue = (elapsed * 60.0 + (i as f32) * 45.0 + 180.0) % 360.0;
                    let (r, g, b) = hsv_to_rgb(hue, 1.0, 0.8);
                    encdr.set_led(device_id, &right_pad, LedValue::Rgb { r, g, b });
                }
            }

            // 2. Touchstrip dual-color meters
            let mut left_blue = [0u8; 25];
            let mut left_orange = [0u8; 25];
            let mut right_blue = [0u8; 25];
            let mut right_orange = [0u8; 25];
            for i in 0..25 {
                let l_b = ((elapsed * 5.0 + (i as f32) * 0.3).sin() * 63.0 + 64.0) as u8;
                let l_o = (((elapsed * 5.0 + (i as f32) * 0.3) + std::f32::consts::PI).sin() * 63.0 + 64.0) as u8;
                left_blue[i] = l_b;
                left_orange[i] = l_o;
                right_blue[i] = l_o;
                right_orange[i] = l_b;
            }
            encdr.set_led_strip(device_id, "left_touchstrip_blue", &left_blue);
            encdr.set_led_strip(device_id, "left_touchstrip_orange", &left_orange);
            encdr.set_led_strip(device_id, "right_touchstrip_blue", &right_blue);
            encdr.set_led_strip(device_id, "right_touchstrip_orange", &right_orange);

            // 3. Pulse single buttons
            for (idx, &btn) in buttons.iter().enumerate() {
                let wave = ((elapsed * 4.0 + (idx as f32) * 0.25).sin() * 63.0 + 64.0) as u8;
                encdr.set_led(device_id, btn, LedValue::Single(wave));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
