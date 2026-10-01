//! NI Traktor Kontrol S2 Mk2 interactive Vegas LED demo & telemetry.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin s2_mk2_vegas

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
    println!("║     NI Traktor Kontrol S2 Mk2 — Vegas Mode & Telemetry       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1320).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Traktor Kontrol S2 Mk2 (0x17cc:0x1320) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Touch jog wheels, move faders, or hit buttons to interact. Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    println!("[{:8.3}s] [BUTTON] {} -> {}", elapsed, name.to_uppercase(), if pressed { "\x1b[1;32mPRESSED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" });
                }
                Event::Slider { name, value, .. } => {
                    println!("[{:8.3}s] [FADER/PITCH] {} = {:5.1}%", elapsed, name.to_uppercase(), value * 100.0);
                }
                Event::Encoder { name, delta, .. } => {
                    println!("[{:8.3}s] [JOG/ENC] {} delta: {:+2}", elapsed, name.to_uppercase(), delta);
                }
                Event::DeviceDisconnected { .. } => return,
                _ => {}
            }
        }

        if last_led_update.elapsed() >= Duration::from_millis(30) {
            last_led_update = Instant::now();

            // 1. Animate RGB Cue buttons on Left and Right decks
            for i in 1..=4 {
                let cue_l = format!("left_cue_{}", i);
                let cue_r = format!("right_cue_{}", i);
                let hue_l = (elapsed * 60.0 + (i as f32) * 45.0) % 360.0;
                let hue_r = (elapsed * 60.0 + (i as f32 + 4.0) * 45.0) % 360.0;
                let (rl, gl, bl) = hsv_to_rgb(hue_l, 1.0, 0.8);
                let (rr, gr, br) = hsv_to_rgb(hue_r, 1.0, 0.8);
                encdr.set_led(device_id, &cue_l, LedValue::Rgb { r: rl, g: gl, b: bl });
                encdr.set_led(device_id, &cue_r, LedValue::Rgb { r: rr, g: gr, b: br });
            }

            // 2. Pulse Transport & Mode Buttons
            let pulse = ((elapsed * 4.0).sin() * 63.0 + 64.0) as u8;
            for btn in &["left_play", "left_cue", "left_sync", "left_flux", "right_play", "right_cue", "right_sync", "right_flux", "mixer_cue_a", "mixer_cue_b"] {
                encdr.set_led(device_id, btn, LedValue::Single(pulse));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
