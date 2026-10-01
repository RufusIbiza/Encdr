//! NI Traktor Kontrol Z2 interactive Vegas LED demo & telemetry.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin z2_vegas

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
    println!("║       NI Traktor Kontrol Z2 — Vegas Mode & Telemetry         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1230).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Traktor Kontrol Z2 (0x17cc:0x1230) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Move Innofaders, turn EQ knobs, or hit cue pads to interact.\n");

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
                    println!("[{:8.3}s] [FADER/KNOB] {} = {:5.1}%", elapsed, name.to_uppercase(), value * 100.0);
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

            // 1. Animate 8 RGB Cue/Remix Pads (Deck A and B)
            for i in 1..=4 {
                let pad_a = format!("pad_a_{}", i);
                let pad_b = format!("pad_b_{}", i);
                let hue_a = (elapsed * 60.0 + (i as f32) * 45.0) % 360.0;
                let hue_b = (elapsed * 60.0 + (i as f32 + 4.0) * 45.0) % 360.0;
                let (ra, ga, ba) = hsv_to_rgb(hue_a, 1.0, 0.8);
                let (rb, gb, bb) = hsv_to_rgb(hue_b, 1.0, 0.8);
                encdr.set_led(device_id, &pad_a, LedValue::Rgb { r: ra, g: ga, b: ba });
                encdr.set_led(device_id, &pad_b, LedValue::Rgb { r: rb, g: gb, b: bb });
            }

            // 2. Animate Stereo Master/Channel VU Meters
            let mut meter_a = [0u8; 7];
            let mut meter_b = [0u8; 7];
            let peak_a = ((elapsed * 4.0).sin() * 0.5 + 0.5) * 7.0;
            let peak_b = (((elapsed * 4.0) + 1.0).sin() * 0.5 + 0.5) * 7.0;

            for i in 0..7 {
                if (i as f32) + 1.0 <= peak_a {
                    meter_a[i] = 127;
                }
                if (i as f32) + 1.0 <= peak_b {
                    meter_b[i] = 127;
                }
            }
            encdr.set_led_strip(device_id, "vu_meter_a", &meter_a);
            encdr.set_led_strip(device_id, "vu_meter_b", &meter_b);

            // 3. Animate Loop Display Digits
            let loop_lengths = [32.0, 16.0, 8.0, 4.0, 2.0, 1.0, 0.5, 0.25];
            let idx = ((elapsed * 1.5) as usize) % loop_lengths.len();
            encdr.set_loop_display(device_id, "display_a_1", "display_a_2", loop_lengths[idx], true);
            encdr.set_loop_display(device_id, "display_b_1", "display_b_2", loop_lengths[(idx + 4) % loop_lengths.len()], false);

            // 4. Pulse Transport / Mode LEDs
            let pulse = ((elapsed * 4.0).sin() * 63.0 + 64.0) as u8;
            for btn in &["play_a", "cue_a", "sync_a", "flux_a", "play_b", "cue_b", "sync_b", "flux_b", "deck_c", "deck_d"] {
                encdr.set_led(device_id, btn, LedValue::Single(pulse));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
