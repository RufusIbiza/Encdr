//! NI Traktor Kontrol Z1 interactive Vegas LED demo & telemetry.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin z1_vegas

use std::time::{Duration, Instant};
use encdr::{Encdr, EncdrConfig, Event, LedValue};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".parse().unwrap()))
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       NI Traktor Kontrol Z1 — Vegas Mode & Telemetry         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1210).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Traktor Kontrol Z1 (0x17cc:0x1210) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Move faders, turn EQ/filter knobs, or press cue buttons to interact.\n");

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
                Event::DeviceDisconnected { .. } => return,
                _ => {}
            }
        }

        if last_led_update.elapsed() >= Duration::from_millis(30) {
            last_led_update = Instant::now();

            // 1. Animate Stereo VU Meters (7 LEDs each) with bouncing peak meters
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

            // 2. Pulse Cue & Mode Buttons
            let pulse = ((elapsed * 5.0).sin() * 63.0 + 64.0) as u8;
            for btn in &["cue_a", "cue_b", "mode_a", "mode_b"] {
                encdr.set_led(device_id, btn, LedValue::Single(pulse));
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
