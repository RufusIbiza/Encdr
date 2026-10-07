//! NI Maschine Mk3 control button brightness & dimming test.
//!
//! All 47 monochrome control buttons illuminate at DIM brightness (half-brightness / 0xE4 / 228, 1 LED under each button).
//! Pressing any button elevates it to BRIGHT (active / 0x9E / 158, both LEDs active).
//! Pads, touchstrip, and group buttons remain OFF.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin mk3_button_brightness

use std::time::Instant;
use encdr::{Encdr, EncdrConfig, Event, LedValue};

/// All 47 monochrome control buttons on the Maschine Mk3.
/// (Pads, Group A-H buttons, and 4D encoder direction rings are excluded).
const CONTROL_BUTTONS: &[&str] = &[
    // Left & Right display column buttons
    "channel", "plugin", "arranger", "mixer", "browser", "sampling",
    "arrow_left", "arrow_right", "file", "settings", "auto", "macro",
    // Top display buttons 1–8
    "top_1", "top_2", "top_3", "top_4", "top_5", "top_6", "top_7", "top_8",
    // Performance & edit controls
    "volume", "swing", "note_repeat", "tempo", "lock", "pitch", "mod", "perform", "notes",
    // Transport controls
    "restart", "erase", "tap", "follow", "play", "rec", "stop", "shift",
    // Mode & sequencing buttons
    "fixed_vel", "pad_mode", "keyboard", "chords", "step", "scene", "pattern", "events",
    "variations", "duplicate", "select", "solo", "mute",
];

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".parse().unwrap()),
        )
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     NI Maschine Mk3 — Button Brightness & Dimming Test       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids
        .into_iter()
        .find(|&id| {
            encdr
                .device_descriptor(id)
                .map(|d| {
                    d.vendor_id.0 == 0x17cc
                        && (d.product_id.0 == 0x1600 || d.product_id.0 == 0x1820)
                })
                .unwrap_or(false)
        })
        .unwrap_or_else(|| {
            eprintln!("❌ NI Maschine Mk3 (0x17cc:0x1600) or Maschine Plus (0x17cc:0x1820) not found.");
            std::process::exit(1);
        });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(
        " Connected: {} (VID:PID 0x{:04x}:0x{:04x})",
        desc.name, desc.vendor_id.0, desc.product_id.0
    );
    println!(" Total control buttons in test: {}", CONTROL_BUTTONS.len());

    // 1. Initialize all control buttons to DIM (half-brightness / 0xE4)
    println!("\nSetting all {} control buttons to DIM (half-brightness)...", CONTROL_BUTTONS.len());
    for &btn in CONTROL_BUTTONS {
        encdr.set_led(device_id, btn, LedValue::Dim);
    }

    // 2. Explicitly ensure Group buttons and RGB pads are OFF
    for group_char in ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'] {
        encdr.set_led(device_id, &format!("group_{group_char}"), LedValue::Off);
    }
    for pad_num in 1..=16 {
        encdr.set_led(device_id, &format!("pad_{pad_num}"), LedValue::Off);
    }

    println!("All control buttons illuminated at DIM brightness (1 LED per button).");
    println!("Pads, touchstrip, and Group buttons remain OFF.\n");
    println!("Press any control button to test BRIGHT response. Release to return to DIM.");
    println!("Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    let btn_name = name.to_lowercase();
                    if CONTROL_BUTTONS.contains(&btn_name.as_str()) {
                        let new_val = if pressed {
                            LedValue::Bright
                        } else {
                            LedValue::Dim
                        };
                        encdr.set_led(device_id, &btn_name, new_val);

                        if pressed {
                            println!(
                                "[{:7.3}s] [BUTTON PRESSED ] {:<12} -> \x1b[1;32mBRIGHT\x1b[0m (0x9E / 158, 2 LEDs lit)",
                                elapsed,
                                btn_name.to_uppercase()
                            );
                        } else {
                            println!(
                                "[{:7.3}s] [BUTTON RELEASED] {:<12} -> \x1b[2;37mDIM\x1b[0m    (0xE4 / 228, half-brightness)",
                                elapsed,
                                btn_name.to_uppercase()
                            );
                        }
                    } else if btn_name.starts_with("group_") {
                        println!(
                            "[{:7.3}s] [GROUP BUTTON  ] {:<12} -> {} (RGB group button, ignored)",
                            elapsed,
                            btn_name.to_uppercase(),
                            if pressed { "PRESSED" } else { "RELEASED" }
                        );
                    } else if btn_name.starts_with("pad_") {
                        println!(
                            "[{:7.3}s] [PAD BUTTON    ] {:<12} -> {} (Pad button, ignored)",
                            elapsed,
                            btn_name.to_uppercase(),
                            if pressed { "PRESSED" } else { "RELEASED" }
                        );
                    } else {
                        println!(
                            "[{:7.3}s] [OTHER BUTTON  ] {:<12} -> {}",
                            elapsed,
                            btn_name.to_uppercase(),
                            if pressed { "PRESSED" } else { "RELEASED" }
                        );
                    }
                }
                Event::Grid { name, index, pressure, .. } => {
                    if pressure > 0.0 {
                        println!(
                            "[{:7.3}s] [PAD PRESSURE  ] {} #{:02} = {:5.1}% (Pads ignored)",
                            elapsed,
                            name.to_uppercase(),
                            index,
                            pressure * 100.0
                        );
                    }
                }
                _ => {}
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
