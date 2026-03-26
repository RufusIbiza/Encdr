//! Maschine Mk3 screen test: renders encoder/button state on dual screens via encdr-view.
//!
//! Run with: cargo run -p encdr-view --example mk3_screen_test
//! Show desktop windows: cargo run -p encdr-view --example mk3_screen_test -- --visible

use std::collections::HashMap;
use std::time::Duration;

use encdr::{Encdr, EncdrConfig, Event, LedValue};
use encdr_view::{ScreenContent, ScreenView};

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

    println!("=== Maschine Mk3 Screen Test ===");
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

    // Create two screen views — one for each display
    let left_html = include_str!("../../screens/mk3_left.html");
    let right_html = include_str!("../../screens/mk3_right.html");

    let left_view = match ScreenView::new(
        &encdr, device_id, "left",
        ScreenContent::Html(left_html.to_string()), visible,
    ) {
        Ok(v) => { println!("Left screen WebView created"); v }
        Err(e) => { eprintln!("Left WebView failed: {}", e); return; }
    };

    let right_view = match ScreenView::new(
        &encdr, device_id, "right",
        ScreenContent::Html(right_html.to_string()), visible,
    ) {
        Ok(v) => { println!("Right screen WebView created"); v }
        Err(e) => { eprintln!("Right WebView failed: {}", e); return; }
    };

    println!("Listening for events... (Ctrl+C to quit)\n");

    let events = encdr.events().clone();

    let mut controls: HashMap<String, serde_json::Value> = HashMap::new();
    let encoder_names = [
        "screen_encoder_1", "screen_encoder_2", "screen_encoder_3", "screen_encoder_4",
        "screen_encoder_5", "screen_encoder_6", "screen_encoder_7", "screen_encoder_8",
    ];
    for name in &encoder_names {
        controls.insert(name.to_string(), serde_json::json!(0.5));
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
                        let current = controls
                            .get(*name)
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let toggled = !current;
                        controls.insert(name.to_string(), serde_json::json!(toggled));

                        if LED_BUTTONS.contains(name) {
                            let led_val = if toggled { LedValue::Single(127) } else { LedValue::Off };
                            encdr.set_led(device_id, name, led_val);
                        }

                        let msg = serde_json::json!(format!(
                            "{} {}", name, if toggled { "ON" } else { "off" }
                        ));
                        left_view.send("event", msg.clone());
                        right_view.send("event", msg);
                        dirty = true;
                    }
                }
                Event::Slider { name, value, .. } => {
                    controls.insert(name.to_string(), serde_json::json!(value));
                    let msg = serde_json::json!(format!("{} {:.3}", name, value));
                    left_view.send("event", msg.clone());
                    right_view.send("event", msg);
                    dirty = true;
                }
                Event::EncoderFine { name, delta, .. } => {
                    let current = controls
                        .get(*name)
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.5);
                    let new_val = (current + *delta as f64).clamp(0.0, 1.0);
                    controls.insert(name.to_string(), serde_json::json!(new_val));
                    let msg = serde_json::json!(format!("{} {:.3}", name, new_val));
                    left_view.send("event", msg.clone());
                    right_view.send("event", msg);
                    dirty = true;
                }
                Event::Touch { name, touched, .. } => {
                    controls.insert(name.to_string(), serde_json::json!(touched));
                    dirty = true;
                }
                Event::Encoder { name, delta, .. } => {
                    let msg = serde_json::json!(format!("{} {:+}", name, delta));
                    left_view.send("event", msg.clone());
                    right_view.send("event", msg);
                }
                _ => {}
            }
        }

        if dirty {
            let state = serde_json::json!(controls);
            left_view.send("controls", state.clone());
            right_view.send("controls", state);
            dirty = false;
            needs_capture = true;
        }

        left_view.poll(&encdr);
        right_view.poll(&encdr);

        if needs_capture {
            ScreenView::pump_events();
            let _ = left_view.capture_and_submit(&encdr);
            let _ = right_view.capture_and_submit(&encdr);
            needs_capture = false;
        }

        frame_count += 1;
        if frame_count % 250 == 0 {
            let _ = left_view.capture_and_submit(&encdr);
            let _ = right_view.capture_and_submit(&encdr);
        }

        std::thread::sleep(Duration::from_millis(8));
    }
}
