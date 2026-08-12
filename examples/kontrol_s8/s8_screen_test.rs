//! S8 screen test: renders deck controls on both S8 screens via encdr-view.
//!
//! Run with: cargo run -p encdr-view --example s8_screen_test
//! Show desktop windows: cargo run -p encdr-view --example s8_screen_test -- --visible

use std::collections::HashMap;
use std::time::Duration;

use encdr::{Encdr, EncdrConfig, Event, LedValue};
use encdr_view::{ScreenContent, ScreenView};

/// Maps input button suffix → LED name for single-color LEDs.
const DECK_LED_BUTTONS: &[&str] = &[
    "fx_select",
    "fx_1", "fx_2", "fx_3", "fx_4",
    "on_1", "on_2", "on_3", "on_4",
    "screen_left_1", "screen_left_2", "screen_left_3", "screen_left_4",
    "screen_right_1", "screen_right_2", "screen_right_3", "screen_right_4",
    "back", "capture", "edit",
    "flux", "shift", "cue", "play",
];

/// Buttons with dual-color LEDs: (button_suffix, white/active_led, blue/idle_led).
const DECK_LED_BUTTONS_DUAL: &[(&str, &str, &str)] = &[
    ("hotcue",  "hotcue_white",  "hotcue_blue"),
    ("loop",    "loop_white",    "loop_blue"),
    ("freeze",  "freeze_white",  "freeze_blue"),
    ("remix",   "remix_white",   "remix_blue"),
    ("deck",    "deck_white",    "deck_blue"),
    ("sync",    "sync_green",    "sync_red"),
];

/// Build touchstrip LEDs for both channels: orange center, blue on either side (position 0.0-1.0 maps to LED 0-24).
fn touchstrip_orange(pos: f32) -> Vec<u8> {
    let pos_clamped = pos.clamp(0.0, 1.0);
    let center = (pos_clamped * 24.0).round() as usize;
    let mut leds = vec![0u8; 25];
    if center < 25 {
        leds[center] = 200; // Orange bright at center
    }
    leds
}

fn touchstrip_blue(pos: f32) -> Vec<u8> {
    let pos_clamped = pos.clamp(0.0, 1.0);
    let center = (pos_clamped * 24.0).round() as usize;
    let mut leds = vec![0u8; 25];
    // Blue dim on either side of center
    if center > 0 {
        leds[center - 1] = 80;
    }
    if center < 25 {
        leds[center] = 0; // Center is orange, not blue
    }
    if center < 24 {
        leds[center + 1] = 80;
    }
    leds
}

