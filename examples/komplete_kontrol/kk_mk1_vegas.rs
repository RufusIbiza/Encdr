//! NI Komplete Kontrol Mk1 (S25 / S49 / S61 / S88) interactive Vegas LED demo & telemetry.
//!
//! Features:
//! - Sweeping Light Guide wave across the keybed (25, 49, 61, or 88 keys).
//! - Chasing brightness animations across function, transport, preset, and octave buttons.
//! - Test pattern rendered onto the 8 OLED parameter displays (128x32 mono each).
//! - Live console telemetry reporting knob touch sensors, rotary encoders, and button presses.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin kk_mk1_vegas

use std::time::{Duration, Instant};
use encdr::{Encdr, EncdrConfig, Event, LedValue, PixelFormat};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".parse().unwrap()))
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     NI Komplete Kontrol Mk1 — Vegas Mode & Telemetry         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| {
            d.vendor_id.0 == 0x17cc && (d.product_id.0 == 0x1340 || d.product_id.0 == 0x1350 || d.product_id.0 == 0x1360 || d.product_id.0 == 0x1410)
        }).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Komplete Kontrol S-Series Mk1 (0x17cc:0x1340/0x1350/0x1360/0x1410) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Touch/rotate encoders or press buttons. Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();
    let mut last_screen_update = Instant::now();

    let buttons = [
        "shift", "scale", "arp", "preset_up", "preset_down",
        "octave_down", "octave_up", "play", "rec", "stop", "loop",
        "instance", "page_left", "page_right"
    ];

    let key_count = match desc.product_id.0 {
        0x1340 => 25,
        0x1360 => 61,
        0x1410 => 88,
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
                Event::DeviceDisconnected { .. } => {
                    eprintln!("Device disconnected.");
                    return;
                }
                _ => {}
            }
        }

        if last_led_update.elapsed() >= Duration::from_millis(30) {
            last_led_update = Instant::now();

            // 1. Light Guide animation
            let mut lg_pattern = vec![0u8; key_count];
            for (idx, slot) in lg_pattern.iter_mut().enumerate() {
                let wave = ((elapsed * 4.0 + (idx as f32) * 0.3).sin() * 63.0 + 64.0) as u8;
                *slot = wave;
            }
            encdr.set_led_strip(device_id, "light_guide", &lg_pattern);

            // 2. Pulse function and transport buttons
            for (idx, &btn) in buttons.iter().enumerate() {
                let wave = ((elapsed * 4.0 + (idx as f32) * 0.25).sin() * 63.0 + 64.0) as u8;
                encdr.set_led(device_id, btn, LedValue::Single(wave));
            }
        }

        // 3. Periodic display animation for the 8 OLEDs (128x32 mono = 512 bytes each)
        if last_screen_update.elapsed() >= Duration::from_millis(100) {
            last_screen_update = Instant::now();
            let frame_idx = (elapsed * 5.0) as usize;

            for disp_num in 1..=8 {
                let screen_name = format!("display_{}", disp_num);
                let mut mono_buf = [0u8; 512]; // 128 cols * 4 pages
                for col in 0..128 {
                    let page = (col + frame_idx * 4 + disp_num * 8) % 32 / 8;
                    let bit = (col + frame_idx * 4 + disp_num * 8) % 8;
                    mono_buf[page * 128 + col] |= 1 << bit;
                }
                encdr.submit_screen_with_format(device_id, &screen_name, &mono_buf, PixelFormat::Mono);
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
