//! NI Maschine Mk3 interactive Vegas LED demo & hardware telemetry.
//!
//! Features:
//! - Sweeping rainbow wave across all 16 RGB pads and Group A-H buttons.
//! - Animated dual-color wave pattern across the Smart Strip touchstrip.
//! - Chasing brightness wave across transport, performance, and mode buttons.
//! - Animated rainbow on both screens (works on Windows with the NI services running).
//! - Every pad velocity, touchstrip position, knob turn, encoder gesture, and button click is
//!   written to `mk3-input-log.txt` (the console only shows a status line).
//!
//! Run with:
//!   cargo run -p encdr-examples --bin mk3_vegas
//!   cargo run -p encdr-examples --bin mk3_vegas -- --no-screens

use std::fs::File;
use std::io::Write;
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

const SCREEN_W: usize = 960;
const SCREEN_H: usize = 272;

/// Rainbow with a moving white bar across both screens, as RGBA8888.
fn screen_frame(t: f32) -> Vec<u8> {
    let mut px = vec![255u8; SCREEN_W * SCREEN_H * 4];
    let bar = ((t * 0.35).fract() * SCREEN_W as f32) as usize;
    for y in 0..SCREEN_H {
        let v = 0.35 + 0.65 * (y as f32 / SCREEN_H as f32);
        for x in 0..SCREEN_W {
            let (r, g, b) = if x.abs_diff(bar) < 5 {
                (255, 255, 255)
            } else {
                hsv_to_rgb(x as f32 * 360.0 / SCREEN_W as f32 + t * 120.0, 1.0, v)
            };
            let i = (y * SCREEN_W + x) * 4;
            px[i] = r;
            px[i + 1] = g;
            px[i + 2] = b;
        }
    }
    px
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".parse().unwrap()))
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         NI Maschine Mk3 — Vegas Mode & Telemetry             ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1600).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Maschine Mk3 (0x17cc:0x1600) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    let screens = !std::env::args().any(|a| a == "--no-screens") && !desc.screens.is_empty();
    let mut input_log = File::create("mk3-input-log.txt").expect("Could not create mk3-input-log.txt");
    println!("Input events are written to mk3-input-log.txt. Screens: {}.", if screens { "on" } else { "off" });
    println!("Interact with buttons, knobs, touchstrip, and pads. Press Ctrl+C to quit.\n");
    let mut input_events = 0u64;
    let mut frames = 0u64;
    let mut last_screen = Instant::now();
    let mut next_status = Instant::now() + Duration::from_secs(2);

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
                    input_events += 1;
                    writeln!(input_log, "[{:8.3}s] [BUTTON] {} -> {}", elapsed, name.to_uppercase(), if pressed { "PRESSED" } else { "RELEASED" }).ok();
                }
                Event::Grid { name, index, pressure, .. } => {
                    if (1..=16).contains(&index) {
                        pressed_pads[(index - 1) as usize] = pressure > 0.0;
                    }
                    input_events += 1;
                    writeln!(input_log, "[{:8.3}s] [GRID] {} (#{index:02}) pressure: {:5.1}%", elapsed, name.to_uppercase(), pressure * 100.0).ok();
                }
                Event::Slider { name, value, .. } => {
                    input_events += 1;
                    writeln!(input_log, "[{:8.3}s] [TOUCHSTRIP] {} = {:5.1}%", elapsed, name.to_uppercase(), value * 100.0).ok();
                }
                Event::Touch { name, touched, .. } => {
                    input_events += 1;
                    writeln!(input_log, "[{:8.3}s] [CAP-TOUCH] {} -> {}", elapsed, name.to_uppercase(), if touched { "TOUCHED" } else { "RELEASED" }).ok();
                }
                Event::Encoder { name, delta, .. } => {
                    input_events += 1;
                    writeln!(input_log, "[{:8.3}s] [ENCODER] {} delta: {:+2}", elapsed, name.to_uppercase(), delta).ok();
                }
                Event::EncoderFine { name, delta, .. } => {
                    input_events += 1;
                    writeln!(input_log, "[{:8.3}s] [ENCODER FINE] {} delta: {:+.3}", elapsed, name.to_uppercase(), delta).ok();
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

            // 1. Rainbow waves on 16 RGB Pads
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

        if screens && last_screen.elapsed() >= Duration::from_millis(50) {
            last_screen = Instant::now();
            encdr.submit_dual_screen(device_id, "left", "right", &screen_frame(elapsed));
            frames += 1;
        }

        if Instant::now() >= next_status {
            next_status = Instant::now() + Duration::from_secs(2);
            input_log.flush().ok();
            println!("[{elapsed:6.1}s] screen frames submitted: {frames}, input events logged: {input_events}");
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