/// Map deck prefix to LED group id.
fn led_group(name: &str) -> Option<(&str, &str)> {
    if let Some(suffix) = name.strip_prefix("left_") {
        Some(("left_deck", suffix))
    } else if let Some(suffix) = name.strip_prefix("right_") {
        Some(("right_deck", suffix))
    } else {
        None
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".parse().unwrap()),
        )
        .init();

    let visible = std::env::args().any(|a| a == "--visible");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");

    println!("=== S8 Screen Test ===");
    println!("Scanning for devices...");

    let ids = encdr.scan().expect("Scan failed");
    if ids.is_empty() {
        eprintln!("No devices found. Is the S8 plugged in?");
        return;
    }

    let device_id = ids[0];
    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected: {} ({} controls)", desc.name, desc.control_count());

    // Wait for device initialization
    std::thread::sleep(Duration::from_millis(500));

    let html = include_str!("../../screens/s8_deck.html");

    // Create left screen
    let left_view = match ScreenView::new(
        &encdr,
        device_id,
        "left",
        ScreenContent::Html(html.to_string()),
        visible,
    ) {
        Ok(v) => {
            println!("Left screen created (visible={})", visible);
            v
        }
        Err(e) => {
            eprintln!("Left screen failed: {}", e);
            return;
        }
    };

    // Create right screen
    let right_view = match ScreenView::new(
        &encdr,
        device_id,
        "right",
        ScreenContent::Html(html.to_string()),
        visible,
    ) {
        Ok(v) => {
            println!("Right screen created (visible={})", visible);
            v
        }
        Err(e) => {
            eprintln!("Right screen failed: {}", e);
            return;
        }
    };

    // Tell each screen which deck it represents
    left_view.send("deck", serde_json::json!("left"));
    right_view.send("deck", serde_json::json!("right"));

    // Set idle-blue state for all dual-color LEDs and loop circle LEDs
    for group in &["left_deck", "right_deck"] {
        for (_btn, _white, blue) in DECK_LED_BUTTONS_DUAL {
            encdr.set_led_in_group(device_id, group, blue, LedValue::Single(64));
        }
        for i in 1..=4 {
            let name = format!("loop_circle_{}_blue", i);
            encdr.set_led_in_group(device_id, group, &name, LedValue::Single(64));
        }
    }

    // Set initial touchstrip LED position (center)
    let init_orange = touchstrip_orange(0.5);
    let init_blue = touchstrip_blue(0.5);
    encdr.set_led_strip_in_group(device_id, "left_deck", "touchstrip_orange", &init_orange);
    encdr.set_led_strip_in_group(device_id, "left_deck", "touchstrip_blue", &init_blue);
    encdr.set_led_strip_in_group(device_id, "right_deck", "touchstrip_orange", &init_orange);
    encdr.set_led_strip_in_group(device_id, "right_deck", "touchstrip_blue", &init_blue);

    println!("Listening for events... (Ctrl+C to quit)\n");

    let events = encdr.events().clone();

    let mut controls: HashMap<String, serde_json::Value> = HashMap::new();
    let mut left_touchstrip_pos: f32 = 0.5;
    let mut right_touchstrip_pos: f32 = 0.5;
    let mut left_dirty = true;
    let mut right_dirty = true;
    let mut left_needs_capture = false;
    let mut right_needs_capture = false;
    let mut frame_count = 0u64;

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

                        if let Some((group, suffix)) = led_group(name) {
                            let led_val = if toggled {
                                LedValue::Single(127)
                            } else {
                                LedValue::Off
                            };

                            // Check single-color LEDs
                            if DECK_LED_BUTTONS.contains(&suffix) {
                                encdr.set_led_in_group(
                                    device_id, group, suffix, led_val,
                                );
                            }
                            // Check dual-color LEDs — white/active when on, blue/idle when off
                            for (btn_suffix, white_led, blue_led) in DECK_LED_BUTTONS_DUAL {
                                if suffix == *btn_suffix {
                                    let (on_led, off_led) = (*white_led, *blue_led);
                                    if toggled {
                                        encdr.set_led_in_group(device_id, group, on_led, LedValue::Single(127));
                                        encdr.set_led_in_group(device_id, group, off_led, LedValue::Off);
                                    } else {
                                        encdr.set_led_in_group(device_id, group, on_led, LedValue::Off);
                                        encdr.set_led_in_group(device_id, group, off_led, LedValue::Single(64));
                                    }
                                    break;
                                }
                            }
                            // Check pads — pad_1 through pad_8
                            if let Some(pad_str) = suffix.strip_prefix("pad_") {
                                if let Ok(pad_num) = pad_str.parse::<usize>() {
                                    let led_name = format!("pad_{}", pad_num);
                                    let pad_led = if toggled {
                                        LedValue::Rgb {
                                            r: 255,
                                            g: 69,
                                            b: 0,
                                        }
                                    } else {
                                        LedValue::Off
                                    };
                                    encdr.set_led_in_group(
                                        device_id, group, &led_name, pad_led,
                                    );
                                }
                            }
                        }

                        let side = if name.starts_with("left_") {
                            "left"
                        } else if name.starts_with("right_") {
                            "right"
                        } else {
                            "both"
                        };

                        let msg = format!(
                            "{} {}",
                            name,
                            if toggled { "ON" } else { "off" }
                        );

                        match side {
                            "left" => {
                                left_view.send("event", serde_json::json!(msg));
                                left_dirty = true;
                            }
                            "right" => {
                                right_view.send("event", serde_json::json!(msg));
                                right_dirty = true;
                            }
                            _ => {
                                left_view.send("event", serde_json::json!(msg));
                                right_view.send("event", serde_json::json!(msg));
                                left_dirty = true;
                                right_dirty = true;
                            }
                        }
                    }
                }
                Event::Slider { name, value, .. } => {
                    controls.insert(name.to_string(), serde_json::json!(value));

                    // Drive touchstrip LEDs based on position (hold last position on release)
                    if *name == "left_touchstrip" {
                        println!("left_touchstrip value={:.4}", value);
                        if *value > 0.0 { left_touchstrip_pos = *value; }
                        let orange = touchstrip_orange(left_touchstrip_pos);
                        let blue = touchstrip_blue(left_touchstrip_pos);
                        encdr.set_led_strip_in_group(device_id, "left_deck", "touchstrip_orange", &orange);
                        encdr.set_led_strip_in_group(device_id, "left_deck", "touchstrip_blue", &blue);
                    } else if *name == "right_touchstrip" {
                        if *value > 0.0 { right_touchstrip_pos = *value; }
                        let orange = touchstrip_orange(right_touchstrip_pos);
                        let blue = touchstrip_blue(right_touchstrip_pos);
                        encdr.set_led_strip_in_group(device_id, "right_deck", "touchstrip_orange", &orange);
                        encdr.set_led_strip_in_group(device_id, "right_deck", "touchstrip_blue", &blue);
                    }

                    let msg = format!("{} {:.3}", name, value);
                    if name.starts_with("left_") {
                        left_view.send("event", serde_json::json!(msg));
                        left_dirty = true;
                    } else if name.starts_with("right_") {
                        right_view.send("event", serde_json::json!(msg));
                        right_dirty = true;
                    } else {
                        left_view.send("event", serde_json::json!(msg));
                        right_view.send("event", serde_json::json!(msg));
                        left_dirty = true;
                        right_dirty = true;
                    }
                }
                Event::Encoder { name, delta, .. } => {
                    let msg = format!("{} {:+}", name, delta);
                    if name.starts_with("left_") {
                        left_view.send("event", serde_json::json!(msg));
                    } else if name.starts_with("right_") {
                        right_view.send("event", serde_json::json!(msg));
                    }
                }
                Event::EncoderFine { name, delta, .. } => {
                    // Accumulate delta into a 0.0–1.0 value
                    let current = controls
                        .get(*name)
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.5) as f32;
                    let new_val = (current + delta).clamp(0.0, 1.0);
                    controls.insert(name.to_string(), serde_json::json!(new_val));

                    let msg = format!("{} {:.3}", name, new_val);
                    if name.starts_with("left_") {
                        left_view.send("event", serde_json::json!(msg));
                        left_dirty = true;
                    } else if name.starts_with("right_") {
                        right_view.send("event", serde_json::json!(msg));
                        right_dirty = true;
                    }
                }
                Event::Touch { name, touched, .. } => {
                    controls.insert(name.to_string(), serde_json::json!(touched));
                    if name.starts_with("left_") {
                        left_dirty = true;
                    } else if name.starts_with("right_") {
                        right_dirty = true;
                    } else {
                        left_dirty = true;
                        right_dirty = true;
                    }
                }
                _ => {}
            }
        }

        // Push state to left screen
        if left_dirty {
            left_view.send("controls", serde_json::json!(controls));
            left_dirty = false;
            left_needs_capture = true;
        }

        // Push state to right screen
        if right_dirty {
            right_view.send("controls", serde_json::json!(controls));
            right_dirty = false;
            right_needs_capture = true;
        }

        // Poll for frame-ready
        left_view.poll(&encdr);
        right_view.poll(&encdr);

        // Force capture after state push
        if left_needs_capture || right_needs_capture {
            ScreenView::pump_events();
            if left_needs_capture {
                let _ = left_view.capture_and_submit(&encdr);
                left_needs_capture = false;
            }
            if right_needs_capture {
                let _ = right_view.capture_and_submit(&encdr);
                right_needs_capture = false;
            }
        }

        frame_count += 1;
        // Periodic keyframe every ~2 seconds
        if frame_count % 250 == 0 {
            let _ = left_view.capture_and_submit(&encdr);
            let _ = right_view.capture_and_submit(&encdr);
        }

        std::thread::sleep(Duration::from_millis(8));
    }
}
