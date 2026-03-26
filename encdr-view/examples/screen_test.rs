//! D2 screen test: renders knob/button positions on the D2 screen via encdr-view.
//!
//! Run with: cargo run -p encdr-view --example screen_test
//! Show desktop window: cargo run -p encdr-view --example screen_test -- --visible

use std::collections::HashMap;
use std::time::Duration;

use encdr::{Encdr, EncdrConfig, Event, LedValue};
use encdr_view::{ScreenContent, ScreenView};

/// Button names that have corresponding single-color hardware LEDs (same name).
const LED_BUTTONS: &[&str] = &[
    "fx_select",
    "fx_1", "fx_2", "fx_3", "fx_4",
    "screen_left_1", "screen_left_2", "screen_left_3", "screen_left_4",
    "screen_right_1", "screen_right_2", "screen_right_3", "screen_right_4",
    "back", "capture", "edit",
    "on_1", "on_2", "on_3", "on_4",
    "flux", "shift", "cue", "play",
];

/// Button names where the LED has a `_white` suffix (dual-color LEDs).
const LED_BUTTONS_WHITE: &[&str] = &[
    "hotcue", "loop", "freeze", "remix", "deck",
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

    println!("=== D2 Screen Test ===");
    println!("Scanning for devices...");

    let ids = encdr.scan().expect("Scan failed");
    if ids.is_empty() {
        eprintln!("No devices found. Is the D2 plugged in?");
        return;
    }

    let device_id = ids[0];
    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected: {} ({} controls)", desc.name, desc.control_count());

    // Wait for splash screen to be sent by device thread
    std::thread::sleep(Duration::from_millis(500));

    let html = include_str!("../../screens/d2_controls.html");
    let view = match ScreenView::new(
        &encdr,
        device_id,
        "main",
        ScreenContent::Html(html.to_string()),
        visible,
    ) {
        Ok(v) => {
            println!("WebView created (visible={})", visible);
            v
        }
        Err(e) => {
            eprintln!("WebView failed: {}", e);
            return;
        }
    };

    println!("Listening for events... (Ctrl+C to quit)\n");

    let events = encdr.events().clone();

    // Track control state — buttons are toggled, encoders accumulate
    let mut controls: HashMap<String, serde_json::Value> = HashMap::new();
    let encoder_names = [
        "screen_encoder_1",
        "screen_encoder_2",
        "screen_encoder_3",
        "screen_encoder_4",
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
                    // Toggle on press, ignore release
                    if *pressed {
                        let current = controls
                            .get(*name)
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let toggled = !current;
                        controls.insert(name.to_string(), serde_json::json!(toggled));

                        // Update hardware LED if this button has one
                        let led_val = if toggled {
                            LedValue::Single(127)
                        } else {
                            LedValue::Off
                        };
                        if LED_BUTTONS.contains(name) {
                            encdr.set_led(device_id, name, led_val);
                        } else if LED_BUTTONS_WHITE.contains(name) {
                            let white_name = format!("{}_white", name);
                            encdr.set_led(device_id, &white_name, led_val);
                        }

                        view.send(
                            "event",
                            serde_json::json!(format!(
                                "{} {}",
                                name,
                                if toggled { "ON" } else { "off" }
                            )),
                        );
                        dirty = true;
                    }
                }
                Event::Slider { name, value, .. } => {
                    controls.insert(name.to_string(), serde_json::json!(value));
                    view.send("event", serde_json::json!(format!("{} {:.3}", name, value)));
                    dirty = true;
                }
                Event::EncoderFine { name, delta, .. } => {
                    let current = controls
                        .get(*name)
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.5);
                    let new_val = (current + *delta as f64).clamp(0.0, 1.0);
                    controls.insert(name.to_string(), serde_json::json!(new_val));
                    view.send(
                        "event",
                        serde_json::json!(format!("{} {:.3}", name, new_val)),
                    );
                    dirty = true;
                }
                Event::Touch { name, touched, .. } => {
                    controls.insert(name.to_string(), serde_json::json!(touched));
                    dirty = true;
                }
                Event::Encoder { name, delta, .. } => {
                    view.send("event", serde_json::json!(format!("{} {:+}", name, delta)));
                }
                _ => {}
            }
        }

        // Push state to WebView if dirty, and schedule an immediate capture
        if dirty {
            view.send("controls", serde_json::json!(controls));
            dirty = false;
            needs_capture = true;
        }

        // Capture and submit: check IPC frame-ready flag first
        view.poll(&encdr);

        // If we just pushed a state change, force a capture on the next iteration
        // to reduce latency (don't wait for the JS rAF round-trip)
        if needs_capture {
            // Give the WebView one pump cycle to process the JS
            ScreenView::pump_events();
            let _ = view.capture_and_submit(&encdr);
            needs_capture = false;
        }

        frame_count += 1;
        // Periodic keyframe every ~2 seconds
        if frame_count % 250 == 0 {
            if let Err(e) = view.capture_and_submit(&encdr) {
                if frame_count <= 250 {
                    eprintln!("Capture error: {}", e);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(8));
    }
}
