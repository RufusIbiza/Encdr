//! Test: Orange LED tracks left touchstrip position.
//!
//! The LED:
//! - Starts at center (0.5)
//! - Moves with the strip when touched (byte_29 == 1)
//! - Stays at last known position when not touched
//!
//! Run with: cargo run -p encdr-view --example touchstrip_position_test

use encdr::{Encdr, EncdrConfig, Event, DeviceId};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");

    println!("=== Touchstrip Position Test ===");
    println!("Scanning for devices...\n");

    let ids = encdr.scan().expect("Scan failed");
    if ids.is_empty() {
        eprintln!("No devices found. Is the S8 plugged in?");
        return;
    }

    let device_id = ids[0];
    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected: {} ({} controls)\n", desc.name, desc.control_count());

    println!("Orange LED at center. Touch the left touchstrip to move it.\n");
    println!("LED position | Touch state | Raw position");
    println!("{}", "-".repeat(60));

    let events = encdr.events().clone();

    // Track state for each strip: position > 0 means touching, position == 0 means released
    let mut left_position: f32 = 0.5;
    let mut right_position: f32 = 0.5;
    let mut last_left: f32 = -1.0;
    let mut last_right: f32 = -1.0;

    loop {
        while let Ok(event) = events.try_recv() {
            if let Event::Slider { name, value, .. } = &event {
                let touch_status = if *value > 0.0 { "TOUCHED" } else { "RELEASED" };
                match *name {
                    "left_touchstrip" => {
                        if *value > 0.0 { left_position = *value; }
                        println!("  LEFT  {}  {}  raw={:.4}", format_led_position(left_position), touch_status, value);
                    }
                    "right_touchstrip" => {
                        if *value > 0.0 { right_position = *value; }
                        println!("  RIGHT {}  {}  raw={:.4}", format_led_position(right_position), touch_status, value);
                    }
                    _ => {}
                }
            }
        }

        if (left_position - last_left).abs() > 0.02 {
            last_left = left_position;
            set_touchstrip_led(&encdr, device_id, "left_touchstrip", left_position);
        }
        if (right_position - last_right).abs() > 0.02 {
            last_right = right_position;
            set_touchstrip_led(&encdr, device_id, "right_touchstrip", right_position);
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Visualize LED position as a string: [===●===] for 50%, [●======] for 0%, etc.
fn format_led_position(position: f32) -> String {
    let pos = (position * 10.0).clamp(0.0, 10.0) as usize;
    let mut visual = String::from("[");
    for i in 0..10 {
        if i == pos {
            visual.push('●');
        } else {
            visual.push('=');
        }
    }
    visual.push(']');
    visual
}

/// Set the orange LED on the given touchstrip to the given position (0.0-1.0).
fn set_touchstrip_led(encdr: &Encdr, device_id: DeviceId, group: &str, position: f32) {
    let mut orange = vec![0u8; 25];
    let led_index = ((position * 24.0).round() as usize).min(24);
    orange[led_index] = 255;

    let blue_off = vec![0u8; 25];
    encdr.set_led_strip_in_group(device_id, group, "touchstrip_orange", &orange);
    encdr.set_led_strip_in_group(device_id, group, "touchstrip_blue", &blue_off);
}
