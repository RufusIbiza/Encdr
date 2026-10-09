//! Maschine Mk3 dual screen test: renders both displays via a single 960x272 WebView with DualScreenView.
//!
//! Run with: cargo run -p encdr-examples --bin mk3_dual_screen
//! Show desktop preview: cargo run -p encdr-examples --bin mk3_dual_screen -- --visible

use std::collections::HashMap;
use std::time::Duration;

use encdr::{Encdr, EncdrConfig, Event, LedValue};
use encdr_view::{DualScreenView, ScreenContent, ScreenView};

/// Buttons that have matching single-color hardware LEDs.
const LED_BUTTONS: &[&str] = &[
    "top_1", "top_2", "top_3", "top_4", "top_5", "top_6", "top_7", "top_8",
    "group_a", "group_b", "group_c", "group_d",
    "group_e", "group_f", "group_g", "group_h",
    "play", "rec", "stop", "restart", "erase", "tap", "follow",
    "shift", "notes", "volume", "swing", "tempo", "note_repeat", "lock",
    "pad_mode", "keyboard", "chords", "step", "fixed_vel",
    "scene", "pattern", "events", "variations", "duplicate", "select", "solo", "mute",
    "perform", "pitch", "mod",
    "channel", "plugin", "arranger", "mixer", "browser", "sampling",
    "arrow_left", "arrow_right", "file", "settings", "auto", "macro",
    "encoder_up", "encoder_down", "encoder_left", "encoder_right",
];

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".parse().unwrap()),
        )
        .init();

    let visible = std::env::args().any(|a| a == "--visible");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");

    println!("=== Maschine Mk3 Dual Screen Test (Single 960x272 WebView) ===");
    println!("Scanning for devices...");

    let ids = encdr.scan().expect("Scan failed");
    if ids.is_empty() {
        eprintln!("No devices found. Is the Maschine Mk3 plugged in?");
        return;
    }

    let device_id = ids[0];
    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected: {} ({} controls)", desc.name, desc.control_count());

    std::thread::sleep(Duration::from_millis(500));

    // Create a single 960x272 DualScreenView spanning both left and right displays
    let dual_html = include_str!("../../screens/mk3_dual_screen.html");
    let dual_view = match DualScreenView::new(
        &encdr,
        device_id,
        ScreenContent::Html(dual_html.to_string()),
        visible,
    ) {
        Ok(v) => {
            println!("Created unified 960x272 DualScreenView");
            v
        }
        Err(e) => {
            eprintln!("Failed to create DualScreenView: {}", e);
            return;
        }
    };

    let events = encdr.events().clone();

    let mut state: HashMap<String, serde_json::Value> = HashMap::new();
    for i in 1..=8 {
        state.insert(format!("screen_encoder_{}", i), serde_json::json!(0.5));
        state.insert(format!("encoder_touch_{}", i), serde_json::json!(false));
        state.insert(format!("top_{}", i), serde_json::json!(false));
    }
    for c in ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'] {
        state.insert(format!("group_{}", c), serde_json::json!(false));
    }

    let mut dirty = true;
    let mut frame_count = 0u64;
    let mut needs_capture = false;

    loop {
        ScreenView::pump_events();

        while let Ok(event) = events.try_recv() {
            match &event {
                Event::Button { name, pressed, .. } => {
                    if *pressed {
                        let current = state
                            .get(*name)
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let toggled = !current;
                        state.insert(name.to_string(), serde_json::json!(toggled));

                        if LED_BUTTONS.contains(name) {
                            let led_val = if toggled { LedValue::Single(127) } else { LedValue::Off };
                            encdr.set_led(device_id, name, led_val);
                        }

                        state.insert("event".to_string(), serde_json::json!(format!(
                            "{} {}", name, if toggled { "ON" } else { "off" }
                        )));
                        dirty = true;
                    }
                }
                Event::EncoderFine { name, delta, .. } => {
                    let current = state
                        .get(*name)
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.5);
                    let new_val = (current + *delta as f64).clamp(0.0, 1.0);
                    state.insert(name.to_string(), serde_json::json!(new_val));
                    state.insert("event".to_string(), serde_json::json!(format!("{} {:.3}", name, new_val)));
                    dirty = true;
                }
                Event::Touch { name, touched, .. } => {
                    state.insert(name.to_string(), serde_json::json!(touched));
                    dirty = true;
                }
                Event::Slider { name, value, .. } => {
                    state.insert(name.to_string(), serde_json::json!(value));
                    state.insert("event".to_string(), serde_json::json!(format!("{} {:.3}", name, value)));
                    dirty = true;
                }
                Event::Encoder { name, delta, .. } => {
                    state.insert("event".to_string(), serde_json::json!(format!("{} {:+}", name, delta)));
                    dirty = true;
                }
                _ => {}
            }
        }

        if dirty {
            dual_view.send("state", serde_json::json!(state));
            dirty = false;
            needs_capture = true;
        }

        dual_view.poll(&encdr);

        if needs_capture {
            ScreenView::pump_events();
            let _ = dual_view.capture_and_submit(&encdr);
            needs_capture = false;
        }

        frame_count += 1;
        if frame_count % 250 == 0 {
            let _ = dual_view.capture_and_submit(&encdr);
        }

        std::thread::sleep(Duration::from_millis(8));
    }
}
